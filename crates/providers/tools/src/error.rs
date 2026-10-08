use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolError {
    #[error("argumentos inválidos para la herramienta: {0}")]
    InvalidArguments(String),
    #[error("error de red: {0}")]
    Network(String),
    #[error("la búsqueda no está configurada (falta la API key)")]
    NotConfigured,
    #[error("el backend de búsqueda respondió con un error: {0}")]
    BackendError(String),
}
