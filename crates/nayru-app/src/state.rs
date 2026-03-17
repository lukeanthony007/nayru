//! Application state — TtsEngine, SentenceTracker, and config.
//!
//! The engine is lazily initialized via `OnceLock` because `TtsEngine::new()`
//! spawns tokio tasks, which requires the async runtime to already be running.
//! First access happens from a Tauri async command, guaranteeing a runtime.

use std::sync::{Arc, Mutex, OnceLock, RwLock};

use nayru_core::types::TtsConfig;
use nayru_lib::kokoro::KokoroSynth;
use nayru_lib::manager::VoiceServiceManager;
use nayru_lib::tts::TtsEngine;

use crate::tracker::SentenceTracker;

pub struct AppState {
    engine: OnceLock<RwLock<TtsEngine>>,
    pub kokoro: OnceLock<Arc<KokoroSynth>>,
    pub tracker: Mutex<SentenceTracker>,
    pub config: RwLock<ReaderConfig>,
    pub service_manager: VoiceServiceManager,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReaderConfig {
    pub voice: String,
    pub speed: f32,
}

impl Default for ReaderConfig {
    fn default() -> Self {
        Self {
            voice: "af_heart".into(),
            speed: 1.0,
        }
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            engine: OnceLock::new(),
            kokoro: OnceLock::new(),
            tracker: Mutex::new(SentenceTracker::empty()),
            config: RwLock::new(ReaderConfig::default()),
            service_manager: VoiceServiceManager::default(),
        }
    }

    /// Get or lazily create the TTS engine. Must be called from async context.
    /// Returns None if the kokoro model hasn't been loaded yet.
    pub fn engine(&self) -> Option<&RwLock<TtsEngine>> {
        // If engine already exists, return it
        if let Some(engine) = self.engine.get() {
            return Some(engine);
        }
        // Otherwise try to create it — need kokoro to be loaded
        let kokoro = self.kokoro.get()?.clone();
        tracing::info!("engine init: creating TtsEngine");
        let t0 = std::time::Instant::now();
        let config = self.config.read().unwrap();
        let engine = TtsEngine::new(
            TtsConfig {
                voice: config.voice.clone(),
                speed: config.speed,
                ..Default::default()
            },
            kokoro,
        );
        tracing::info!("engine init: done in {:?}", t0.elapsed());
        let _ = self.engine.set(RwLock::new(engine));
        self.engine.get()
    }

    /// Store the loaded KokoroSynth instance.
    pub fn set_kokoro(&self, kokoro: Arc<KokoroSynth>) {
        let _ = self.kokoro.set(kokoro);
    }

    /// Replace the engine (used when config changes).
    pub fn replace_engine(&self, engine: TtsEngine) {
        if let Some(lock) = self.engine.get() {
            *lock.write().unwrap() = engine;
        }
    }
}
