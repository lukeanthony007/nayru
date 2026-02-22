//! TTS engine — text queue → Kokoro fetch → rodio playback.
//!
//! Three-task actor pipeline connected by tokio channels:
//!
//! ```text
//! speak("text") → [cmd_tx] → Task 1: clean/split/merge → [fetch_tx cap=4]
//!     → Task 2: POST Kokoro /v1/audio/speech → [play_tx cap=2]
//!     → Task 3: rodio Sink playback (dedicated OS thread)
//! ```
//!
//! Epoch-based cancellation: `stop()` bumps an [`AtomicU64`] so all in-flight
//! work for the previous epoch is silently discarded.

use std::io::Cursor;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use rodio::{Decoder, OutputStream, Sink};
use tokio::sync::{mpsc, watch};
use tracing::{debug, error};

use crate::text_prep::{clean_text_for_tts, split_text, DEFAULT_MAX_CHUNK_LEN};

// ─── Public types ──────────────────────────────────────────────────────────

/// TTS engine configuration.
#[derive(Debug, Clone)]
pub struct TtsConfig {
    pub kokoro_url: String,
    pub voice: String,
    pub speed: f32,
    pub max_chunk_len: usize,
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            kokoro_url: "http://localhost:8880".into(),
            voice: "af_jadzia".into(),
            speed: 1.0,
            max_chunk_len: DEFAULT_MAX_CHUNK_LEN,
        }
    }
}

/// Observable TTS state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TtsState {
    Idle,
    Converting,
    Playing,
}

/// Status snapshot.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TtsStatus {
    pub state: TtsState,
    pub queue_length: usize,
    pub voice: String,
}

/// Cloneable handle to the TTS engine. All methods are non-blocking.
#[derive(Clone)]
pub struct TtsEngine {
    cmd_tx: mpsc::UnboundedSender<Cmd>,
    play_cmd_tx: std::sync::mpsc::Sender<PlayCmd>,
    status_rx: watch::Receiver<TtsStatus>,
    epoch: Arc<AtomicU64>,
}

// ─── Internal types ────────────────────────────────────────────────────────

enum Cmd {
    Speak(String),
    Stop,
}

/// Message from text-processor to fetcher.
struct FetchJob {
    text: String,
    epoch: u64,
}

/// Message from fetcher to playback thread.
struct PlayJob {
    wav_bytes: Vec<u8>,
    epoch: u64,
}

/// Commands sent to the dedicated playback OS thread.
enum PlayCmd {
    Play(PlayJob),
    Skip,
    Stop,
    Pause,
    Resume,
    Shutdown,
}

// ─── Engine construction ───────────────────────────────────────────────────

impl TtsEngine {
    /// Spawn the TTS pipeline. Returns a cloneable handle.
    pub fn new(config: TtsConfig) -> Self {
        let epoch = Arc::new(AtomicU64::new(0));
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (status_tx, status_rx) = watch::channel(TtsStatus {
            state: TtsState::Idle,
            queue_length: 0,
            voice: config.voice.clone(),
        });

        // Channels between pipeline stages
        let (fetch_tx, fetch_rx) = mpsc::channel::<FetchJob>(4);
        let (play_tx, play_rx) = mpsc::channel::<PlayJob>(2);

        // Playback OS thread (rodio OutputStream is !Send)
        let (play_cmd_tx, play_cmd_rx) = std::sync::mpsc::channel::<PlayCmd>();
        let play_epoch = epoch.clone();
        let play_status_tx = status_tx.clone();
        let play_voice = config.voice.clone();
        std::thread::Builder::new()
            .name("nayru-playback".into())
            .spawn(move || {
                playback_thread(play_cmd_rx, play_epoch, play_status_tx, play_voice);
            })
            .expect("failed to spawn playback thread");

        // Task 3 bridge: forward PlayJob from async channel to sync channel
        let bridge_play_cmd_tx = play_cmd_tx.clone();
        tokio::spawn(async move {
            play_bridge(play_rx, bridge_play_cmd_tx).await;
        });

        // Task 2: Kokoro fetcher
        let fetcher_epoch = epoch.clone();
        let fetcher_status_tx = status_tx.clone();
        let kokoro_url = config.kokoro_url.clone();
        let voice = config.voice.clone();
        let speed = config.speed;
        tokio::spawn(async move {
            fetcher_task(
                fetch_rx,
                play_tx,
                fetcher_epoch,
                fetcher_status_tx,
                &kokoro_url,
                &voice,
                speed,
            )
            .await;
        });

        // Task 1: text processor + command handler
        let proc_epoch = epoch.clone();
        tokio::spawn(async move {
            text_processor_task(cmd_rx, fetch_tx, proc_epoch, status_tx, config).await;
        });

        Self {
            cmd_tx,
            play_cmd_tx,
            status_rx,
            epoch,
        }
    }

