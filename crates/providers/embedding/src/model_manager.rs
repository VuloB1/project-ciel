use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use candle_core::{DType, Device};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config as BertConfig};
use futures::StreamExt;
use tokenizers::{Tokenizer, TruncationParams};
use tokio::io::AsyncWriteExt;
use tokio::sync::{Mutex, RwLock};

use aivt_eventbus::{AppEvent, EventBus};

use crate::error::EmbeddingError;
use crate::model::{self, CONFIG_FILE, MAX_SEQUENCE_LENGTH, TOKENIZER_FILE, WEIGHTS_FILE};

/// How often (at most) the `model.safetensors` download publishes a
/// progress event — same throttling shape as `SttModelManager`, and for
/// the same reason (a fast connection shouldn't spam the event bus, a slow
/// one should still update regularly).
const PROGRESS_THROTTLE: Duration = Duration::from_millis(250);

/// An already-loaded model, ready for inference — cheap to clone (just an
/// `Arc` bump), expensive to build (disk I/O + parsing a 250k-token
/// tokenizer + mmap-ing the weights), so `EmbeddingModelManager` caches one
/// of these rather than rebuilding it per call. Not exported outside this
/// crate: `LocalEmbeddingProvider` (same crate) is the only consumer —
/// external crates only ever see the `EmbeddingProvider` trait.
pub(crate) struct LoadedEmbeddingModel {
    pub(crate) model: BertModel,
    pub(crate) tokenizer: Tokenizer,
    pub(crate) device: Device,
}

struct Inner {
    models_dir: PathBuf,
    client: reqwest::Client,
    event_bus: EventBus,
    loaded: RwLock<Option<Arc<LoadedEmbeddingModel>>>,
    /// Only one download at a time — same rationale as
    /// `SttModelManager::downloading`: there's only ever one "Descargar"
    /// button active in Settings anyway.
    downloading: Mutex<()>,
}

/// Owns the on-disk model files (`config.json`/`tokenizer.json`/
/// `model.safetensors`) and the one cached loaded model. Unlike
/// `SttModelManager` there's no size/GPU variant to key the cache on —
/// this app ships exactly one embedding model.
#[derive(Clone)]
pub struct EmbeddingModelManager(Arc<Inner>);

impl EmbeddingModelManager {
    pub fn new(models_dir: PathBuf, event_bus: EventBus) -> Self {
        Self(Arc::new(Inner {
            models_dir,
            client: reqwest::Client::new(),
            event_bus,
            loaded: RwLock::new(None),
            downloading: Mutex::new(()),
        }))
    }

    fn config_path(&self) -> PathBuf {
        self.0.models_dir.join(CONFIG_FILE)
    }

    fn tokenizer_path(&self) -> PathBuf {
        self.0.models_dir.join(TOKENIZER_FILE)
    }

    fn weights_path(&self) -> PathBuf {
        self.0.models_dir.join(WEIGHTS_FILE)
    }

    fn part_path(&self, file_name: &str) -> PathBuf {
        self.0.models_dir.join(format!("{file_name}.part"))
    }

    pub fn is_downloaded(&self) -> bool {
        self.config_path().is_file() && self.tokenizer_path().is_file() && self.weights_path().is_file()
    }

    /// Removes every downloaded file. A no-op (not an error) if nothing is
    /// downloaded — same "safe to call unconditionally" stance as
    /// `SttModelManager::delete`. Also drops the in-memory cache.
    pub async fn delete(&self) -> Result<(), EmbeddingError> {
        for path in [self.config_path(), self.tokenizer_path(), self.weights_path()] {
            if path.is_file() {
                tokio::fs::remove_file(&path).await?;
            }
        }
        *self.0.loaded.write().await = None;
        Ok(())
    }

