use thiserror::Error;

#[derive(Debug, Error)]
pub enum SttError {
    /// No modelo descargado todavía para el tamaño seleccionado — mirrors
    /// the old HTTP path's "no server configured" state: explicit, never a
    /// silent no-op (docs/security.md).
    #[error("motor de transcripción no disponible: {0}")]
    Unavailable(String),
    #[error("error de red descargando el modelo de transcripción: {0}")]
    Network(#[from] reqwest::Error),
    /// Hugging Face respondió con un error al descargar el modelo (no
    /// existe ese archivo, límite de tasa, etc.).
    #[error("la descarga del modelo respondió con un error: {0}")]
    Server(String),
    #[error("no se pudo cargar el modelo de transcripción: {0}")]
    ModelLoad(String),
    #[error("la transcripción falló: {0}")]
    Transcription(String),
    #[error("error de E/S con el modelo de transcripción: {0}")]
    Io(#[from] std::io::Error),
}
