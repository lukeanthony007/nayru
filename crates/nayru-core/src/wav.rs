//! WAV encoding and audio math utilities.
//!
//! Pure functions — no I/O, no async runtime.

/// Default sample rate for STT capture (16 kHz mono).
pub const SAMPLE_RATE: u32 = 16_000;

/// Valid Whisper model names.
const VALID_STT_MODELS: &[&str] = &["tiny", "base", "small", "medium", "large"];

/// Validate a Whisper STT model name.
pub fn validate_stt_model(model: &str) -> Result<(), String> {
    if VALID_STT_MODELS.contains(&model) {
        Ok(())
    } else {
        Err(format!(
            "invalid STT model '{}'; valid models: {}",
            model,
            VALID_STT_MODELS.join(", ")
        ))
    }
}

/// Compute RMS level of 16-bit PCM samples, normalized to 0.0–1.0.
pub fn compute_rms(samples: &[i16]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f64 = samples
        .iter()
        .map(|&s| {
            let v = s as f64 / 32768.0;
            v * v
        })
        .sum();
    (sum / samples.len() as f64).sqrt() as f32
}

/// Write a minimal WAV file (16-bit mono PCM) from raw samples.
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

/// Parsed WAV header fields needed for streaming playback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WavHeader {
    pub channels: u16,
    pub sample_rate: u32,
    pub bits_per_sample: u16,
    /// Byte offset in the buffer where raw PCM data begins.
    pub data_offset: usize,
}

/// Parse a WAV header from a byte buffer.
///
/// Returns the audio format parameters and the byte offset where PCM data
/// starts.  Handles Kokoro's `0xFFFFFFFF` sentinel sizes by ignoring them
/// (we're streaming, so total size is unknown anyway).
pub fn parse_wav_header(buf: &[u8]) -> Result<WavHeader, &'static str> {
    if buf.len() < 12 {
        return Err("too short for RIFF header");
    }
    if &buf[0..4] != b"RIFF" {
        return Err("missing RIFF tag");
    }
    if &buf[8..12] != b"WAVE" {
        return Err("missing WAVE tag");
    }

    let mut pos = 12;
    let mut channels: Option<u16> = None;
    let mut sample_rate: Option<u32> = None;
    let mut bits_per_sample: Option<u16> = None;

    while pos + 8 <= buf.len() {
        let chunk_id = &buf[pos..pos + 4];
        let chunk_size =
            u32::from_le_bytes([buf[pos + 4], buf[pos + 5], buf[pos + 6], buf[pos + 7]]);

        if chunk_id == b"fmt " {
            if pos + 24 > buf.len() {
                return Err("fmt chunk truncated");
            }
            let audio_format = u16::from_le_bytes([buf[pos + 8], buf[pos + 9]]);
            if audio_format != 1 {
                return Err("not PCM format");
            }
            channels = Some(u16::from_le_bytes([buf[pos + 10], buf[pos + 11]]));
            sample_rate = Some(u32::from_le_bytes([
                buf[pos + 12],
                buf[pos + 13],
                buf[pos + 14],
                buf[pos + 15],
            ]));
            bits_per_sample = Some(u16::from_le_bytes([buf[pos + 22], buf[pos + 23]]));

            let skip = if chunk_size == 0xFFFFFFFF {
                16 // standard fmt chunk payload
            } else {
                chunk_size as usize
            };
            pos += 8 + skip;
            continue;
        }

        if chunk_id == b"data" {
            let ch = channels.ok_or("data chunk before fmt chunk")?;
            let sr = sample_rate.ok_or("data chunk before fmt chunk")?;
            let bps = bits_per_sample.ok_or("data chunk before fmt chunk")?;
            return Ok(WavHeader {
                channels: ch,
                sample_rate: sr,
                bits_per_sample: bps,
                data_offset: pos + 8,
            });
        }

        // Skip unknown chunks
        let skip = if chunk_size == 0xFFFFFFFF {
            0
        } else {
            chunk_size as usize
        };
        pos += 8 + skip;
    }

    Err("data chunk not found")
}

