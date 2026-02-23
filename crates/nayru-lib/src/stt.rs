//! Speech-to-text protocol — VAD, transcription client, cancellation handles

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use nayru_core::types::{SttListenEvent, SttResponse};
use nayru_core::wav::{compute_rms, validate_stt_model, write_wav, SAMPLE_RATE};

use crate::capture::AudioCapture;

// VAD constants
const SILENCE_THRESHOLD: f32 = 0.004;
const MIN_SPEECH_MS: u64 = 180;
const SILENCE_DURATION_MS: u64 = 700;
const MAX_CAPTURE_MS: u64 = 12_000;
const NO_SPEECH_TIMEOUT_MS: u64 = 7_000;
const VAD_LEVEL_EMIT_INTERVAL: u32 = 5;

// ---------------------------------------------------------------------------
// STT handle manager (cancellation tokens)
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct SttHandles {
    inner: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl SttHandles {
    pub fn create(&self, id: &str) -> Arc<AtomicBool> {
        let token = Arc::new(AtomicBool::new(false));
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id.to_string(), token.clone());
        token
    }

    pub fn cancel(&self, id: &str) {
        if let Some(token) = self
            .inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
        {
            token.store(true, Ordering::Relaxed);
        }
    }

    pub fn remove(&self, id: &str) {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(id);
    }
}

// ---------------------------------------------------------------------------
// Transcribe WAV bytes via local Whisper server
// ---------------------------------------------------------------------------

pub async fn transcribe_wav(wav_bytes: &[u8], model: &str) -> Result<(String, Option<u64>), String> {
    let client = reqwest::Client::new();
    let part = reqwest::multipart::Part::bytes(wav_bytes.to_vec())
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| format!("mime error: {e}"))?;

    let form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("model", model.to_string())
        .text("language", "en")
        .text("response_format", "json");

    let resp = client
        .post("http://localhost:2022/v1/audio/transcriptions")
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("transcription request failed: {e}"))?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("transcription failed ({status}): {body}"));
    }

    let body = resp
        .text()
        .await
        .map_err(|e| format!("response read error: {e}"))?;
    let value: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("invalid JSON: {e}; raw={body}"))?;

    let raw_text = value.get("text").and_then(|v| v.as_str()).unwrap_or("");
    let text = raw_text.replace("[BLANK_AUDIO]", "").trim().to_string();
    let duration_ms = value.get("duration_ms").and_then(|v| v.as_u64());

    Ok((text, duration_ms))
}

// ---------------------------------------------------------------------------
// One-shot capture + transcribe
// ---------------------------------------------------------------------------

pub async fn transcribe_once(seconds: u64, model: &str) -> Result<SttResponse, String> {
    let secs = seconds.clamp(1, 15);
    validate_stt_model(model)?;

    let mut capture = AudioCapture::new()?;
    let total_samples = SAMPLE_RATE as usize * secs as usize;
    let mut audio_buffer: Vec<i16> = Vec::with_capacity(total_samples);

    while audio_buffer.len() < total_samples {
        let chunk = capture.read_chunk().await?;
        audio_buffer.extend_from_slice(&chunk);
    }
    audio_buffer.truncate(total_samples);
    drop(capture);

    if audio_buffer.is_empty() {
        return Ok(SttResponse {
            text: String::new(),
            duration_ms: None,
        });
    }

    let wav = write_wav(&audio_buffer, SAMPLE_RATE);
    let (text, duration_ms) = transcribe_wav(&wav, model).await?;

    let capture_ms = (audio_buffer.len() as u64 * 1000) / SAMPLE_RATE as u64;
    Ok(SttResponse {
        text,
        duration_ms: duration_ms.or(Some(capture_ms)),
    })
}

// ---------------------------------------------------------------------------
// VAD listen loop
// ---------------------------------------------------------------------------

pub async fn listen(
    listen_id: &str,
    model: &str,
    cancel: Arc<AtomicBool>,
    on_event: impl Fn(SttListenEvent),
) -> Result<SttResponse, String> {
    validate_stt_model(model)?;

    let mut capture = AudioCapture::new()?;
    let mut audio_buffer: Vec<i16> = Vec::new();

    let start = Instant::now();
    let mut speech_detected = false;
    let mut speech_start: Option<Instant> = None;
    let mut silence_start: Option<Instant> = None;
    let mut chunk_count: u32 = 0;
    let mut speech_event_emitted = false;

    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelled".to_string());
        }

        let samples = match tokio::time::timeout(
            std::time::Duration::from_millis(500),
            capture.read_chunk(),
        )
        .await
        {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                if audio_buffer.is_empty() {
                    return Err(format!("audio capture error: {e}"));
                }
                break;
            }
            Err(_) => {
                return Err("audio capture read timeout".to_string());
            }
        };

        let rms = compute_rms(&samples);
        let elapsed = start.elapsed();
        chunk_count += 1;

        if chunk_count % VAD_LEVEL_EMIT_INTERVAL == 0 {
            on_event(SttListenEvent {
                listen_id: listen_id.to_string(),
                event_type: "vad_level".to_string(),
                rms_level: Some(rms),
            });
        }

        if rms > SILENCE_THRESHOLD {
            silence_start = None;
            if !speech_detected {
                speech_detected = true;
                speech_start = Some(Instant::now());
            }

            if !speech_event_emitted {
                if let Some(ss) = speech_start {
                    if ss.elapsed().as_millis() as u64 >= MIN_SPEECH_MS {
                        speech_event_emitted = true;
                        on_event(SttListenEvent {
                            listen_id: listen_id.to_string(),
                            event_type: "speech_start".to_string(),
                            rms_level: Some(rms),
                        });
                    }
                }
            }

            audio_buffer.extend_from_slice(&samples);
        } else {
            if speech_detected {
                audio_buffer.extend_from_slice(&samples);

                let speech_dur = speech_start
                    .map(|s| s.elapsed().as_millis() as u64)
                    .unwrap_or(0);
                if speech_dur >= MIN_SPEECH_MS {
                    if silence_start.is_none() {
                        silence_start = Some(Instant::now());
                    }
                    let silence_dur = silence_start
                        .map(|s| s.elapsed().as_millis() as u64)
                        .unwrap_or(0);
                    if silence_dur >= SILENCE_DURATION_MS {
                        break;
                    }
                }
            }
        }

        if !speech_detected && elapsed.as_millis() as u64 >= NO_SPEECH_TIMEOUT_MS {
            return Ok(SttResponse {
                text: String::new(),
                duration_ms: None,
            });
        }

        if speech_detected && elapsed.as_millis() as u64 >= MAX_CAPTURE_MS {
            break;
        }
    }

    drop(capture);

    if audio_buffer.is_empty() {
        return Ok(SttResponse {
            text: String::new(),
            duration_ms: None,
        });
    }

    on_event(SttListenEvent {
        listen_id: listen_id.to_string(),
        event_type: "transcribing".to_string(),
        rms_level: None,
    });

    let wav = write_wav(&audio_buffer, SAMPLE_RATE);
    let (text, duration_ms) = transcribe_wav(&wav, model).await?;

    let capture_ms = (audio_buffer.len() as u64 * 1000) / SAMPLE_RATE as u64;

    Ok(SttResponse {
        text,
        duration_ms: duration_ms.or(Some(capture_ms)),
    })
}
