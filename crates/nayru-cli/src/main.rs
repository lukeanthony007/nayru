//! nayru CLI — standalone voice server.
//!
//! ```text
//! nayru serve [--port 2003] [--host 127.0.0.1] [--voice af_jadzia]
//! nayru speak "hello world" [--server http://localhost:2003]
//! nayru stop / skip / pause / resume / status [--server ...]
//! ```

use std::sync::Arc;

use clap::{Parser, Subcommand};

/// nayru — voice server with TTS playback
#[derive(Parser)]
#[command(name = "nayru", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start the nayru voice server
    Serve {
        /// Listen port
        #[arg(long, default_value = "2003")]
        port: u16,
        /// Listen host
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// Default TTS voice
        #[arg(long, default_value = "af_heart")]
        voice: String,
        /// TTS playback speed
        #[arg(long, default_value = "1.0")]
        speed: f32,
        /// Path to kokoro ONNX model file
        #[arg(long)]
        model: String,
        /// Path to kokoro voices file
        #[arg(long)]
        voices: String,
    },
    /// Send text to the running server for speech
    Speak {
        /// Text to speak
        text: String,
        /// Server URL
        #[arg(long, default_value = "http://localhost:2003")]
        server: String,
    },
    /// Stop all speech
    Stop {
        #[arg(long, default_value = "http://localhost:2003")]
        server: String,
    },
    /// Skip current clip
    Skip {
        #[arg(long, default_value = "http://localhost:2003")]
        server: String,
    },
    /// Pause playback
    Pause {
        #[arg(long, default_value = "http://localhost:2003")]
        server: String,
    },
    /// Resume playback
    Resume {
        #[arg(long, default_value = "http://localhost:2003")]
        server: String,
    },
    /// Get server status
    Status {
        #[arg(long, default_value = "http://localhost:2003")]
        server: String,
    },
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Command::Serve {
            port,
            host,
            voice,
            speed,
            model,
            voices,
        } => {
            eprintln!("loading kokoro model...");
            let kokoro = nayru_lib::kokoro::KokoroSynth::new(
                std::path::Path::new(&model),
                std::path::Path::new(&voices),
            )
            .await
            .expect("failed to load kokoro model");
            let kokoro = Arc::new(kokoro);

            let config = nayru_lib::nayru_core::types::TtsConfig {
                voice,
                speed,
                ..Default::default()
            };

            let engine = nayru_lib::tts::TtsEngine::new(config, kokoro);
            let app = nayru_lib::server::router(engine);

            let addr = format!("{host}:{port}");
            eprintln!("nayru listening on {addr}");

            let listener = tokio::net::TcpListener::bind(&addr)
                .await
                .expect("failed to bind");

            axum::serve(listener, app).await.expect("server error");
        }

        Command::Speak { text, server } => {
            let resp = reqwest::Client::new()
                .post(format!("{server}/speak"))
                .json(&serde_json::json!({ "text": text }))
                .send()
                .await
                .expect("request failed");
            println!("{}", resp.text().await.unwrap_or_default());
        }

        Command::Stop { server } => post_simple(&server, "stop").await,
        Command::Skip { server } => post_simple(&server, "skip").await,
        Command::Pause { server } => post_simple(&server, "pause").await,
        Command::Resume { server } => post_simple(&server, "resume").await,

        Command::Status { server } => {
            let resp = reqwest::Client::new()
                .get(format!("{server}/status"))
                .send()
                .await
                .expect("request failed");
            println!("{}", resp.text().await.unwrap_or_default());
        }
    }
}

async fn post_simple(server: &str, endpoint: &str) {
    let resp = reqwest::Client::new()
        .post(format!("{server}/{endpoint}"))
        .send()
        .await
        .expect("request failed");
    println!("{}", resp.text().await.unwrap_or_default());
}
