//! Shared types for the nayru voice server ecosystem.
//!
//! These types are used across nayru-lib, nayru-cli, and downstream consumers
//! like raia-core. Keeping them in nayru-core means consumers can depend on
//! types without pulling in tokio, rodio, or other heavy deps.

use crate::text_prep::DEFAULT_MAX_CHUNK_LEN;
use serde::{Deserialize, Serialize};

// ─── TTS types ─────────────────────────────────────────────────────────────

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
            kokoro_url: "http://localhost:3001".into(),
            voice: "af_heart".into(),
            speed: 1.0,
            max_chunk_len: DEFAULT_MAX_CHUNK_LEN,
        }
    }
}

/// Observable TTS state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TtsState {
    Idle,
    Converting,
    Playing,
}

/// TTS status snapshot.
#[derive(Debug, Clone, Serialize)]
pub struct TtsStatus {
    pub state: TtsState,
    pub queue_length: usize,
    pub voice: String,
}

// ─── STT types ─────────────────────────────────────────────────────────────

/// STT transcription result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SttResponse {
    pub text: String,
    pub duration_ms: Option<u64>,
}

/// Events emitted during a listen session.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SttListenEvent {
    pub listen_id: String,
    pub event_type: String, // "speech_start" | "vad_level" | "transcribing"
    pub rms_level: Option<f32>,
}

// ─── Download types ────────────────────────────────────────────────────────

/// Model file definition.
pub struct ModelInfo {
    pub name: &'static str,
    pub filename: &'static str,
    pub url: &'static str,
    pub expected_size: u64,
}

pub const WHISPER_MODEL: ModelInfo = ModelInfo {
    name: "whisper",
    filename: "ggml-base.en-q5_1.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en-q5_1.bin",
    expected_size: 57_000_000,
};

pub const KOKORO_MODEL: ModelInfo = ModelInfo {
    name: "kokoro",
    filename: "kokoro-v1.0.onnx",
    url: "https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0/kokoro-v1.0.onnx",
    expected_size: 326_000_000,
};

pub const KOKORO_VOICES: ModelInfo = ModelInfo {
    name: "kokoro-voices",
    filename: "voices-v1.0.bin",
    url: "https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0/voices-v1.0.bin",
    expected_size: 5_200_000,
};

/// Download progress payload.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub model: String,
    pub percent: f32,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub status: String, // "downloading" | "complete" | "error"
}

// ─── Server startup event ─────────────────────────────────────────────────

/// Event emitted to the frontend during the Kokoro server startup sequence.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStartupEvent {
    /// "checking" | "installing" | "downloading" | "starting" | "ready" | "error"
    pub phase: String,
    pub message: String,
    pub progress: Option<f32>,
}

// ─── Service types ─────────────────────────────────────────────────────────

/// Status of a single voice service (whisper or kokoro).
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceStatus {
    pub model_downloaded: bool,
    pub running: bool,
    pub port: u16,
}

/// Combined status of all voice services.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceServicesStatus {
    pub whisper: ServiceStatus,
    pub kokoro: ServiceStatus,
}
