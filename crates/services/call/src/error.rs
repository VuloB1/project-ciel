use thiserror::Error;

#[derive(Debug, Error)]
pub enum CallError {
    #[error("no se pudo abrir el micrófono: {0}")]
    MicrophoneUnavailable(String),
    #[error("no hay un personaje activo — selecciónalo en Identidad")]
    NoActiveProfile,
    #[error("no hay ningún proveedor de IA configurado — ve a Configuración > Proveedores IA")]
    NoLlmConnection,
    #[error("no hay un modelo de transcripción descargado — ve a Configuración > Transcripción de voz")]
    NoSttModel,
    #[error("hay una llamada en curso — el micrófono ya está en uso")]
    CallInProgress,
    #[error("la transcripción falló: {0}")]
    Transcription(#[from] aivt_provider_stt::SttError),
    #[error("la generación de la respuesta falló: {0}")]
    Generation(String),
    #[error("error de base de datos: {0}")]
    Database(#[from] aivt_database::DatabaseError),
    #[error("error guardando ajustes: {0}")]
    Settings(String),
}
