use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::StreamExt;
use tokio::io::AsyncWriteExt;
use tokio::sync::{Mutex, RwLock};
use whisper_rs::{WhisperContext, WhisperContextParameters};

use aivt_eventbus::{AppEvent, EventBus};

use crate::error::SttError;
use crate::model::WhisperModelSize;

/// How often (at most) a download publishes a progress event — throttled
/// by time, not by chunk count, so a fast connection doesn't spam the
/// event bus and a slow one still updates regularly.
const PROGRESS_THROTTLE: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelDownloadStatus {
    pub size: WhisperModelSize,
    pub downloaded: bool,
}

struct Inner {
    models_dir: PathBuf,
    client: reqwest::Client,
    event_bus: EventBus,
    /// Keyed on `(size, gpu_enabled)`, not just `size` — flipping the GPU
    /// toggle in Settings without changing the model size must still
    /// trigger a reload, since `WhisperContextParameters::use_gpu` is only
    /// applied at load time.
    loaded: RwLock<Option<(WhisperModelSize, bool, Arc<WhisperContext>)>>,
    /// Only one download at a time — simple, and there's only ever one
    /// "Descargar" button active in the Settings UI anyway.
    downloading: Mutex<()>,
}

/// Owns the on-disk GGML model files and the one loaded `WhisperContext`
/// kept in memory. Loading a model is real work (disk I/O + building the
/// inference graph) so it's cached here and only reloaded when the
/// selected size actually changes — unlike the old `HttpSttProvider`,
/// which was cheap enough to rebuild on every utterance.
#[derive(Clone)]
pub struct SttModelManager(Arc<Inner>);

impl SttModelManager {
    pub fn new(models_dir: PathBuf, event_bus: EventBus) -> Self {
        Self(Arc::new(Inner {
            models_dir,
            client: reqwest::Client::new(),
            event_bus,
            loaded: RwLock::new(None),
            downloading: Mutex::new(()),
        }))
    }

    pub fn model_path(&self, size: WhisperModelSize) -> PathBuf {
        self.0.models_dir.join(size.file_name())
    }

    fn part_path(&self, size: WhisperModelSize) -> PathBuf {
        self.0.models_dir.join(format!("{}.part", size.file_name()))
    }

    pub fn is_downloaded(&self, size: WhisperModelSize) -> bool {
        self.model_path(size).is_file()
    }

    pub fn list_status(&self) -> Vec<ModelDownloadStatus> {
        WhisperModelSize::ALL.into_iter().map(|size| ModelDownloadStatus { size, downloaded: self.is_downloaded(size) }).collect()
    }

    /// Removes a downloaded model file. A no-op (not an error) if it isn't
    /// downloaded — mirrors `TTSProvider::stop`'s "safe to call even when
    /// there's nothing to do" stance. Also drops it from the in-memory
    /// cache if it happened to be the loaded one.
    pub async fn delete(&self, size: WhisperModelSize) -> Result<(), SttError> {
        let path = self.model_path(size);
        if path.is_file() {
            tokio::fs::remove_file(&path).await?;
        }
        let mut loaded = self.0.loaded.write().await;
        if matches!(&*loaded, Some((loaded_size, _, _)) if *loaded_size == size) {
            *loaded = None;
        }
        Ok(())
    }

    /// Downloads `size`'s GGML file to a `.part` path, publishing
    /// throttled `AppEvent::SttModelDownloadProgress` events, then renames
    /// it into place atomically — a half-downloaded file (e.g. the app
    /// closing mid-download) can never satisfy `is_downloaded()`.
    pub async fn download(&self, size: WhisperModelSize) -> Result<(), SttError> {
        let _guard = self.0.downloading.lock().await;

        if self.is_downloaded(size) {
            return Ok(());
        }

        tokio::fs::create_dir_all(&self.0.models_dir).await?;
        let part_path = self.part_path(size);

        let result = self.download_to_part(size, &part_path).await;
        match &result {
            Ok(()) => {
                tokio::fs::rename(&part_path, self.model_path(size)).await?;
                self.0.event_bus.publish(AppEvent::SttModelDownloadFinished { model_size: size.as_str().to_string() });
            }
            Err(err) => {
                let _ = tokio::fs::remove_file(&part_path).await;
                self.0.event_bus.publish(AppEvent::SttModelDownloadError {
                    model_size: size.as_str().to_string(),
                    message: err.to_string(),
                });
            }
        }
        result
    }

