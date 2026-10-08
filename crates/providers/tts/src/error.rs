use thiserror::Error;

#[derive(Debug, Error)]
pub enum TtsError {
    /// The requested backend can't be used right now — e.g. native
    /// synthesis failed to initialize (no speech engine installed) or an
    /// HTTP backend was requested with no `base_url` configured. Mirrors
    /// `SecretError::Unavailable` (see docs/security.md): explicit, never a
    /// silent no-op.
    #[error("motor de voz no disponible: {0}")]
    Unavailable(String),
    #[error("el motor de voz nativo falló: {0}")]
    Native(String),
    #[error("error de red hablando con el servidor de voz: {0}")]
    Network(#[from] reqwest::Error),
    #[error("el servidor de voz respondió con un error: {0}")]
    Server(String),
    #[error("no se pudo reproducir el audio: {0}")]
    Playback(String),
}
