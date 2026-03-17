//! Tauri commands for the reader app.

use serde::{Deserialize, Serialize};
use tauri::State;

use nayru_core::text_prep::split_sentences;
use nayru_core::types::TtsConfig;
use nayru_lib::tts::TtsEngine;

use crate::state::{AppState, ReaderConfig};
use crate::tracker::SentenceTracker;

#[derive(Debug, Clone, Serialize)]
pub struct ReaderStatus {
    pub state: String,
    pub current_sentence_index: Option<usize>,
    pub total_sentences: usize,
    pub voice: String,
    pub speed: f32,
}

#[derive(Debug, Deserialize)]
pub struct TtsConfigPatch {
    pub voice: Option<String>,
    pub speed: Option<f32>,
}

fn engine_or_err(state: &AppState) -> Result<&std::sync::RwLock<TtsEngine>, String> {
    state.engine().ok_or_else(|| "TTS engine not ready (model still loading)".to_string())
}

fn build_status(state: &AppState) -> ReaderStatus {
    let tracker = state.tracker.lock().unwrap();
    let config = state.config.read().unwrap();

    let (state_str, current_sentence_index) = match state.engine() {
        Some(lock) => {
            let engine = lock.read().unwrap();
            let status = engine.status();
            drop(engine);

            let chunks_completed = tracker.total_chunks.saturating_sub(status.queue_length);
            let idx = if status.state != nayru_core::types::TtsState::Idle {
                tracker.current_sentence(chunks_completed)
            } else {
                None
            };

            let s = match status.state {
                nayru_core::types::TtsState::Idle => "idle",
                nayru_core::types::TtsState::Converting => "converting",
                nayru_core::types::TtsState::Playing => "playing",
            };
            (s, idx)
        }
        None => ("idle", None),
    };

    ReaderStatus {
        state: state_str.to_string(),
        current_sentence_index,
        total_sentences: tracker.total_sentences_in_text(),
        voice: config.voice.clone(),
        speed: config.speed,
    }
}

#[tauri::command]
pub async fn speak_from(
    text: String,
    sentence_index: usize,
    state: State<'_, AppState>,
) -> Result<ReaderStatus, String> {
    let t0 = std::time::Instant::now();
    tracing::info!("speak_from: idx={sentence_index} text_len={}", text.len());

    engine_or_err(&state)?.read().unwrap().stop();

    let tracker = SentenceTracker::new(&text, sentence_index);
    let to_speak: String = tracker.sentences.join(" ");

    engine_or_err(&state)?.read().unwrap().speak(&to_speak);
    *state.tracker.lock().unwrap() = tracker;

    let status = build_status(&state);
    tracing::info!("speak_from: done in {:?}", t0.elapsed());
    Ok(status)
}

#[tauri::command]
pub async fn tts_stop(state: State<'_, AppState>) -> Result<(), String> {
    engine_or_err(&state)?.read().unwrap().stop();
    *state.tracker.lock().unwrap() = SentenceTracker::empty();
    Ok(())
}

#[tauri::command]
pub fn tts_pause(state: State<'_, AppState>) -> Result<(), String> {
    engine_or_err(&state)?.read().unwrap().pause();
    Ok(())
}

#[tauri::command]
pub fn tts_resume(state: State<'_, AppState>) -> Result<(), String> {
    engine_or_err(&state)?.read().unwrap().resume();
    Ok(())
}

#[tauri::command]
pub async fn tts_skip_sentence(state: State<'_, AppState>) -> Result<ReaderStatus, String> {
    let engine = engine_or_err(&state)?;

    let (next_index, full_text) = {
        let status = engine.read().unwrap().status();
        let tracker = state.tracker.lock().unwrap();

        let chunks_completed = tracker.total_chunks.saturating_sub(status.queue_length);
        let idx = match tracker.current_sentence(chunks_completed) {
            Some(idx) => idx + 1,
            None => return Ok(build_status(&state)),
        };
        (idx, tracker.full_text.clone())
    };

    let all_sentences = split_sentences(&full_text);
    if next_index >= all_sentences.len() {
        engine.read().unwrap().stop();
        *state.tracker.lock().unwrap() = SentenceTracker::empty();
        return Ok(build_status(&state));
    }

    engine.read().unwrap().stop();
    let tracker = SentenceTracker::new(&full_text, next_index);
    let to_speak: String = tracker.sentences.join(" ");
    engine.read().unwrap().speak(&to_speak);
    *state.tracker.lock().unwrap() = tracker;

    Ok(build_status(&state))
}

#[tauri::command]
pub async fn get_reader_status(state: State<'_, AppState>) -> Result<ReaderStatus, String> {
    Ok(build_status(&state))
}

#[tauri::command]
pub async fn set_tts_config(
    patch: TtsConfigPatch,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let new_engine = {
        let mut config = state.config.write().unwrap();
        let mut changed = false;

        if let Some(voice) = patch.voice {
            if voice != config.voice {
                config.voice = voice;
                changed = true;
            }
        }
        if let Some(speed) = patch.speed {
            if (speed - config.speed).abs() > 0.01 {
                config.speed = speed;
                changed = true;
            }
        }

        if changed {
            let kokoro = state
                .kokoro
                .get()
                .ok_or("Kokoro model not loaded")?
                .clone();
            Some(TtsEngine::new(
                TtsConfig {
                    voice: config.voice.clone(),
                    speed: config.speed,
                    ..Default::default()
                },
                kokoro,
            ))
        } else {
            None
        }
    };

    if let Some(engine) = new_engine {
        state.replace_engine(engine);
        *state.tracker.lock().unwrap() = SentenceTracker::empty();
    }

    Ok(())
}

#[tauri::command]
pub fn get_tts_config(state: State<'_, AppState>) -> Result<ReaderConfig, String> {
    Ok(state.config.read().unwrap().clone())
}

#[tauri::command]
pub async fn get_server_status(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.kokoro.get().is_some())
}