    // ─── Public API ────────────────────────────────────────────────────

    /// Queue text for speech. Returns the estimated number of chunks.
    pub fn speak(&self, text: &str) -> usize {
        let cleaned = clean_text_for_tts(text);
        if cleaned.len() < 2 || !cleaned.chars().any(|c| c.is_alphanumeric()) {
            return 0;
        }
        let n = split_text(&cleaned, DEFAULT_MAX_CHUNK_LEN).len();
        let _ = self.cmd_tx.send(Cmd::Speak(cleaned));
        n
    }

    /// Stop all speech immediately (clear queue + stop playback).
    pub fn stop(&self) {
        self.epoch.fetch_add(1, Ordering::SeqCst);
        let _ = self.cmd_tx.send(Cmd::Stop);
        let _ = self.play_cmd_tx.send(PlayCmd::Stop);
    }

    /// Skip the currently playing clip.
    pub fn skip(&self) {
        let _ = self.play_cmd_tx.send(PlayCmd::Skip);
    }

    /// Pause playback.
    pub fn pause(&self) {
        let _ = self.play_cmd_tx.send(PlayCmd::Pause);
    }

    /// Resume playback.
    pub fn resume(&self) {
        let _ = self.play_cmd_tx.send(PlayCmd::Resume);
    }

    /// Get current status.
    pub fn status(&self) -> TtsStatus {
        self.status_rx.borrow().clone()
    }

    /// Subscribe to status changes.
    pub fn subscribe_status(&self) -> watch::Receiver<TtsStatus> {
        self.status_rx.clone()
    }
}

// ─── Task 1: Text processor + command router ───────────────────────────────

async fn text_processor_task(
    mut cmd_rx: mpsc::UnboundedReceiver<Cmd>,
    fetch_tx: mpsc::Sender<FetchJob>,
    epoch: Arc<AtomicU64>,
    status_tx: watch::Sender<TtsStatus>,
    config: TtsConfig,
) {
    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            Cmd::Speak(text) => {
                let mut chunks: Vec<String> = split_text(&text, config.max_chunk_len);
                let current_epoch = epoch.load(Ordering::SeqCst);

                // Merge small chunks into ~max_chunk_len batches for fewer API calls
                let mut batched = Vec::new();
                while !chunks.is_empty() {
                    let mut merged = chunks.remove(0);
                    while !chunks.is_empty()
                        && merged.len() + 1 + chunks[0].len() <= config.max_chunk_len
                    {
                        merged.push(' ');
                        merged.push_str(&chunks.remove(0));
                    }
                    batched.push(merged);
                }

                for (i, text) in batched.into_iter().enumerate() {
                    update_status(&status_tx, |s| {
                        s.queue_length = s.queue_length.saturating_sub(1).max(i);
                        if s.state == TtsState::Idle {
                            s.state = TtsState::Converting;
                        }
                    });

                    if fetch_tx
                        .send(FetchJob {
                            text,
                            epoch: current_epoch,
                        })
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            }
            Cmd::Stop => {
                update_status(&status_tx, |s| {
                    s.queue_length = 0;
                    s.state = TtsState::Idle;
                });
            }
        }
    }
}

// ─── Task 2: Kokoro fetcher ────────────────────────────────────────────────

async fn fetcher_task(
    mut fetch_rx: mpsc::Receiver<FetchJob>,
    play_tx: mpsc::Sender<PlayJob>,
    epoch: Arc<AtomicU64>,
    status_tx: watch::Sender<TtsStatus>,
    kokoro_url: &str,
    voice: &str,
    speed: f32,
) {
    let client = reqwest::Client::new();
    let url = format!("{kokoro_url}/v1/audio/speech");

    while let Some(job) = fetch_rx.recv().await {
        if job.epoch != epoch.load(Ordering::SeqCst) {
            debug!("fetcher: discarding stale job (epoch {})", job.epoch);
            continue;
        }

        update_status(&status_tx, |s| {
            if s.state == TtsState::Idle {
                s.state = TtsState::Converting;
            }
        });

        let body = serde_json::json!({
            "input": job.text,
            "voice": voice,
            "model": "kokoro",
            "response_format": "wav",
            "speed": speed,
        });

        match client.post(&url).json(&body).send().await {
            Ok(resp) if resp.status().is_success() => {
                if job.epoch != epoch.load(Ordering::SeqCst) {
                    debug!("fetcher: discarding stale response (epoch {})", job.epoch);
                    continue;
                }

                match resp.bytes().await {
                    Ok(bytes) => {
                        if play_tx
                            .send(PlayJob {
                                wav_bytes: bytes.to_vec(),
                                epoch: job.epoch,
                            })
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(e) => error!("fetcher: failed to read response body: {e}"),
                }
            }
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                error!("fetcher: Kokoro returned {status}: {text}");
            }
            Err(e) => {
                error!("fetcher: request failed: {e}");
            }
        }
    }
}

