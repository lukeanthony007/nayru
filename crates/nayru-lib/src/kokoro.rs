//! Direct Kokoro ONNX inference with espeak-ng phonemization.
//!
//! Bypasses kokoro-tts's built-in minimal espeak dictionary (which produces
//! low-quality English phonemes) by calling system espeak-ng for IPA conversion,
//! then running the ONNX model directly via `ort`.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use kokoro_tts::get_token_ids;
use ndarray::Array;
use ort::inputs;
use ort::session::{RunOptions, Session};
use ort::value::TensorRef;
use tokio::sync::Mutex;
use tracing::{debug, warn};

/// Voice style pack: indexed by [sequence_length][style_index][feature_dim].
type VoicePack = Vec<Vec<Vec<f32>>>;

/// In-process Kokoro synthesizer with espeak-ng phonemization.
pub struct KokoroSynth {
    model: Arc<Mutex<Session>>,
    voices: Arc<HashMap<String, VoicePack>>,
}

impl KokoroSynth {
    /// Load the ONNX model and voices file.
    pub async fn new(model_path: &Path, voices_path: &Path) -> Result<Self, String> {
        let voices_data = tokio::fs::read(voices_path)
            .await
            .map_err(|e| format!("failed to read voices: {e}"))?;

        let (voices, _): (HashMap<String, VoicePack>, _) =
            bincode::decode_from_slice(&voices_data, bincode::config::standard())
                .map_err(|e| format!("failed to decode voices: {e}"))?;

        let model = Session::builder()
            .map_err(|e| format!("ort session builder: {e}"))?
            .commit_from_file(model_path)
            .map_err(|e| format!("ort load model: {e}"))?;

        Ok(Self {
            model: Arc::new(Mutex::new(model)),
            voices: Arc::new(voices),
        })
    }

    /// Synthesize text to f32 audio samples at 24kHz.
    pub async fn synth(&self, text: &str, voice_name: &str, speed: f32) -> Result<(Vec<f32>, Duration), String> {
        let ipa = text_to_ipa(text).await?;
        debug!("phonemes: {}", ipa);

        let pack = self.voices.get(voice_name)
            .ok_or_else(|| format!("voice '{}' not found in voices.bin", voice_name))?;

        let tokens = get_token_ids(&ipa, false);
        let seq_len = tokens.len();
        let phonemes = Array::from_shape_vec((1, seq_len), tokens)
            .map_err(|e| format!("ndarray shape: {e}"))?;

        // Voice style: pick the style vector for this sequence length
        let style_idx = (seq_len - 1).min(pack.len() - 1);
        let ref_s = pack[style_idx]
            .first()
            .cloned()
            .unwrap_or_default();

        let style = Array::from_shape_vec((1, ref_s.len()), ref_s)
            .map_err(|e| format!("style shape: {e}"))?;

        let speed_arr = Array::from_vec(vec![speed]);
        let options = RunOptions::new()
            .map_err(|e| format!("run options: {e}"))?;

        let tokens_tensor = TensorRef::from_array_view(&phonemes)
            .map_err(|e| format!("tokens tensor: {e}"))?;
        let style_tensor = TensorRef::from_array_view(&style)
            .map_err(|e| format!("style tensor: {e}"))?;
        let speed_tensor = TensorRef::from_array_view(&speed_arr)
            .map_err(|e| format!("speed tensor: {e}"))?;

        let mut model = self.model.lock().await;
        let t = SystemTime::now();

        let output = model
            .run_async(
                inputs![
                    "tokens" => tokens_tensor,
                    "style" => style_tensor,
                    "speed" => speed_tensor,
                ],
                &options,
            )
            .map_err(|e| format!("inference: {e}"))?
            .await
            .map_err(|e| format!("inference await: {e}"))?;

        let elapsed = t.elapsed().unwrap_or_default();
        let (_, audio) = output["audio"]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("extract audio: {e}"))?;

        Ok((audio.to_owned(), elapsed))
    }
}

/// Convert text to IPA using system espeak-ng.
async fn text_to_ipa(text: &str) -> Result<String, String> {
    let output = tokio::process::Command::new("espeak-ng")
        .args(["--ipa", "-q", "-v", "en-us", text])
        .output()
        .await
        .map_err(|e| format!("espeak-ng failed to execute: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!("espeak-ng stderr: {}", stderr);
    }

    // Replace newlines with spaces (espeak-ng outputs one line per clause)
    let ipa = String::from_utf8_lossy(&output.stdout)
        .replace('\n', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    if ipa.is_empty() {
        return Err("espeak-ng returned empty output".to_string());
    }

    Ok(ipa)
}
