//! Nayru — TTS reader desktop application.
//!
//! Tauri backend providing sentence-aware TTS playback via nayru-lib's TtsEngine.

pub mod commands;
pub mod state;
pub mod tracker;

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
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            eprintln!("Tauri application error: {e}");
        });
}