// ─── Task 3 bridge: async → sync ──────────────────────────────────────────

async fn play_bridge(
    mut play_rx: mpsc::Receiver<PlayJob>,
    play_cmd_tx: std::sync::mpsc::Sender<PlayCmd>,
) {
    while let Some(job) = play_rx.recv().await {
        if play_cmd_tx.send(PlayCmd::Play(job)).is_err() {
            break;
        }
    }
    let _ = play_cmd_tx.send(PlayCmd::Shutdown);
}

// ─── Playback OS thread ───────────────────────────────────────────────────

fn playback_thread(
    cmd_rx: std::sync::mpsc::Receiver<PlayCmd>,
    epoch: Arc<AtomicU64>,
    status_tx: watch::Sender<TtsStatus>,
    _voice: String,
) {
    // rodio OutputStream is !Send — must stay on this thread
    let (_stream, stream_handle) = match OutputStream::try_default() {
        Ok(pair) => pair,
        Err(e) => {
            error!("playback: failed to open audio output: {e}");
            return;
        }
    };

    let mut sink = Sink::try_new(&stream_handle).expect("failed to create sink");

    loop {
        // If sink just became empty, transition to idle
        if sink.empty() {
            update_status(&status_tx, |s| {
                if s.state == TtsState::Playing {
                    s.state = TtsState::Idle;
                }
            });
        }

        match cmd_rx.recv() {
            Ok(PlayCmd::Play(job)) => {
                if job.epoch != epoch.load(Ordering::SeqCst) {
                    debug!("playback: discarding stale audio (epoch {})", job.epoch);
                    continue;
                }

                let wav = fix_wav_sizes(job.wav_bytes);
                match Decoder::new(Cursor::new(wav)) {
                    Ok(source) => {
                        sink.append(source);
                        update_status(&status_tx, |s| s.state = TtsState::Playing);
                    }
                    Err(e) => {
                        error!("playback: failed to decode WAV: {e}");
                    }
                }
            }
            Ok(PlayCmd::Skip) => {
                sink.skip_one();
                if sink.empty() {
                    update_status(&status_tx, |s| s.state = TtsState::Idle);
                }
            }
            Ok(PlayCmd::Stop) => {
                sink.stop();
                sink = Sink::try_new(&stream_handle).expect("failed to create sink");
                update_status(&status_tx, |s| s.state = TtsState::Idle);
            }
            Ok(PlayCmd::Pause) => {
                sink.pause();
            }
            Ok(PlayCmd::Resume) => {
                sink.play();
            }
            Ok(PlayCmd::Shutdown) | Err(_) => {
                sink.stop();
                break;
            }
        }
    }
}

// ─── Helpers ───────────────────────────────────────────────────────────────

/// Fix WAV files with indeterminate sizes (0xFFFFFFFF).
///
/// Kokoro streams WAV with chunked transfer encoding, writing `0xFFFFFFFF`
/// for the RIFF chunk size (bytes 4..8) and `data` chunk size. Since we've
/// buffered the full response, we can compute the real sizes.
fn fix_wav_sizes(mut wav: Vec<u8>) -> Vec<u8> {
    if wav.len() < 44 {
        return wav;
    }
    // Check for RIFF header
    if &wav[0..4] != b"RIFF" {
        return wav;
    }
    // Patch RIFF chunk size: total_len - 8
    let riff_size = (wav.len() - 8) as u32;
    wav[4..8].copy_from_slice(&riff_size.to_le_bytes());

    // Find the "data" sub-chunk and patch its size
    let mut pos = 12; // skip "RIFF" + size + "WAVE"
    while pos + 8 <= wav.len() {
        let chunk_id = &wav[pos..pos + 4];
        if chunk_id == b"data" {
            let data_size = (wav.len() - pos - 8) as u32;
            wav[pos + 4..pos + 8].copy_from_slice(&data_size.to_le_bytes());
            break;
        }
        // Skip to next chunk
        let chunk_size =
            u32::from_le_bytes([wav[pos + 4], wav[pos + 5], wav[pos + 6], wav[pos + 7]]);
        // Guard against bogus size (0xFFFFFFFF) in non-data chunks
        let skip = if chunk_size == 0xFFFFFFFF { 0 } else { chunk_size as usize };
        pos += 8 + skip;
    }

    wav
}

fn update_status(tx: &watch::Sender<TtsStatus>, f: impl FnOnce(&mut TtsStatus)) {
    tx.send_modify(f);
}
