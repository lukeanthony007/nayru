//! Application state — TtsEngine, SentenceTracker, and config.
//!
//! The engine is lazily initialized via `OnceLock` because `TtsEngine::new()`
//! spawns tokio tasks, which requires the async runtime to already be running.
//! First access happens from a Tauri async command, guaranteeing a runtime.

use std::sync::{Mutex, OnceLock, RwLock};

use nayru_core::types::TtsConfig;
use nayru_lib::manager::VoiceServiceManager;
use nayru_lib::tts::TtsEngine;

use crate::tracker::SentenceTracker;

pub struct AppState {
    engine: OnceLock<RwLock<TtsEngine>>,
    pub tracker: Mutex<SentenceTracker>,
    pub config: RwLock<ReaderConfig>,
    pub service_manager: VoiceServiceManager,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReaderConfig {
    pub kokoro_url: String,
    pub voice: String,
    pub speed: f32,
}

impl Default for ReaderConfig {
    fn default() -> Self {
        Self {
            kokoro_url: "http://localhost:3001".into(),
            voice: "af_heart".into(),
            speed: 1.0,
        }
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            engine: OnceLock::new(),
            tracker: Mutex::new(SentenceTracker::empty()),
            config: RwLock::new(ReaderConfig::default()),
            service_manager: VoiceServiceManager::default(),
        }
    }

    /// Get or lazily create the TTS engine. Must be called from async context.
    pub fn engine(&self) -> &RwLock<TtsEngine> {
        self.engine.get_or_init(|| {
            tracing::info!("engine init: creating TtsEngine");
            let t0 = std::time::Instant::now();
            let config = self.config.read().unwrap();
            let engine = TtsEngine::new(TtsConfig {
                kokoro_url: config.kokoro_url.clone(),
                voice: config.voice.clone(),
                speed: config.speed,
                ..Default::default()
            });
            tracing::info!("engine init: done in {:?}", t0.elapsed());
            RwLock::new(engine)
        })
    }

    /// Replace the engine (used when config changes).
    pub fn replace_engine(&self, engine: TtsEngine) {
        if let Some(lock) = self.engine.get() {
            *lock.write().unwrap() = engine;
        }
    }
}
