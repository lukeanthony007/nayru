//! TTS engine — text queue → pipelined Kokoro fetch → rodio playback.
//!
//! Pipeline:
//!
//! ```text
//! speak("text") → [cmd_tx] → text_processor: split sentences
//!     → [fetch_tx] → fetcher_0: POST Kokoro, stream PCM, create source on first data
//!     → [fetch_tx] → fetcher_1: (prefetch) POST Kokoro concurrently
//!     → playback thread: gapless sequential playback
//! ```
//!
//! Two fetcher tasks consume from a shared job channel. While fetcher_0 streams
//! the current sentence to the sink, fetcher_1 pre-fetches the next sentence from
//! Kokoro. This overlaps synthesis with playback — by the time sentence 1 finishes
//! playing, sentence 2 is usually ready or nearly ready.
//!
//! Sentences are dispatched individually (no merging) to minimize time-to-first-audio.
//! Kokoro's internal smart_split handles its own chunking.
//!
//! Epoch-based cancellation: `stop()` bumps an [`AtomicU64`] so all in-flight
//! work for the previous epoch is silently discarded.
//!
//! **Streaming API:** For LLM streaming, use `stream_chunk()` / `stream_end()`
//! instead of `speak()`. The text_processor accumulates chunks, extracts complete
//! sentences as they arrive, and dispatches them through the same fetch pipeline —
//! one continuous epoch, gapless playback.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use futures_util::StreamExt;
use rodio::{OutputStream, Sink};
use tokio::sync::{mpsc, watch};
use tracing::{debug, error};

use nayru_core::text_prep::{clean_text_for_tts, split_sentences, split_text, DEFAULT_MAX_CHUNK_LEN};
use nayru_core::types::{TtsConfig, TtsState, TtsStatus};

use crate::streaming_source::{PcmChunk, StreamingSource};

/// Kokoro PCM streaming format: 24 kHz mono 16-bit signed LE.
const PCM_SAMPLE_RATE: u32 = 24_000;
const PCM_CHANNELS: u16 = 1;

/// Number of concurrent fetcher tasks.
/// 2 = one active (streaming to sink) + one pre-fetching the next chunk.
const FETCHER_COUNT: usize = 2;

/// Capacity of the fetch job channel. Must be large enough that the
/// text_processor never blocks on send — blocking would stall StreamChunk
/// processing and create gaps between clips.
const FETCH_QUEUE_CAPACITY: usize = 32;

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
    StreamChunk(String),
    StreamEnd,
    Stop,
}

struct FetchJob {
    text: String,
    epoch: u64,
}

enum PlayCmd {
    PlayStream(StreamingSource),
    Skip,
    Stop,
    Pause,
    Resume,
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

        // Job channel — bounded to FETCHER_COUNT so text_processor applies backpressure
        let (fetch_tx, fetch_rx) = mpsc::channel::<FetchJob>(FETCH_QUEUE_CAPACITY);

        // Playback OS thread (rodio OutputStream is !Send)
        let (play_cmd_tx, play_cmd_rx) = std::sync::mpsc::channel::<PlayCmd>();
        let play_status_tx = status_tx.clone();
        std::thread::Builder::new()
            .name("nayru-playback".into())
            .spawn(move || {
                playback_thread(play_cmd_rx, play_status_tx);
            })
            .expect("failed to spawn playback thread");

        // Spawn FETCHER_COUNT fetcher tasks sharing the job channel
        let fetch_rx = Arc::new(tokio::sync::Mutex::new(fetch_rx));
        for i in 0..FETCHER_COUNT {
            let fetch_rx = fetch_rx.clone();
            let epoch = epoch.clone();
            let play_cmd_tx = play_cmd_tx.clone();
            let status_tx = status_tx.clone();
            let kokoro_url = config.kokoro_url.clone();
            let voice = config.voice.clone();
            let speed = config.speed;
            tokio::spawn(async move {
                fetcher_task(i, fetch_rx, play_cmd_tx, epoch, status_tx, &kokoro_url, &voice, speed)
                    .await;
            });
        }

        // Text processor — splits, merges, and dispatches jobs
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

    /// Stop all speech immediately.
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

