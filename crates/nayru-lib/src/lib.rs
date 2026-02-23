//! nayru-lib — Voice server engine.
//!
//! TTS playback, STT capture, model download, service lifecycle, and HTTP API.
//! Depends on nayru-core for pure types and text processing.

pub mod capture;
pub mod download;
pub mod manager;
pub mod server;
pub mod streaming_source;
pub mod stt;
pub mod tts;

// Re-export nayru-core for convenience
pub use nayru_core;
