//! HTTP API for nayru TTS engine.
//!
//! Runs on port 2003 by default. CORS-permissive so raia-app can call from
//! localhost:3000.

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use tower_http::cors::CorsLayer;

use crate::tts::{TtsEngine, TtsStatus};

/// Build the axum router with a shared [`TtsEngine`].
pub fn router(engine: TtsEngine) -> Router {
    Router::new()
        .route("/speak", post(speak))
        .route("/stop", post(stop))
        .route("/skip", post(skip))
        .route("/pause", post(pause))
        .route("/resume", post(resume))
        .route("/status", get(status))
        .layer(CorsLayer::permissive())
        .with_state(engine)
}

// ─── Request / response types ──────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct SpeakRequest {
    text: String,
    #[serde(default)]
    voice: Option<String>,
}

#[derive(serde::Serialize)]
struct SpeakResponse {
    ok: bool,
    queued_chunks: usize,
}

#[derive(serde::Serialize)]
struct OkResponse {
    ok: bool,
}

// ─── Handlers ──────────────────────────────────────────────────────────────

async fn speak(State(engine): State<TtsEngine>, Json(req): Json<SpeakRequest>) -> Json<SpeakResponse> {
    let _ = req.voice; // TODO: per-request voice override
    let n = engine.speak(&req.text);
    Json(SpeakResponse {
        ok: true,
        queued_chunks: n,
    })
}

async fn stop(State(engine): State<TtsEngine>) -> Json<OkResponse> {
    engine.stop();
    Json(OkResponse { ok: true })
}

async fn skip(State(engine): State<TtsEngine>) -> Json<OkResponse> {
    engine.skip();
    Json(OkResponse { ok: true })
}

async fn pause(State(engine): State<TtsEngine>) -> Json<OkResponse> {
    engine.pause();
    Json(OkResponse { ok: true })
}

async fn resume(State(engine): State<TtsEngine>) -> Json<OkResponse> {
    engine.resume();
    Json(OkResponse { ok: true })
}

async fn status(State(engine): State<TtsEngine>) -> Json<TtsStatus> {
    Json(engine.status())
}
