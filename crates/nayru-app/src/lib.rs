//! Nayru — TTS reader desktop application.
//!
//! Tauri backend providing sentence-aware TTS playback via nayru-lib's TtsEngine.

pub mod commands;
pub mod state;
pub mod tracker;

use std::sync::Arc;
use tauri::{Emitter, Manager};

/// Configure and run the Tauri application.
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "nayru_app_lib=debug,nayru_lib=debug".into()),
        )
        .init();

    tauri::Builder::default()
        .manage(state::AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::speak_from,
            commands::tts_stop,
            commands::tts_pause,
            commands::tts_resume,
            commands::tts_skip_sentence,
            commands::get_reader_status,
            commands::set_tts_config,
            commands::get_tts_config,
            commands::get_server_status,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                load_kokoro_model(handle).await;
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error building tauri application")
        .run(|_app_handle, _event| {});
}

async fn load_kokoro_model(handle: tauri::AppHandle) {
    use nayru_core::types::ServerStartupEvent;

    let emit = |phase: &str, message: &str, progress: Option<f32>| {
        let _ = handle.emit(
            "server-startup",
            ServerStartupEvent {
                phase: phase.to_string(),
                message: message.to_string(),
                progress,
            },
        );
    };

    let state = handle.state::<state::AppState>();

    // Resolve models directory
    let models_dir = match handle.path().app_data_dir() {
        Ok(dir) => dir.join("models"),
        Err(e) => {
            emit(
                "error",
                &format!("Failed to resolve app data dir: {e}"),
                None,
            );
            return;
        }
    };

    // Download model files if needed
    emit("downloading", "Preparing Kokoro TTS model...", Some(0.0));
    let emit_handle = handle.clone();
    let result = state
        .service_manager
        .ensure_kokoro_models(&models_dir, move |progress| {
            let _ = emit_handle.emit(
                "server-startup",
                ServerStartupEvent {
                    phase: if progress.status == "complete" {
                        "loading".to_string()
                    } else {
                        "downloading".to_string()
                    },
                    message: format!("Downloading Kokoro model: {:.0}%", progress.percent),
                    progress: Some(progress.percent),
                },
            );
        })
        .await;

    let (model_path, voices_path) = match result {
        Ok(paths) => paths,
        Err(e) => {
            emit("error", &format!("Failed to download models: {e}"), None);
            tracing::error!("failed to download kokoro models: {e}");
            return;
        }
    };

    // Load the ONNX model into memory
    emit("loading", "Loading Kokoro TTS model...", None);
    tracing::info!("loading kokoro model from {}", model_path.display());
    let t0 = std::time::Instant::now();

    match nayru_lib::kokoro::KokoroSynth::new(&model_path, &voices_path).await {
        Ok(kokoro) => {
            tracing::info!("kokoro model loaded in {:?}", t0.elapsed());
            state.set_kokoro(Arc::new(kokoro));
            emit("ready", "Kokoro TTS is ready", None);
        }
        Err(e) => {
            emit("error", &format!("Failed to load Kokoro model: {e}"), None);
            tracing::error!("failed to load kokoro model: {e}");
        }
    }
}
