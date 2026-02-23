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
    pub kokoro_url: Option<String>,
}

fn build_status(state: &AppState) -> ReaderStatus {
    let t0 = std::time::Instant::now();
    let engine = state.engine().read().unwrap();
    let status = engine.status();
    drop(engine);
    tracing::debug!("build_status: engine status in {:?}", t0.elapsed());

    let tracker = state.tracker.lock().unwrap();
    let config = state.config.read().unwrap();

    let chunks_completed = tracker.total_chunks.saturating_sub(status.queue_length);
    let current_sentence_index = if status.state != nayru_core::types::TtsState::Idle {
        tracker.current_sentence(chunks_completed)
    } else {
        None
    };

    let state_str = match status.state {
        nayru_core::types::TtsState::Idle => "idle",
        nayru_core::types::TtsState::Converting => "converting",
        nayru_core::types::TtsState::Playing => "playing",
    };

    ReaderStatus {
        state: state_str.to_string(),
        current_sentence_index,
        total_sentences: tracker.total_sentences_in_text(),
        voice: config.voice.clone(),
        speed: config.speed,
    }
}

/// Start speaking from a specific sentence index.
#[tauri::command]
pub async fn speak_from(
    text: String,
    sentence_index: usize,
    state: State<'_, AppState>,
) -> Result<ReaderStatus, String> {
    let t0 = std::time::Instant::now();
    tracing::info!("speak_from: idx={sentence_index} text_len={}", text.len());

    // Stop any current speech
    state.engine().read().unwrap().stop();
    tracing::info!("speak_from: stop() in {:?}", t0.elapsed());

    // Build tracker
    let tracker = SentenceTracker::new(&text, sentence_index);
    let to_speak: String = tracker.sentences.join(" ");
    tracing::info!("speak_from: tracker built, {} sentences, speaking {} chars", tracker.sentences.len(), to_speak.len());

    // Speak
    state.engine().read().unwrap().speak(&to_speak);
    tracing::info!("speak_from: speak() dispatched in {:?}", t0.elapsed());

    // Store tracker
    *state.tracker.lock().unwrap() = tracker;

    let status = build_status(&state);
    tracing::info!("speak_from: done in {:?}", t0.elapsed());
    Ok(status)
}

/// Stop all speech.
#[tauri::command]
pub async fn tts_stop(state: State<'_, AppState>) -> Result<(), String> {
    state.engine().read().unwrap().stop();
    *state.tracker.lock().unwrap() = SentenceTracker::empty();
    Ok(())
}

/// Pause playback.
#[tauri::command]
pub fn tts_pause(state: State<'_, AppState>) -> Result<(), String> {
    state.engine().read().unwrap().pause();
    Ok(())
}

/// Resume playback.
#[tauri::command]
pub fn tts_resume(state: State<'_, AppState>) -> Result<(), String> {
    state.engine().read().unwrap().resume();
    Ok(())
}

/// Skip to the next sentence.
#[tauri::command]
pub async fn tts_skip_sentence(state: State<'_, AppState>) -> Result<ReaderStatus, String> {
    // Compute next index — read engine status, then drop guard before .await
    let (next_index, full_text) = {
        let status = state.engine().read().unwrap().status();
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
        state.engine().read().unwrap().stop();
        *state.tracker.lock().unwrap() = SentenceTracker::empty();
        return Ok(build_status(&state));
    }

    // Re-speak from next sentence
    state.engine().read().unwrap().stop();

    let tracker = SentenceTracker::new(&full_text, next_index);
    let to_speak: String = tracker.sentences.join(" ");

    state.engine().read().unwrap().speak(&to_speak);
    *state.tracker.lock().unwrap() = tracker;

    Ok(build_status(&state))
}

/// Get current reader status (polled by frontend).
#[tauri::command]
pub async fn get_reader_status(state: State<'_, AppState>) -> Result<ReaderStatus, String> {
    Ok(build_status(&state))
}

/// Update TTS config. Recreates the engine if settings changed.
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
        if let Some(url) = patch.kokoro_url {
            if url != config.kokoro_url {
                config.kokoro_url = url;
                changed = true;
            }
        }

        if changed {
            Some(TtsEngine::new(TtsConfig {
                kokoro_url: config.kokoro_url.clone(),
                voice: config.voice.clone(),
                speed: config.speed,
                ..Default::default()
            }))
        } else {
            None
        }
    }; // config guard dropped here

    if let Some(engine) = new_engine {
        state.replace_engine(engine);
        *state.tracker.lock().unwrap() = SentenceTracker::empty();
    }

    Ok(())
}

/// Get current TTS config.
#[tauri::command]
pub fn get_tts_config(state: State<'_, AppState>) -> Result<ReaderConfig, String> {
    Ok(state.config.read().unwrap().clone())
}
