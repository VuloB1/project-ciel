use thiserror::Error;

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error(transparent)]
    Database(#[from] aivt_database::DatabaseError),

    #[error("stored value for '{key}' is not valid JSON: {source}")]
    Decode { key: String, source: serde_json::Error },

    #[error(transparent)]
    Secret(#[from] aivt_secrets::SecretError),

    /// El proveedor de voz "en uso" no se puede resolver a un `TtsBackend`
    /// real ahora mismo (no hay ninguno configurado, el `id` apuntado ya no
    /// existe, o a un `Azure` le falta la región) — ver
    /// `SettingsService::build_tts_backend`.
    #[error("{0}")]
    Unavailable(String),
}
