use thiserror::Error;

#[derive(Debug, Error)]
pub enum TwitchError {
    #[error("no hay una cuenta de Twitch conectada — ve a Configuración > Integraciones")]
    NotConnected,
    #[error("no se pudo abrir el navegador para autorizar: {0}")]
    BrowserOpenFailed(String),
    #[error("se agotó el tiempo de espera por la autorización")]
    AuthorizationTimedOut,
    #[error("Twitch rechazó la autorización: {0}")]
    AuthorizationDenied(String),
    #[error("error de red hablando con Twitch: {0}")]
    Network(#[from] reqwest::Error),
    #[error("Twitch respondió con un error ({status}): {body}")]
    Api { status: u16, body: String },
    #[error("no se encontró el canal «{0}» en Twitch — revisa que el nombre esté bien escrito")]
    ChannelNotFound(String),
    #[error("no se pudo conectar a EventSub: {0}")]
    EventSubConnect(String),
    #[error("EventSub cerró la conexión inesperadamente")]
    EventSubClosed,
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