/// Fix WAV files with indeterminate sizes (0xFFFFFFFF).
///
/// Kokoro streams WAV with chunked transfer encoding, writing `0xFFFFFFFF`
/// for the RIFF chunk size (bytes 4..8) and `data` chunk size. Since we've
/// buffered the full response, we can compute the real sizes.
pub fn fix_wav_sizes(mut wav: Vec<u8>) -> Vec<u8> {
    if wav.len() < 44 {
        return wav;
    }
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
        let chunk_size =
            u32::from_le_bytes([wav[pos + 4], wav[pos + 5], wav[pos + 6], wav[pos + 7]]);
        let skip = if chunk_size == 0xFFFFFFFF {
            0
        } else {
            chunk_size as usize
        };
        pos += 8 + skip;
    }

    wav
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_wav_produces_valid_header() {
        let samples = vec![0i16; 100];
        let wav = write_wav(&samples, 16000);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(wav.len(), 44 + 200); // 44 header + 100 samples * 2 bytes
    }

    #[test]
    fn compute_rms_silence() {
        let samples = vec![0i16; 1000];
        assert_eq!(compute_rms(&samples), 0.0);
    }

    #[test]
    fn compute_rms_nonzero() {
        let samples = vec![16384i16; 100]; // ~0.5 normalized
        let rms = compute_rms(&samples);
        assert!(rms > 0.4 && rms < 0.6, "rms={rms}");
    }

    #[test]
    fn compute_rms_empty() {
        assert_eq!(compute_rms(&[]), 0.0);
    }

    #[test]
    fn validate_stt_model_valid() {
        assert!(validate_stt_model("tiny").is_ok());
        assert!(validate_stt_model("base").is_ok());
        assert!(validate_stt_model("large").is_ok());
    }

    #[test]
    fn validate_stt_model_invalid() {
        assert!(validate_stt_model("huge").is_err());
        assert!(validate_stt_model("").is_err());
    }

    #[test]
    fn fix_wav_sizes_patches_sentinel() {
        let mut wav = write_wav(&vec![0i16; 50], 16000);
        // Corrupt sizes to simulate Kokoro streaming
        wav[4..8].copy_from_slice(&0xFFFFFFFFu32.to_le_bytes());
        let fixed = fix_wav_sizes(wav.clone());
        let riff_size = u32::from_le_bytes([fixed[4], fixed[5], fixed[6], fixed[7]]);
        assert_eq!(riff_size, (fixed.len() - 8) as u32);
    }

    #[test]
    fn fix_wav_sizes_noop_on_good_wav() {
        let wav = write_wav(&vec![0i16; 50], 16000);
        let fixed = fix_wav_sizes(wav.clone());
        assert_eq!(wav, fixed);
    }

    #[test]
    fn parse_wav_header_basic() {
        let wav = write_wav(&vec![0i16; 50], 24000);
        let hdr = parse_wav_header(&wav).unwrap();
        assert_eq!(hdr.channels, 1);
        assert_eq!(hdr.sample_rate, 24000);
        assert_eq!(hdr.bits_per_sample, 16);
        assert_eq!(hdr.data_offset, 44);
    }

    #[test]
    fn parse_wav_header_sentinel_sizes() {
        let mut wav = write_wav(&vec![0i16; 50], 24000);
        // Simulate Kokoro: sentinel RIFF and data sizes
        wav[4..8].copy_from_slice(&0xFFFFFFFFu32.to_le_bytes());
        // data chunk size at offset 40
        wav[40..44].copy_from_slice(&0xFFFFFFFFu32.to_le_bytes());
        let hdr = parse_wav_header(&wav).unwrap();
        assert_eq!(hdr.channels, 1);
        assert_eq!(hdr.sample_rate, 24000);
        assert_eq!(hdr.data_offset, 44);
    }

    #[test]
    fn parse_wav_header_too_short() {
        assert!(parse_wav_header(b"RIFF").is_err());
    }

    #[test]
    fn parse_wav_header_not_riff() {
        let mut wav = write_wav(&vec![0i16; 10], 16000);
        wav[0..4].copy_from_slice(b"NOPE");
        assert!(parse_wav_header(&wav).is_err());
    }
}
