//! Cross-platform audio capture using cpal.
//!
//! Provides an async-friendly `AudioCapture` struct that reads from the system
//! default microphone and delivers 16kHz mono i16 samples, regardless of the
//! device's native format/rate/channel count.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

const TARGET_SAMPLE_RATE: u32 = 16_000;

/// Chunk size returned by `read_chunk()` — 100 ms at 16 kHz mono.
pub const CHUNK_SAMPLES: usize = 1_600;

pub struct AudioCapture {
    rx: mpsc::UnboundedReceiver<Vec<i16>>,
    buf: Vec<i16>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl AudioCapture {
    /// Open the default input device and start capturing.
    pub fn new() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("No microphone found. Please connect an audio input device.")?;

        let supported = device
            .default_input_config()
            .map_err(|e| format!("Failed to get audio config: {e}"))?;

        let native_rate = supported.sample_rate().0;
        let channels = supported.channels();
        let sample_format = supported.sample_format();

        let config: cpal::StreamConfig = supported.into();

        let (tx, rx) = mpsc::unbounded_channel::<Vec<i16>>();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_clone = stop.clone();

        // cpal Stream is !Send on macOS — must live on a dedicated OS thread.
        let thread = std::thread::spawn(move || {
            let stream = match sample_format {
                SampleFormat::I16 => {
                    let tx = tx.clone();
                    let stop = stop_clone.clone();
                    device.build_input_stream(
                        &config,
                        move |data: &[i16], _: &cpal::InputCallbackInfo| {
                            if stop.load(Ordering::Relaxed) {
                                return;
                            }
                            let mono = mix_to_mono(data, channels);
                            let resampled = resample_linear(&mono, native_rate, TARGET_SAMPLE_RATE);
                            let _ = tx.send(resampled);
                        },
                        |err| eprintln!("[audio] capture error: {err}"),
                        None,
                    )
                }
                SampleFormat::F32 => {
                    let tx = tx.clone();
                    let stop = stop_clone.clone();
                    device.build_input_stream(
                        &config,
                        move |data: &[f32], _: &cpal::InputCallbackInfo| {
                            if stop.load(Ordering::Relaxed) {
                                return;
                            }
                            let i16_data: Vec<i16> = data
                                .iter()
                                .map(|&s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
                                .collect();
                            let mono = mix_to_mono(&i16_data, channels);
                            let resampled = resample_linear(&mono, native_rate, TARGET_SAMPLE_RATE);
                            let _ = tx.send(resampled);
                        },
                        |err| eprintln!("[audio] capture error: {err}"),
                        None,
                    )
                }
                other => {
                    eprintln!("[audio] unsupported sample format: {other:?}");
                    return;
                }
            };

            let stream = match stream {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[audio] failed to build stream: {e}");
                    return;
                }
            };

            if let Err(e) = stream.play() {
                eprintln!("[audio] failed to start stream: {e}");
                return;
            }

            // Park until stop signal
            loop {
                std::thread::park();
                if stop_clone.load(Ordering::Relaxed) {
                    break;
                }
            }
            // stream dropped here — stops cpal
        });

        Ok(AudioCapture {
            rx,
            buf: Vec::new(),
            stop,
            thread: Some(thread),
        })
    }

    /// Read exactly `CHUNK_SAMPLES` (1600) i16 samples.
    /// Returns an error if the capture stream ends unexpectedly.
    pub async fn read_chunk(&mut self) -> Result<Vec<i16>, String> {
        while self.buf.len() < CHUNK_SAMPLES {
            match self.rx.recv().await {
                Some(samples) => self.buf.extend_from_slice(&samples),
                None => return Err("audio capture stream ended".to_string()),
            }
        }
        let chunk = self.buf.drain(..CHUNK_SAMPLES).collect();
        Ok(chunk)
    }
}

impl Drop for AudioCapture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.thread.take() {
            handle.thread().unpark();
            let _ = handle.join();
        }
    }
}

// ---------------------------------------------------------------------------
// Audio processing helpers
// ---------------------------------------------------------------------------

/// Mix multi-channel audio to mono by averaging channels.
fn mix_to_mono(input: &[i16], channels: u16) -> Vec<i16> {
    if channels <= 1 {
        return input.to_vec();
    }
    let ch = channels as usize;
    input
        .chunks_exact(ch)
        .map(|frame| {
            let sum: i32 = frame.iter().map(|&s| s as i32).sum();
            (sum / channels as i32) as i16
        })
        .collect()
}

/// Resample using linear interpolation. Good enough for speech.
fn resample_linear(input: &[i16], from_rate: u32, to_rate: u32) -> Vec<i16> {
    if from_rate == to_rate || input.is_empty() {
        return input.to_vec();
    }
    let ratio = from_rate as f64 / to_rate as f64;
    let output_len = (input.len() as f64 / ratio) as usize;
    let mut output = Vec::with_capacity(output_len);
    for i in 0..output_len {
        let src_pos = i as f64 * ratio;
        let idx = src_pos as usize;
        let frac = src_pos - idx as f64;
        let s0 = input[idx] as f64;
        let s1 = if idx + 1 < input.len() {
            input[idx + 1] as f64
        } else {
            s0
        };
        output.push((s0 + frac * (s1 - s0)) as i16);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mix_to_mono_passthrough() {
        let input = vec![100, 200, 300];
        assert_eq!(mix_to_mono(&input, 1), vec![100, 200, 300]);
    }

    #[test]
    fn test_mix_to_mono_stereo() {
        let input = vec![100, 200, 300, 400];
        assert_eq!(mix_to_mono(&input, 2), vec![150, 350]);
    }

    #[test]
    fn test_resample_passthrough() {
        let input = vec![1, 2, 3, 4];
        assert_eq!(resample_linear(&input, 16000, 16000), vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_resample_downsample() {
        // 48kHz → 16kHz = 3:1 ratio, 9 input samples → 3 output samples
        let input: Vec<i16> = (0..9).collect();
        let output = resample_linear(&input, 48000, 16000);
        assert_eq!(output.len(), 3);
        assert_eq!(output[0], 0);
        assert_eq!(output[1], 3);
        assert_eq!(output[2], 6);
    }

    #[test]
    fn test_resample_empty() {
        assert_eq!(resample_linear(&[], 48000, 16000), Vec::<i16>::new());
    }
}