    /// Downloads all three model files. `config.json`/`tokenizer.json` are
    /// small enough (well under one `PROGRESS_THROTTLE` tick even on a slow
    /// connection) to fetch as a single buffered request each — only
    /// `model.safetensors` (the ~449MB file that actually matters for UX)
    /// streams with progress events. If either small file fails, the big
    /// download never starts — fail fast.
    pub async fn download(&self) -> Result<(), EmbeddingError> {
        let _guard = self.0.downloading.lock().await;

        if self.is_downloaded() {
            return Ok(());
        }
        tokio::fs::create_dir_all(&self.0.models_dir).await?;

        let result = self.download_all().await;
        match &result {
            Ok(()) => self.0.event_bus.publish(AppEvent::EmbeddingModelDownloadFinished),
            Err(err) => {
                self.0.event_bus.publish(AppEvent::EmbeddingModelDownloadError { message: err.to_string() });
            }
        }
        result
    }

    async fn download_all(&self) -> Result<(), EmbeddingError> {
        self.download_small_file(CONFIG_FILE).await?;
        self.download_small_file(TOKENIZER_FILE).await?;
        self.download_weights().await
    }

    async fn download_small_file(&self, file_name: &str) -> Result<(), EmbeddingError> {
        let final_path = self.0.models_dir.join(file_name);
        if final_path.is_file() {
            return Ok(());
        }

        let part_path = self.part_path(file_name);
        let result = self.fetch_small_file(file_name, &part_path).await;
        match &result {
            Ok(()) => tokio::fs::rename(&part_path, &final_path).await?,
            Err(_) => {
                let _ = tokio::fs::remove_file(&part_path).await;
            }
        }
        result
    }

    async fn fetch_small_file(&self, file_name: &str, part_path: &Path) -> Result<(), EmbeddingError> {
        let response = self.0.client.get(model::download_url(file_name)).send().await?;
        if !response.status().is_success() {
            return Err(EmbeddingError::Server(format!("{}", response.status())));
        }
        let bytes = response.bytes().await?;
        tokio::fs::write(part_path, &bytes).await?;
        Ok(())
    }

    /// Streams `model.safetensors` to a `.part` path, publishing throttled
    /// `AppEvent::EmbeddingModelDownloadProgress` events, then renames it
    /// into place atomically — identical shape to
    /// `SttModelManager::download`/`download_to_part`.
    async fn download_weights(&self) -> Result<(), EmbeddingError> {
        let part_path = self.part_path(WEIGHTS_FILE);
        let result = self.stream_weights_to_part(&part_path).await;
        match &result {
            Ok(()) => tokio::fs::rename(&part_path, self.weights_path()).await?,
            Err(_) => {
                let _ = tokio::fs::remove_file(&part_path).await;
            }
        }
        result
    }

    async fn stream_weights_to_part(&self, part_path: &Path) -> Result<(), EmbeddingError> {
        let response = self.0.client.get(model::download_url(WEIGHTS_FILE)).send().await?;
        if !response.status().is_success() {
            return Err(EmbeddingError::Server(format!("{}", response.status())));
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
                self.0.event_bus.publish(AppEvent::EmbeddingModelDownloadProgress { downloaded_bytes, total_bytes });
                last_emit = Instant::now();
            }
        }
        file.flush().await?;

        self.0.event_bus.publish(AppEvent::EmbeddingModelDownloadProgress { downloaded_bytes, total_bytes });
        Ok(())
    }

    /// Returns the cached model if one's already loaded; otherwise loads it
    /// from disk (inside `spawn_blocking` — file I/O plus building the
    /// inference graph, not async-friendly work) and caches it. Fails with
    /// `EmbeddingError::Unavailable` (a normal, expected error, not a
    /// panic) if the model isn't downloaded yet.
    pub(crate) async fn ensure_loaded(&self) -> Result<Arc<LoadedEmbeddingModel>, EmbeddingError> {
        if let Some(loaded) = self.0.loaded.read().await.as_ref() {
            return Ok(loaded.clone());
        }

        if !self.is_downloaded() {
            return Err(EmbeddingError::Unavailable(
                "el modelo de embeddings no está descargado — ve a Configuración > Memoria".to_string(),
            ));
        }

        let config_path = self.config_path();
        let tokenizer_path = self.tokenizer_path();
        let weights_path = self.weights_path();

        let loaded = tokio::task::spawn_blocking(move || load_model(&config_path, &tokenizer_path, &weights_path))
            .await
            .map_err(|err| EmbeddingError::ModelLoad(err.to_string()))??;
        let loaded = Arc::new(loaded);

        *self.0.loaded.write().await = Some(loaded.clone());
        Ok(loaded)
    }
}

