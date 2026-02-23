//! Model downloader with progress reporting via callback

use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

use nayru_core::types::{DownloadProgress, ModelInfo, KOKORO_MODEL, WHISPER_MODEL};

/// Check if a model file exists under the given models directory
pub fn model_exists(models_dir: &std::path::Path, model: &ModelInfo) -> bool {
    models_dir.join(model.filename).is_file()
}

/// Get the path to a model file under the given models directory
pub fn model_path(models_dir: &std::path::Path, model: &ModelInfo) -> PathBuf {
    models_dir.join(model.filename)
}

/// Download a model with progress reporting.
pub async fn download_model(
    models_dir: &std::path::Path,
    model: &ModelInfo,
    on_progress: impl Fn(DownloadProgress),
) -> Result<PathBuf, String> {
    tokio::fs::create_dir_all(models_dir)
        .await
        .map_err(|e| format!("failed to create models dir: {e}"))?;

    let dest = models_dir.join(model.filename);

    if dest.is_file() {
        on_progress(DownloadProgress {
            model: model.name.to_string(),
            percent: 100.0,
            bytes_done: model.expected_size,
            bytes_total: model.expected_size,
            status: "complete".to_string(),
        });
        return Ok(dest);
    }

    let partial = models_dir.join(format!("{}.partial", model.filename));
    let existing_size = if partial.is_file() {
        tokio::fs::metadata(&partial)
            .await
            .map(|m| m.len())
            .unwrap_or(0)
    } else {
        0
    };

    let client = reqwest::Client::new();
    let mut req = client.get(model.url);

    if existing_size > 0 {
        req = req.header("Range", format!("bytes={existing_size}-"));
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("download request failed: {e}"))?;

    if !resp.status().is_success() && resp.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        return Err(format!("download failed with status {}", resp.status()));
    }

    let total_size = if resp.status() == reqwest::StatusCode::PARTIAL_CONTENT {
        resp.headers()
            .get("content-range")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.rsplit('/').next())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(model.expected_size)
    } else {
        resp.content_length().unwrap_or(model.expected_size)
    };

    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&partial)
        .await
        .map_err(|e| format!("failed to open partial file: {e}"))?;

    let mut bytes_done = existing_size;
    let mut stream = resp.bytes_stream();

    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("download stream error: {e}"))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("failed to write chunk: {e}"))?;

        bytes_done += chunk.len() as u64;
        let percent = (bytes_done as f32 / total_size as f32 * 100.0).min(100.0);

        on_progress(DownloadProgress {
            model: model.name.to_string(),
            percent,
            bytes_done,
            bytes_total: total_size,
            status: "downloading".to_string(),
        });
    }

    file.flush()
        .await
        .map_err(|e| format!("flush failed: {e}"))?;
    drop(file);

    tokio::fs::rename(&partial, &dest)
        .await
        .map_err(|e| format!("failed to finalize download: {e}"))?;

    on_progress(DownloadProgress {
        model: model.name.to_string(),
        percent: 100.0,
        bytes_done: total_size,
        bytes_total: total_size,
        status: "complete".to_string(),
    });

    Ok(dest)
}

/// Ensure both models are downloaded.
pub async fn ensure_models(
    models_dir: &std::path::Path,
    on_progress: impl Fn(DownloadProgress),
) -> Result<(PathBuf, PathBuf), String> {
    let whisper = download_model(models_dir, &WHISPER_MODEL, &on_progress).await?;
    let kokoro = download_model(models_dir, &KOKORO_MODEL, &on_progress).await?;
    Ok((whisper, kokoro))
}
