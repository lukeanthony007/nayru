//! Speech-to-text protocol — VAD, WAV encoding, transcription client, cancellation handles

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::capture::AudioCapture;

// ---------------------------------------------------------------------------
// VAD constants (matching existing WebView VAD)
// ---------------------------------------------------------------------------
pub const SAMPLE_RATE: u32 = 16_000;

const SILENCE_THRESHOLD: f32 = 0.004;
const MIN_SPEECH_MS: u64 = 180;
const SILENCE_DURATION_MS: u64 = 700;
const MAX_CAPTURE_MS: u64 = 12_000;
const NO_SPEECH_TIMEOUT_MS: u64 = 7_000;

// How often to emit vad_level events (every N chunks = N * 100 ms)
const VAD_LEVEL_EMIT_INTERVAL: u32 = 5; // every 500 ms

// Valid Whisper model names
const VALID_STT_MODELS: &[&str] = &["tiny", "base", "small", "medium", "large"];

/// STT transcription result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SttResponse {
    pub text: String,
    pub duration_ms: Option<u64>,
}

/// Events emitted during a listen session
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SttListenEvent {
    pub listen_id: String,
    pub event_type: String, // "speech_start" | "vad_level" | "transcribing"
    pub rms_level: Option<f32>,
}

pub fn validate_stt_model(model: &str) -> Result<(), String> {
    if VALID_STT_MODELS.contains(&model) {
        Ok(())
    } else {
        Err(format!("invalid STT model '{}'; valid models: {}", model, VALID_STT_MODELS.join(", ")))
    }
}

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
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).insert(id.to_string(), token.clone());
        token
    }

    pub fn cancel(&self, id: &str) {
        if let Some(token) = self.inner.lock().unwrap_or_else(|e| e.into_inner()).get(id) {
            token.store(true, Ordering::Relaxed);
        }
    }

    pub fn remove(&self, id: &str) {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).remove(id);
    }
}

// ---------------------------------------------------------------------------
// RMS computation on S16_LE samples
// ---------------------------------------------------------------------------
fn compute_rms(samples: &[i16]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f64 = samples.iter().map(|&s| {
        let v = s as f64 / 32768.0;
        v * v
    }).sum();
    (sum / samples.len() as f64).sqrt() as f32
}

// ---------------------------------------------------------------------------
// Write a minimal WAV header for 16-bit mono PCM
// ---------------------------------------------------------------------------
pub fn write_wav(samples: &[i16], sample_rate: u32) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let file_len = 36 + data_len;
    let mut buf = Vec::with_capacity(44 + data_len as usize);

    // RIFF header
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&file_len.to_le_bytes());
    buf.extend_from_slice(b"WAVE");

    // fmt chunk
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes()); // chunk size
    buf.extend_from_slice(&1u16.to_le_bytes()); // PCM
    buf.extend_from_slice(&1u16.to_le_bytes()); // mono
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
    buf.extend_from_slice(&2u16.to_le_bytes()); // block align
    buf.extend_from_slice(&16u16.to_le_bytes()); // bits per sample

    // data chunk
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_len.to_le_bytes());
    for &sample in samples {
        buf.extend_from_slice(&sample.to_le_bytes());
    }

    buf
}

// ---------------------------------------------------------------------------
// Transcribe WAV bytes via local Whisper server (using reqwest multipart)
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

    let body = resp.text().await.map_err(|e| format!("response read error: {e}"))?;
    let value: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| format!("invalid JSON: {e}; raw={body}"))?;

    let raw_text = value.get("text").and_then(|v| v.as_str()).unwrap_or("");
    // Filter out Whisper hallucination markers
    let text = raw_text.replace("[BLANK_AUDIO]", "").trim().to_string();
    let duration_ms = value.get("duration_ms").and_then(|v| v.as_u64());

    Ok((text, duration_ms))
}

// ---------------------------------------------------------------------------
// One-shot capture + transcribe (no VAD, fixed duration)
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
        return Ok(SttResponse { text: String::new(), duration_ms: None });
    }

    let wav = write_wav(&audio_buffer, SAMPLE_RATE);
    let (text, duration_ms) = transcribe_wav(&wav, model).await?;

    let capture_ms = (audio_buffer.len() as u64 * 1000) / SAMPLE_RATE as u64;
    Ok(SttResponse { text, duration_ms: duration_ms.or(Some(capture_ms)) })
}

// ---------------------------------------------------------------------------
// VAD listen loop — capture with voice activity detection, transcribe on silence
// ---------------------------------------------------------------------------

/// Listen with VAD, calling `on_event` for progress, and return the transcription.
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
        // Check cancellation
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelled".to_string());
        }

        // Read one chunk (100 ms of audio)
        let samples = match tokio::time::timeout(
            std::time::Duration::from_millis(500),
            capture.read_chunk(),
        ).await {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                if audio_buffer.is_empty() {
                    return Err(format!("audio capture error: {e}"));
                }
                break; // transcribe what we have
            }
            Err(_) => {
                return Err("audio capture read timeout".to_string());
            }
        };

        let rms = compute_rms(&samples);
        let elapsed = start.elapsed();
        chunk_count += 1;

        // Emit periodic VAD level for visualizer
        if chunk_count % VAD_LEVEL_EMIT_INTERVAL == 0 {
            on_event(SttListenEvent {
                listen_id: listen_id.to_string(),
                event_type: "vad_level".to_string(),
                rms_level: Some(rms),
            });
        }

        if rms > SILENCE_THRESHOLD {
            // Speech detected
            silence_start = None;
            if !speech_detected {
                speech_detected = true;
                speech_start = Some(Instant::now());
            }

            // Emit speech_start once
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
            // Silence
            if speech_detected {
                audio_buffer.extend_from_slice(&samples);

                let speech_dur = speech_start.map(|s| s.elapsed().as_millis() as u64).unwrap_or(0);
                if speech_dur >= MIN_SPEECH_MS {
                    if silence_start.is_none() {
                        silence_start = Some(Instant::now());
                    }
                    let silence_dur = silence_start.map(|s| s.elapsed().as_millis() as u64).unwrap_or(0);
                    if silence_dur >= SILENCE_DURATION_MS {
                        break;
                    }
                }
            }
        }

        // No speech timeout
        if !speech_detected && elapsed.as_millis() as u64 >= NO_SPEECH_TIMEOUT_MS {
            return Ok(SttResponse {
                text: String::new(),
                duration_ms: None,
            });
        }

        // Max capture timeout
        if speech_detected && elapsed.as_millis() as u64 >= MAX_CAPTURE_MS {
            break;
        }
    }

    // capture dropped here — stops audio stream
    drop(capture);

    if audio_buffer.is_empty() {
        return Ok(SttResponse {
            text: String::new(),
            duration_ms: None,
        });
    }

    // Emit transcribing event
    on_event(SttListenEvent {
        listen_id: listen_id.to_string(),
        event_type: "transcribing".to_string(),
        rms_level: None,
    });

    // Encode as WAV and transcribe
    let wav = write_wav(&audio_buffer, SAMPLE_RATE);
    let (text, duration_ms) = transcribe_wav(&wav, model).await?;

    let capture_ms = (audio_buffer.len() as u64 * 1000) / SAMPLE_RATE as u64;

    Ok(SttResponse {
        text,
        duration_ms: duration_ms.or(Some(capture_ms)),
    })
}