    async fn download_to_part(&self, size: WhisperModelSize, part_path: &Path) -> Result<(), SttError> {
        let response = self.0.client.get(size.download_url()).send().await?;
        if !response.status().is_success() {
            return Err(SttError::Server(format!("{}", response.status())));
        }
        let total_bytes = response.content_length();

        let mut file = tokio::fs::File::create(part_path).await?;
        let mut downloaded_bytes = 0u64;
        let mut last_emit = Instant::now();
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            downloaded_bytes += chunk.len() as u64;

            if last_emit.elapsed() >= PROGRESS_THROTTLE {
                self.0.event_bus.publish(AppEvent::SttModelDownloadProgress {
                    model_size: size.as_str().to_string(),
                    downloaded_bytes,
                    total_bytes,
                });
                last_emit = Instant::now();
            }
        }
        file.flush().await?;

        self.0.event_bus.publish(AppEvent::SttModelDownloadProgress {
            model_size: size.as_str().to_string(),
            downloaded_bytes,
            total_bytes,
        });
        Ok(())
    }

    /// Returns the cached context if it already matches `size`; otherwise
    /// loads it from disk (inside `spawn_blocking` — real file I/O plus
    /// building the model graph, not async-friendly work) and caches it,
    /// replacing whatever was loaded before. Every `CallService::transcribe`
    /// call goes through this, so switching the selected model in Settings
    /// takes effect on the very next utterance with no extra invalidation
    /// step needed.
    pub async fn ensure_loaded(&self, size: WhisperModelSize, gpu_enabled: bool) -> Result<Arc<WhisperContext>, SttError> {
        if let Some((loaded_size, loaded_gpu, context)) = self.0.loaded.read().await.as_ref() {
            if *loaded_size == size && *loaded_gpu == gpu_enabled {
                return Ok(context.clone());
            }
        }

        let path = self.model_path(size);
        if !path.is_file() {
            return Err(SttError::Unavailable(format!(
                "el modelo '{}' no está descargado — ve a Configuración > Transcripción de voz",
                size.as_str()
            )));
        }

        let path_str = path.to_string_lossy().to_string();
        let context = tokio::task::spawn_blocking(move || {
            // CPU-only stays the default (see crate-level docs) — the one
            // behavior guaranteed identical on every customer machine
            // regardless of GPU vendor. `gpu_enabled` is an explicit opt-in
            // (Configuración > Transcripción de voz) that asks whisper.cpp's
            // Vulkan backend to try first; whisper.cpp itself falls back to
            // CPU automatically when `use_gpu: true` but no compatible
            // device is found, so this never hard-fails just because the
            // toggle is on without a matching GPU.
            let params = WhisperContextParameters { use_gpu: gpu_enabled, ..Default::default() };
            WhisperContext::new_with_params(&path_str, params)
        })
        .await
        .map_err(|err| SttError::ModelLoad(err.to_string()))?
        .map_err(|err| SttError::ModelLoad(err.to_string()))?;
        let context = Arc::new(context);

        *self.0.loaded.write().await = Some((size, gpu_enabled, context.clone()));
        Ok(context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager(dir: &std::path::Path) -> SttModelManager {
        SttModelManager::new(dir.to_path_buf(), EventBus::new())
    }

    #[test]
    fn is_downloaded_is_false_for_a_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!manager(dir.path()).is_downloaded(WhisperModelSize::Tiny));
    }

    #[test]
    fn is_downloaded_is_true_once_the_file_exists() {
        let dir = tempfile::tempdir().unwrap();
        let manager = manager(dir.path());
        std::fs::write(manager.model_path(WhisperModelSize::Tiny), b"fake model bytes").unwrap();
        assert!(manager.is_downloaded(WhisperModelSize::Tiny));
    }

    #[test]
    fn a_stale_part_file_does_not_count_as_downloaded() {
        let dir = tempfile::tempdir().unwrap();
        let manager = manager(dir.path());
        std::fs::write(manager.part_path(WhisperModelSize::Tiny), b"half a model").unwrap();
        assert!(!manager.is_downloaded(WhisperModelSize::Tiny));
    }

    #[test]
    fn list_status_reports_every_size() {
        let dir = tempfile::tempdir().unwrap();
        let manager = manager(dir.path());
        std::fs::write(manager.model_path(WhisperModelSize::Base), b"x").unwrap();

        let status = manager.list_status();
        assert_eq!(status.len(), 4);
        assert!(status.iter().any(|s| s.size == WhisperModelSize::Base && s.downloaded));
        assert!(status.iter().any(|s| s.size == WhisperModelSize::Tiny && !s.downloaded));
    }

    #[tokio::test]
    async fn ensure_loaded_fails_with_unavailable_when_not_downloaded() {
        let dir = tempfile::tempdir().unwrap();
        let manager = manager(dir.path());
        let result = manager.ensure_loaded(WhisperModelSize::Tiny, false).await;
        assert!(matches!(result, Err(SttError::Unavailable(_))));
    }

    #[tokio::test]
    async fn delete_is_a_no_op_when_nothing_is_downloaded() {
        let dir = tempfile::tempdir().unwrap();
        let manager = manager(dir.path());
        assert!(manager.delete(WhisperModelSize::Tiny).await.is_ok());
    }
}
