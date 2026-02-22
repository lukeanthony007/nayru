//! nayru — Voice server library
//!
//! Audio capture, STT transcription, TTS playback, and HTTP API.
//! No framework dependency — integration hooks use callbacks.

pub mod capture;
pub mod download;
pub mod manager;
pub mod server;
pub mod stt;
pub mod text_prep;
pub mod tts;