fn load_model(config_path: &Path, tokenizer_path: &Path, weights_path: &Path) -> Result<LoadedEmbeddingModel, EmbeddingError> {
    let config_json = std::fs::read_to_string(config_path)?;
    let config: BertConfig = serde_json::from_str(&config_json).map_err(|err| EmbeddingError::ModelLoad(err.to_string()))?;

    let mut tokenizer = Tokenizer::from_file(tokenizer_path).map_err(|err| EmbeddingError::ModelLoad(err.to_string()))?;
    tokenizer
        .with_truncation(Some(TruncationParams { max_length: MAX_SEQUENCE_LENGTH, ..Default::default() }))
        .map_err(|err| EmbeddingError::ModelLoad(err.to_string()))?;

    let device = Device::Cpu;
    // Safety: `weights_path` is a file this same manager downloaded and
    // never mutates in place afterward (downloads land via `.part` +
    // atomic rename), so mmap-ing it for the lifetime of the loaded model
    // is sound in practice, same as candle's own reference examples.
    let vb = unsafe {
        VarBuilder::from_mmaped_safetensors(&[weights_path], DType::F32, &device)
            .map_err(|err| EmbeddingError::ModelLoad(err.to_string()))?
    };
    let model = BertModel::load(vb, &config).map_err(|err| EmbeddingError::ModelLoad(err.to_string()))?;

    Ok(LoadedEmbeddingModel { model, tokenizer, device })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager(dir: &std::path::Path) -> EmbeddingModelManager {
        EmbeddingModelManager::new(dir.to_path_buf(), EventBus::new())
    }

    #[test]
    fn is_downloaded_is_false_when_any_file_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let manager = manager(dir.path());
        assert!(!manager.is_downloaded());

        std::fs::write(manager.config_path(), b"{}").unwrap();
        std::fs::write(manager.tokenizer_path(), b"{}").unwrap();
        assert!(!manager.is_downloaded(), "el peso todavía falta");
    }

    #[test]
    fn is_downloaded_is_true_once_all_three_files_exist() {
        let dir = tempfile::tempdir().unwrap();
        let manager = manager(dir.path());
        std::fs::write(manager.config_path(), b"{}").unwrap();
        std::fs::write(manager.tokenizer_path(), b"{}").unwrap();
        std::fs::write(manager.weights_path(), b"fake weights").unwrap();
        assert!(manager.is_downloaded());
    }

    #[test]
    fn a_stale_part_file_does_not_count_as_downloaded() {
        let dir = tempfile::tempdir().unwrap();
        let manager = manager(dir.path());
        std::fs::write(manager.part_path(WEIGHTS_FILE), b"half a model").unwrap();
        assert!(!manager.is_downloaded());
    }

    #[tokio::test]
    async fn ensure_loaded_fails_with_unavailable_when_not_downloaded() {
        let dir = tempfile::tempdir().unwrap();
        let manager = manager(dir.path());
        let result = manager.ensure_loaded().await;
        assert!(matches!(result, Err(EmbeddingError::Unavailable(_))));
    }

    #[tokio::test]
    async fn delete_is_a_no_op_when_nothing_is_downloaded() {
        let dir = tempfile::tempdir().unwrap();
        let manager = manager(dir.path());
        assert!(manager.delete().await.is_ok());
    }

    #[tokio::test]
    async fn delete_removes_every_downloaded_file() {
        let dir = tempfile::tempdir().unwrap();
        let manager = manager(dir.path());
        std::fs::write(manager.config_path(), b"{}").unwrap();
        std::fs::write(manager.tokenizer_path(), b"{}").unwrap();
        std::fs::write(manager.weights_path(), b"fake weights").unwrap();
        assert!(manager.is_downloaded());

        manager.delete().await.unwrap();
        assert!(!manager.is_downloaded());
    }
}
