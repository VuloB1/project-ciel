use thiserror::Error;

#[derive(Debug, Error)]
pub enum EmbeddingError {
    /// No modelo descargado todavía — mirrors `SttError::Unavailable`:
    /// explicit, never a silent no-op (docs/security.md). Callers (see
    /// `aivt_memory::MemoryService`) match on this variant specifically to
    /// degrade to FTS-only search instead of surfacing an error.
    #[error("modelo de embeddings no disponible: {0}")]
    Unavailable(String),
    #[error("error de red descargando el modelo de embeddings: {0}")]
    Network(#[from] reqwest::Error),
    /// Hugging Face respondió con un error al descargar el modelo (no
    /// existe ese archivo, límite de tasa, etc.).
    #[error("la descarga del modelo respondió con un error: {0}")]
    Server(String),
    #[error("no se pudo cargar el modelo de embeddings: {0}")]
    ModelLoad(String),
    #[error("no se pudo calcular el embedding: {0}")]
    Inference(String),
    #[error("error de E/S con el modelo de embeddings: {0}")]
    Io(#[from] std::io::Error),
}
