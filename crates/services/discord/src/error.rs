use thiserror::Error;

#[derive(Debug, Error)]
pub enum DiscordError {
    #[error("no hay un bot de Discord conectado — ve a Configuración > Integraciones")]
    NotConnected,
    #[error("falta el token del bot — pégalo desde el Developer Portal de Discord")]
    MissingToken,
    #[error("Discord rechazó el token del bot: {0}")]
    InvalidToken(String),
    #[error("error de red hablando con Discord: {0}")]
    Network(String),
    #[error("no se pudo conectar al Gateway de Discord: {0}")]
    GatewayConnect(String),
    #[error("el Gateway de Discord cerró la conexión inesperadamente")]
    GatewayClosed,
    #[error("error de base de datos: {0}")]
    Database(#[from] aivt_database::DatabaseError),
    #[error("error guardando ajustes: {0}")]
    Settings(String),
    #[error("error de secretos: {0}")]
    Secret(#[from] aivt_secrets::SecretError),
    #[error("no hay un personaje activo — selecciona uno en Identidad")]
    NoActiveProfile,
    #[error("no hay ningún proveedor de IA configurado — ve a Configuración > Proveedores IA")]
    NoLlmConnection,
    #[error("la generación de la respuesta falló: {0}")]
    Generation(String),
    #[error("la IA generó una respuesta vacía")]
    EmptyResponse,
}
