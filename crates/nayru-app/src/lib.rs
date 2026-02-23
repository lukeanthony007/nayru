//! Nayru — TTS reader desktop application.
//!
//! Tauri backend providing sentence-aware TTS playback via nayru-lib's TtsEngine.

pub mod commands;
pub mod state;
pub mod tracker;

use tauri::{Emitter, Manager};

/// Configure and run the Tauri application.
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "nayru_app_lib=debug,nayru_lib=debug".into()),
        )
        .init();

    // AppState::new() is cheap — engine is lazily created on first use
    // (from an async command where the tokio runtime is guaranteed).
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
                start_kokoro_server(handle).await;
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                let state = app_handle.state::<state::AppState>();
                state.service_manager.stop_kokoro_sync();
                tracing::info!("kokoro server stopped on exit");
            }
        });
}

async fn start_kokoro_server(handle: tauri::AppHandle) {
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

    // Check if Kokoro is already running externally
    emit("checking", "Checking for Kokoro TTS server...", None);
    if state.service_manager.is_kokoro_reachable().await {
        tracing::info!("kokoro server already running on port 3001");
        emit("ready", "Kokoro TTS server is ready", None);
        return;
    }

    // Resolve models directory
    let models_dir = match handle.path().app_data_dir() {
        Ok(dir) => dir.join("models"),
        Err(e) => {
            emit("error", &format!("Failed to resolve app data dir: {e}"), None);
            return;
        }
    };

    // Start Kokoro: download model + spawn server + health check
    emit("downloading", "Preparing Kokoro TTS model...", Some(0.0));
    let emit_handle = handle.clone();
    let result = state
        .service_manager
        .start_kokoro_only(&models_dir, move |progress| {
            let _ = emit_handle.emit(
                "server-startup",
                ServerStartupEvent {
                    phase: if progress.status == "complete" {
                        "starting".to_string()
                    } else {
                        "downloading".to_string()
                    },
                    message: format!("Downloading Kokoro model: {:.0}%", progress.percent),
                    progress: Some(progress.percent),
                },
            );
        })
        .await;

    match result {
        Ok(()) => {
            emit("ready", "Kokoro TTS server is ready", None);
            tracing::info!("kokoro server started successfully");
        }
        Err(e) => {
            emit("error", &format!("Failed to start Kokoro: {e}"), None);
            tracing::error!("failed to start kokoro server: {e}");
        }
    }
}