    /// Feed a text chunk from an LLM stream. The engine accumulates text
    /// internally, extracts complete sentences, and dispatches them through
    /// the synthesis pipeline immediately for gapless playback.
    ///
    /// Text should already be cleaned (no markdown) — the caller is responsible
    /// for preprocessing since markdown patterns span multiple chunks.
    pub fn stream_chunk(&self, text: &str) {
        if !text.is_empty() {
            let _ = self.cmd_tx.send(Cmd::StreamChunk(text.to_string()));
        }
    }

    /// Signal that the LLM stream is complete. Flushes any remaining buffered
    /// text as a final synthesis job.
    pub fn stream_end(&self) {
        let _ = self.cmd_tx.send(Cmd::StreamEnd);
    }
}

// ─── Text processor ──────────────────────────────────────────────────────

async fn text_processor_task(
    mut cmd_rx: mpsc::UnboundedReceiver<Cmd>,
    fetch_tx: mpsc::Sender<FetchJob>,
    epoch: Arc<AtomicU64>,
    status_tx: watch::Sender<TtsStatus>,
    config: TtsConfig,
) {
    // Streaming state — persists across loop iterations
    let mut stream_buffer = String::new();
    let mut stream_epoch: Option<u64> = None;

    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            Cmd::Speak(text) => {
                let current_epoch = epoch.load(Ordering::SeqCst);

                // Split into sentences, then sub-split any that exceed max_chunk_len.
                // Each sentence is dispatched individually to minimize first-audio latency.
                let sentences = split_sentences(&text);
                let mut batched: Vec<String> = Vec::new();
                for sentence in sentences {
                    if sentence.len() <= config.max_chunk_len {
                        batched.push(sentence);
                    } else {
                        batched.extend(split_text(&sentence, config.max_chunk_len));
                    }
                }

                let total = batched.len();
                update_status(&status_tx, |s| {
                    s.queue_length += total;
                    if s.state == TtsState::Idle {
                        s.state = TtsState::Converting;
                    }
                });

                debug!(
                    "processor: dispatching {} jobs (epoch {})",
                    total, current_epoch
                );

                for text in batched {
                    debug!("processor: queuing job ({} chars)", text.len());
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

            Cmd::StreamChunk(chunk) => {
                // Initialize stream epoch on first chunk
                if stream_epoch.is_none() {
                    let e = epoch.load(Ordering::SeqCst);
                    stream_epoch = Some(e);
                    debug!("stream started (epoch {})", e);
                    update_status(&status_tx, |s| {
                        if s.state == TtsState::Idle {
                            s.state = TtsState::Converting;
                        }
                    });
                }

                let current_epoch = stream_epoch.unwrap();
                if epoch.load(Ordering::SeqCst) != current_epoch {
                    // Stream was stopped — discard
                    stream_buffer.clear();
                    stream_epoch = None;
                    continue;
                }

                stream_buffer.push_str(&chunk);

                // Extract and dispatch complete sentences
                dispatch_stream_sentences(
                    &mut stream_buffer,
                    current_epoch,
                    &fetch_tx,
                    &epoch,
                    &status_tx,
                    &config,
                )
                .await;
            }

            Cmd::StreamEnd => {
                debug!("stream end — buffer={} chars", stream_buffer.len());
                if let Some(current_epoch) = stream_epoch.take() {
                    if epoch.load(Ordering::SeqCst) == current_epoch {
                        // Flush remaining buffer as final chunk(s)
                        let remaining = stream_buffer.trim().to_string();
                        if remaining.len() >= 2
                            && remaining.chars().any(|c| c.is_alphanumeric())
                        {
                            let chunks = if remaining.len() <= config.max_chunk_len {
                                vec![remaining]
                            } else {
                                split_text(&remaining, config.max_chunk_len)
                            };

                            let count = chunks.len();
                            update_status(&status_tx, |s| {
                                s.queue_length += count;
                            });

                            debug!("stream: flushing {} final chunk(s)", count);

                            for text in chunks {
                                if epoch.load(Ordering::SeqCst) != current_epoch {
                                    break;
                                }
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
                    }
                }
                stream_buffer.clear();
            }

            Cmd::Stop => {
                stream_buffer.clear();
                stream_epoch = None;
                update_status(&status_tx, |s| {
                    s.queue_length = 0;
                    s.state = TtsState::Idle;
                });
            }
        }
    }
}

/// Extract complete sentences from the stream buffer and dispatch them as FetchJobs.
/// Leaves the incomplete tail (last element from split_sentences) in the buffer.
async fn dispatch_stream_sentences(
    buffer: &mut String,
    current_epoch: u64,
    fetch_tx: &mpsc::Sender<FetchJob>,
    epoch: &Arc<AtomicU64>,
    status_tx: &watch::Sender<TtsStatus>,
    config: &TtsConfig,
) {
    let sentences = split_sentences(buffer);

    if sentences.len() <= 1 {
        // Zero or one sentence — might be incomplete. Don't dispatch yet.
        // Exception: if buffer is very long without punctuation, force-split.
        if buffer.len() >= config.max_chunk_len * 2 {
            let split_at = buffer[..config.max_chunk_len]
                .rfind(' ')
                .unwrap_or(config.max_chunk_len);
            let chunk = buffer[..split_at].trim().to_string();
            let tail = buffer[split_at..].trim_start().to_string();
            *buffer = tail;

            if chunk.len() >= 2 {
                update_status(status_tx, |s| {
                    s.queue_length += 1;
                });
                debug!("stream: force-split dispatch ({} chars)", chunk.len());
                let _ = fetch_tx
                    .send(FetchJob {
                        text: chunk,
                        epoch: current_epoch,
                    })
                    .await;
            }
        }
        return;
    }

    // All sentences except the last are complete — dispatch them.
    // Keep the last (potentially incomplete) sentence in the buffer.
    let last = sentences.last().unwrap().clone();
    let complete = &sentences[..sentences.len() - 1];

    let mut to_dispatch: Vec<String> = Vec::new();
    for sentence in complete {
        if sentence.len() <= config.max_chunk_len {
            to_dispatch.push(sentence.clone());
        } else {
            to_dispatch.extend(split_text(sentence, config.max_chunk_len));
        }
    }

    if !to_dispatch.is_empty() {
        let count = to_dispatch.len();
        update_status(status_tx, |s| {
            s.queue_length += count;
        });

        for text in &to_dispatch {
            debug!("stream: dispatch sentence ({} chars)", text.len());
        }

        for text in to_dispatch {
            if epoch.load(Ordering::SeqCst) != current_epoch {
                break;
            }
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

    // Replace buffer with the incomplete tail
    *buffer = last;
}

// ─── Fetcher task (FETCHER_COUNT instances share the job channel) ───────

async fn fetcher_task(
    worker_id: usize,
    fetch_rx: Arc<tokio::sync::Mutex<mpsc::Receiver<FetchJob>>>,
    play_cmd_tx: std::sync::mpsc::Sender<PlayCmd>,
    epoch: Arc<AtomicU64>,
    status_tx: watch::Sender<TtsStatus>,
    kokoro_url: &str,
    voice: &str,
    speed: f32,
) {
    let client = reqwest::Client::new();
    let url = format!("{kokoro_url}/v1/audio/speech");

    loop {
        // Acquire lock to take next job — only one fetcher holds the lock at a time
        let job = {
            let mut rx = fetch_rx.lock().await;
            rx.recv().await
        };

        let job = match job {
            Some(j) => j,
            None => break, // channel closed
        };

        if job.epoch != epoch.load(Ordering::SeqCst) {
            debug!("fetch[{worker_id}]: discarding stale job");
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
            "response_format": "pcm",
            "stream": true,
            "speed": speed,
        });

        debug!("fetch[{worker_id}]: POST {} chars", job.text.len());

        let resp = match client.post(&url).json(&body).send().await {
            Ok(resp) if resp.status().is_success() => resp,
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                error!("fetch[{worker_id}]: Kokoro error {status}: {text}");
                continue;
            }
            Err(e) => {
                error!("fetch[{worker_id}]: request failed: {e}");
                continue;
            }
        };

        if job.epoch != epoch.load(Ordering::SeqCst) {
            debug!("fetch[{worker_id}]: stale response, discarding");
            continue;
        }

        // Stream PCM data — create source on first chunk
        let mut stream = resp.bytes_stream();
        let mut leftover: Option<u8> = None;
        let mut pcm_tx: Option<std::sync::mpsc::Sender<PcmChunk>> = None;

        while let Some(chunk_result) = stream.next().await {
            if job.epoch != epoch.load(Ordering::SeqCst) {
                break;
            }

            let chunk = match chunk_result {
                Ok(c) => c,
                Err(e) => {
                    error!("fetch[{worker_id}]: stream error: {e}");
                    break;
                }
            };

            let (samples, lo) = bytes_to_i16(&chunk, leftover.take());
            leftover = lo;

            if pcm_tx.is_none() && !samples.is_empty() {
                let (tx, rx) = std::sync::mpsc::channel();
                let source = StreamingSource::new(rx, PCM_CHANNELS, PCM_SAMPLE_RATE);
                let _ = tx.send(PcmChunk::Data(samples));

                if play_cmd_tx.send(PlayCmd::PlayStream(source)).is_err() {
                    break;
                }
                pcm_tx = Some(tx);
                continue;
            }

            if !samples.is_empty() {
                if let Some(ref tx) = pcm_tx {
                    if tx.send(PcmChunk::Data(samples)).is_err() {
                        break;
                    }
                }
            }
        }

        if let Some(tx) = pcm_tx.take() {
            let _ = tx.send(PcmChunk::Done);
        }

        update_status(&status_tx, |s| {
            s.queue_length = s.queue_length.saturating_sub(1);
        });
    }
}

/// Convert raw bytes to i16 PCM samples (little-endian).
fn bytes_to_i16(bytes: &[u8], leftover: Option<u8>) -> (Vec<i16>, Option<u8>) {
    let mut data: Vec<u8>;
    let slice = if let Some(lo) = leftover {
        data = Vec::with_capacity(1 + bytes.len());
        data.push(lo);
        data.extend_from_slice(bytes);
        &data[..]
    } else {
        bytes
    };

    let mut samples = Vec::with_capacity(slice.len() / 2);
    for pair in slice.chunks_exact(2) {
        samples.push(i16::from_le_bytes([pair[0], pair[1]]));
    }

    let remainder = if slice.len() % 2 == 1 {
        Some(slice[slice.len() - 1])
    } else {
        None
    };

    (samples, remainder)
}

// ─── Playback OS thread ───────────────────────────────────────────────────

fn playback_thread(
    cmd_rx: std::sync::mpsc::Receiver<PlayCmd>,
    status_tx: watch::Sender<TtsStatus>,
) {
    let (_stream, stream_handle) = match OutputStream::try_default() {
        Ok(pair) => pair,
        Err(e) => {
            error!("playback: failed to open audio output: {e}");
            return;
        }
    };

    let mut sink = Sink::try_new(&stream_handle).expect("failed to create sink");

    loop {
        if sink.empty() {
            update_status(&status_tx, |s| {
                if s.state == TtsState::Playing {
                    s.state = TtsState::Idle;
                }
            });
        }

        match cmd_rx.recv() {
            Ok(PlayCmd::PlayStream(source)) => {
                debug!("playback: source appended to sink");
                sink.append(source);
                update_status(&status_tx, |s| s.state = TtsState::Playing);
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
            Err(_) => {
                sink.stop();
                break;
            }
        }
    }
}

fn update_status(tx: &watch::Sender<TtsStatus>, f: impl FnOnce(&mut TtsStatus)) {
    tx.send_modify(f);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_to_i16_basic() {
        let bytes = [0x01, 0x00, 0xFF, 0x7F]; // 1, 32767
        let (samples, lo) = bytes_to_i16(&bytes, None);
        assert_eq!(samples, vec![1, 32767]);
        assert_eq!(lo, None);
    }

    #[test]
    fn bytes_to_i16_with_leftover() {
        let bytes = [0x01, 0x00, 0xFF];
        let (samples, lo) = bytes_to_i16(&bytes, None);
        assert_eq!(samples, vec![1]);
        assert_eq!(lo, Some(0xFF));
    }

    #[test]
    fn bytes_to_i16_carry_leftover() {
        let bytes = [0x7F, 0x01, 0x00];
        let (samples, lo) = bytes_to_i16(&bytes, Some(0xFF));
        assert_eq!(samples, vec![32767, 1]);
        assert_eq!(lo, None);
    }

    #[test]
    fn bytes_to_i16_empty() {
        let (samples, lo) = bytes_to_i16(&[], None);
        assert!(samples.is_empty());
        assert_eq!(lo, None);
    }

    #[test]
    fn bytes_to_i16_single_byte() {
        let (samples, lo) = bytes_to_i16(&[0x42], None);
        assert!(samples.is_empty());
        assert_eq!(lo, Some(0x42));
    }

}
