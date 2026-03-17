//! Voice service lifecycle manager — spawns and monitors whisper-server.
//!
//! Kokoro TTS is now handled in-process via kokoro-tts crate (no sidecar).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::process::Child;
use tokio::sync::Mutex;

pub use nayru_core::types::{ServiceStatus, VoiceServicesStatus};
use nayru_core::types::{DownloadProgress, KOKORO_MODEL, KOKORO_VOICES, WHISPER_MODEL};

use crate::download;

const WHISPER_SIDECAR: &str = "whisper-server";
const WHISPER_PORT: u16 = 2022;

struct RunningService {
    child: Child,
    #[allow(dead_code)]
    name: String,
}

pub struct VoiceServiceManager {
    whisper: Arc<Mutex<Option<RunningService>>>,
}

impl Default for VoiceServiceManager {
    fn default() -> Self {
        Self {
            whisper: Arc::new(Mutex::new(None)),
        }
    }
}

impl VoiceServiceManager {
    pub async fn status(&self, models_dir: &Path) -> VoiceServicesStatus {
        let whisper_model = download::model_exists(models_dir, &WHISPER_MODEL);
        let kokoro_model = download::model_exists(models_dir, &KOKORO_MODEL);

        let whisper_running = self.is_running(&self.whisper).await;

        VoiceServicesStatus {
            whisper: ServiceStatus {
                model_downloaded: whisper_model,
                running: whisper_running,
                port: WHISPER_PORT,
            },
            kokoro: ServiceStatus {
                model_downloaded: kokoro_model,
                running: true, // in-process, always "running" when engine is loaded
                port: 0,
            },
        }
    }

    /// Download Kokoro model files (ONNX + voices) and return their paths.
    pub async fn ensure_kokoro_models(
        &self,
        models_dir: &Path,
        on_progress: impl Fn(DownloadProgress),
    ) -> Result<(PathBuf, PathBuf), String> {
        let model_path =
            download::download_model(models_dir, &KOKORO_MODEL, &on_progress).await?;
        let voices_path =
            download::download_model(models_dir, &KOKORO_VOICES, &on_progress).await?;
        Ok((model_path, voices_path))
    }

    pub async fn stop(&self) {
        self.kill_service(&self.whisper).await;
    }

    pub fn stop_sync(&self) {
        if let Ok(mut guard) = self.whisper.try_lock() {
            if let Some(mut svc) = guard.take() {
                let _ = svc.child.start_kill();
            }
        }
    }

    async fn start_whisper(&self, model_path: &PathBuf) -> Result<(), String> {
        let binary = self.resolve_sidecar(WHISPER_SIDECAR)?;

        let child = tokio::process::Command::new(&binary)
            .args([
                "--model",
                &model_path.to_string_lossy(),
                "--host",
                "127.0.0.1",
                "--port",
                &WHISPER_PORT.to_string(),
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("failed to spawn whisper-server: {e}"))?;

        Self::drain_stderr(child, "whisper", &self.whisper).await;
        Ok(())
    }

    async fn drain_stderr(
        mut child: Child,
        name: &str,
        slot: &Arc<Mutex<Option<RunningService>>>,
    ) {
        let name_owned = name.to_string();

        if let Some(stderr) = child.stderr.take() {
            let name_log = name_owned.clone();
            tokio::spawn(async move {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    tracing::debug!("[{name_log}] {line}");
                }
            });
        }

        let mut guard = slot.lock().await;
        *guard = Some(RunningService {
            child,
            name: name_owned,
        });
    }

    fn resolve_sidecar(&self, name: &str) -> Result<PathBuf, String> {
        let exe = std::env::current_exe()
            .map_err(|e| format!("cannot determine executable path: {e}"))?;
        let exe_dir = exe
            .parent()
            .ok_or_else(|| "executable has no parent directory".to_string())?;

        let triple = target_triple();

        let with_triple = exe_dir.join(format!("{name}-{triple}"));
        if with_triple.is_file() {
            return Ok(with_triple);
        }

        let with_triple_exe = exe_dir.join(format!("{name}-{triple}.exe"));
        if with_triple_exe.is_file() {
            return Ok(with_triple_exe);
        }

        let without = exe_dir.join(name);
        if without.is_file() {
            return Ok(without);
        }

        Ok(PathBuf::from(name))
    }

    async fn is_running(&self, slot: &Arc<Mutex<Option<RunningService>>>) -> bool {
        let mut guard = slot.lock().await;
        if let Some(ref mut svc) = *guard {
            match svc.child.try_wait() {
                Ok(None) => true,
                Ok(Some(_)) => {
                    *guard = None;
                    false
                }
                Err(_) => {
                    *guard = None;
                    false
                }
            }
        } else {
            false
        }
    }

    async fn kill_service(&self, slot: &Arc<Mutex<Option<RunningService>>>) {
        let mut guard = slot.lock().await;
        if let Some(mut svc) = guard.take() {
            let _ = svc.child.kill().await;
        }
    }
}

fn target_triple() -> &'static str {
    #[cfg(all(target_arch = "x86_64", target_os = "linux"))]
    {
        "x86_64-unknown-linux-gnu"
    }
    #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
    {
        "aarch64-unknown-linux-gnu"
    }
    #[cfg(all(target_arch = "x86_64", target_os = "macos"))]
    {
        "x86_64-apple-darwin"
    }
    #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
    {
        "aarch64-apple-darwin"
    }
    #[cfg(all(target_arch = "x86_64", target_os = "windows"))]
    {
        "x86_64-pc-windows-msvc"
    }
}
