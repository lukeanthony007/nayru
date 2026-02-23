//! Voice service lifecycle manager — spawns and monitors whisper-server and kokoro-server

use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::process::Child;
use tokio::sync::Mutex;

pub use nayru_core::types::{ServiceStatus, VoiceServicesStatus};
use nayru_core::types::{DownloadProgress, KOKORO_MODEL, KOKORO_VOICES, WHISPER_MODEL};

use crate::download;

const WHISPER_SIDECAR: &str = "whisper-server";
const KOKORO_SIDECAR: &str = "koko";

const WHISPER_PORT: u16 = 2022;
const KOKORO_PORT: u16 = 3001;

struct RunningService {
    child: Child,
    #[allow(dead_code)]
    name: String,
}

pub struct VoiceServiceManager {
    whisper: Arc<Mutex<Option<RunningService>>>,
    kokoro: Arc<Mutex<Option<RunningService>>>,
}

impl Default for VoiceServiceManager {
    fn default() -> Self {
        Self {
            whisper: Arc::new(Mutex::new(None)),
            kokoro: Arc::new(Mutex::new(None)),
        }
    }
}

impl VoiceServiceManager {
    pub async fn status(&self, models_dir: &Path) -> VoiceServicesStatus {
        let whisper_model = download::model_exists(models_dir, &WHISPER_MODEL);
        let kokoro_model = download::model_exists(models_dir, &KOKORO_MODEL);

        let whisper_running = self.is_running(&self.whisper).await;
        let kokoro_running = self.is_running(&self.kokoro).await;

        VoiceServicesStatus {
            whisper: ServiceStatus {
                model_downloaded: whisper_model,
                running: whisper_running,
                port: WHISPER_PORT,
            },
            kokoro: ServiceStatus {
                model_downloaded: kokoro_model,
                running: kokoro_running,
                port: KOKORO_PORT,
            },
        }
    }

    pub async fn start(
        &self,
        models_dir: &Path,
        on_progress: impl Fn(DownloadProgress),
    ) -> Result<(), String> {
        let (whisper_model, kokoro_model) =
            download::ensure_models(models_dir, on_progress).await?;

        if !self.is_running(&self.whisper).await {
            self.start_whisper(&whisper_model).await?;
        }

        if !self.is_running(&self.kokoro).await {
            let voices = download::model_path(models_dir, &KOKORO_VOICES);
            self.start_kokoro(&kokoro_model, &voices).await?;
        }

        self.wait_for_health(WHISPER_PORT, "whisper", 15).await?;
        self.wait_for_health(KOKORO_PORT, "kokoro", 30).await?;

        Ok(())
    }

    /// Start only the Kokoro TTS server (download model + voices + spawn + health check).
    pub async fn start_kokoro_only(
        &self,
        models_dir: &Path,
        on_progress: impl Fn(DownloadProgress),
    ) -> Result<(), String> {
        let kokoro_model =
            download::download_model(models_dir, &KOKORO_MODEL, &on_progress).await?;
        let kokoro_voices =
            download::download_model(models_dir, &KOKORO_VOICES, &on_progress).await?;

        if !self.is_running(&self.kokoro).await {
            self.start_kokoro(&kokoro_model, &kokoro_voices).await?;
        }

        self.wait_for_health(KOKORO_PORT, "kokoro", 60).await?;

        Ok(())
    }

    /// Check if the Kokoro port is already responding.
    pub async fn is_kokoro_reachable(&self) -> bool {
        let client = reqwest::Client::new();
        client
            .get(format!("http://127.0.0.1:{KOKORO_PORT}/"))
            .timeout(std::time::Duration::from_secs(1))
            .send()
            .await
            .is_ok()
    }

    pub async fn stop(&self) {
        self.kill_service(&self.whisper).await;
        self.kill_service(&self.kokoro).await;
    }

    pub fn stop_sync(&self) {
        if let Ok(mut guard) = self.whisper.try_lock() {
            if let Some(mut svc) = guard.take() {
                let _ = svc.child.start_kill();
            }
        }
        if let Ok(mut guard) = self.kokoro.try_lock() {
            if let Some(mut svc) = guard.take() {
                let _ = svc.child.start_kill();
            }
        }
    }

    /// Synchronously kill only the Kokoro server process.
    pub fn stop_kokoro_sync(&self) {
        if let Ok(mut guard) = self.kokoro.try_lock() {
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

    async fn start_kokoro(
        &self,
        model_path: &PathBuf,
        voices_path: &PathBuf,
    ) -> Result<(), String> {
        let binary = self.resolve_sidecar(KOKORO_SIDECAR)?;

        // Ensure onnxruntime.dll is findable — place it next to the binary
        if let Some(binary_dir) = binary.parent() {
            let ort_dll = binary_dir.join("onnxruntime.dll");
            if !ort_dll.exists() {
                // Also check the exe directory
                if let Ok(exe) = std::env::current_exe() {
                    if let Some(exe_dir) = exe.parent() {
                        let exe_ort = exe_dir.join("onnxruntime.dll");
                        if exe_ort.exists() && !ort_dll.exists() {
                            let _ = std::fs::copy(&exe_ort, &ort_dll);
                        }
                    }
                }
            }
        }

        // koko CLI: koko --model <path> --data <voices> openai --ip 127.0.0.1 --port 3001
        let child = tokio::process::Command::new(&binary)
            .args([
                "--model",
                &model_path.to_string_lossy(),
                "--data",
                &voices_path.to_string_lossy(),
                "openai",
                "--ip",
                "127.0.0.1",
                "--port",
                &KOKORO_PORT.to_string(),
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("failed to spawn koko: {e}"))?;

        Self::drain_stderr(child, "kokoro", &self.kokoro).await;
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

        // Check for bundled sidecar with triple suffix (Tauri convention)
        let with_triple = exe_dir.join(format!("{name}-{triple}"));
        if with_triple.is_file() {
            return Ok(with_triple);
        }

        // Check with .exe extension (Windows)
        let with_triple_exe = exe_dir.join(format!("{name}-{triple}.exe"));
        if with_triple_exe.is_file() {
            return Ok(with_triple_exe);
        }

        // Check without triple
        let without = exe_dir.join(name);
        if without.is_file() {
            return Ok(without);
        }

        // PATH fallback
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

    async fn wait_for_health(
        &self,
        port: u16,
        name: &str,
        timeout_secs: u64,
    ) -> Result<(), String> {
        let url = format!("http://127.0.0.1:{port}/");
        let client = reqwest::Client::new();
        let deadline =
            tokio::time::Instant::now() + tokio::time::Duration::from_secs(timeout_secs);

        loop {
            if tokio::time::Instant::now() > deadline {
                return Err(format!(
                    "{name} service did not become ready within {timeout_secs}s"
                ));
            }

            match client.get(&url).send().await {
                Ok(_) => return Ok(()),
                Err(_) => {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }
            }
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
