//! `EmbeddingProvider` trait + its local implementation — semantic memory
//! retrieval beyond SQLite FTS5 (Etapa 2/3 de la búsqueda semántica de
//! memoria; ver docs/ADR/004-memory-architecture.md, que ya anticipaba
//! "FTS5 primero, embeddings como mejora posterior").
//!
//! Runs entirely in-process via `candle` (framework de ML en Rust puro),
//! sin runtime externo que empaquetar (a diferencia de ONNX Runtime) —
//! misma filosofía "sin Docker, sin servidor, funciona offline" que ya
//! aplica `aivt-provider-stt` con whisper.cpp, y de hecho más simple de
//! compilar (candle no exige cmake ni un SDK de GPU).
//! `EmbeddingModelManager` owns downloading the model files on first use and
//! caching the loaded model; `LocalEmbeddingProvider` is the cheap per-call
//! wrapper around it.

mod error;
mod local_provider;
mod model;
mod model_manager;

use async_trait::async_trait;

pub use error::EmbeddingError;
pub use local_provider::LocalEmbeddingProvider;
pub use model::{APPROX_SIZE_MB, EMBEDDING_DIMENSION, MODEL_VERSION};
pub use model_manager::EmbeddingModelManager;

/// Named in `docs/ADR/003-provider-architecture.md` among the exchangeable
/// AI-capability traits from Fase 0 — filled in here. `embed` returns an
/// L2-normalized vector; callers use a plain dot product as cosine
/// similarity.
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;
}
