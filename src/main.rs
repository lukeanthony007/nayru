//! nayru CLI — standalone voice server.
//!
//! ```text
//! nayru serve [--port 2003] [--host 127.0.0.1] [--voice af_jadzia]
//! nayru speak "hello world" [--server http://localhost:2003]
//! nayru stop / skip / pause / resume / status [--server ...]
//! ```

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
        #[arg(long, default_value = "af_jadzia")]
        voice: String,
        /// Kokoro TTS server URL
        #[arg(long, default_value = "http://localhost:8880")]
        kokoro_url: String,
        /// TTS playback speed
        #[arg(long, default_value = "1.0")]
        speed: f32,
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
            kokoro_url,
            speed,
        } => {
            let config = nayru::tts::TtsConfig {
                kokoro_url,
                voice,
                speed,
                ..Default::default()
            };

            let engine = nayru::tts::TtsEngine::new(config);
            let app = nayru::server::router(engine);

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
