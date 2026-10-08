use std::sync::Arc;

use serde::{de::DeserializeOwned, Serialize};
use aivt_database::{AppSettingsRepository, DbPool};
use aivt_secrets::SecretStore;

use crate::SettingsError;

pub const DEFAULT_LANGUAGE: &str = "es";

const KEY_LANGUAGE: &str = "language";

const KEY_USER_AVATAR_PATH: &str = "user_avatar_path";

pub(crate) const SECRET_KEY_API_KEY: &str = "api_key";

/// Application-level view over `app_settings` (+ `SecretStore` for
/// anything sensitive): typed getters/setters for Configuración > General
/// and > AI Providers. Grows one well-known key at a time as later
/// Configuración subtabs (STT, TTS, Integrations...) land in their fases —
/// see sección 13 de docs/architecture.md for what's intentionally deferred.
///
/// "Iniciar con sistema" is *not* stored here: `tauri-plugin-autostart`
/// reads/writes the OS-level registration directly, which is already the
/// single source of truth.
#[derive(Clone)]
pub struct SettingsService {
    pub(crate) pool: DbPool,
    pub(crate) secrets: Arc<dyn SecretStore>,
}

impl SettingsService {
    pub fn new(pool: DbPool, secrets: Arc<dyn SecretStore>) -> Self {
        Self { pool, secrets }
    }

    pub async fn get_json<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, SettingsError> {
        let repo = AppSettingsRepository::new(&self.pool);
        match repo.get(key).await? {
            Some(raw) => {
                let value = serde_json::from_str(&raw)
                    .map_err(|source| SettingsError::Decode { key: key.to_string(), source })?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    pub async fn set_json<T: Serialize>(&self, key: &str, value: &T) -> Result<(), SettingsError> {
        let repo = AppSettingsRepository::new(&self.pool);
        let raw = serde_json::to_string(value).expect("settings values are always serializable");
        repo.set(key, &raw).await?;
        Ok(())
    }

    pub async fn language(&self) -> Result<String, SettingsError> {
        Ok(self
            .get_json(KEY_LANGUAGE)
            .await?
            .unwrap_or_else(|| DEFAULT_LANGUAGE.to_string()))
    }

    pub async fn set_language(&self, language: &str) -> Result<(), SettingsError> {
        self.set_json(KEY_LANGUAGE, &language).await
    }

    /// Where the user's own (not a character's) avatar image lives —
    /// relative to `data_dir`, written by `ProfileService::set_user_avatar`.
    /// This service only remembers the pointer; the file itself is that
    /// service's responsibility (it already owns all avatar file I/O).
    ///
    /// `get_json<T>` decodes the stored row's raw JSON directly as `T` —
    /// it only returns `None` itself when there's no row at all. Since the
    /// stored value here is *itself* an `Option<String>` (clearing writes
    /// the JSON literal `null`), `T` must be `Option<String>`, not
    /// `String` — otherwise decoding a cleared value as a bare `String`
    /// fails ("invalid type: null, expected a string"). `.flatten()`
    /// collapses "no row" and "row explicitly cleared" into the same
    /// `None`, which is the right answer either way.
    pub async fn user_avatar_path(&self) -> Result<Option<String>, SettingsError> {
        Ok(self.get_json::<Option<String>>(KEY_USER_AVATAR_PATH).await?.flatten())
    }

    pub async fn set_user_avatar_path(&self, path: Option<&str>) -> Result<(), SettingsError> {
        self.set_json(KEY_USER_AVATAR_PATH, &path).await
    }
}

#[cfg(test)]
pub(crate) async fn test_service() -> SettingsService {
    let pool = DbPool::connect("sqlite::memory:").await.unwrap();
    aivt_database::migrate(&pool).await.unwrap();
    SettingsService::new(pool, Arc::new(aivt_testing::MockSecretStore::new()))
}

#[cfg(test)]
mod tests {
    
    
    use aivt_testing::MockSecretStore;

    use super::*;

    async fn test_service() -> SettingsService {
        let pool = DbPool::connect("sqlite::memory:").await.unwrap();
        aivt_database::migrate(&pool).await.unwrap();
        SettingsService::new(pool, Arc::new(MockSecretStore::new()))
    }
    #[tokio::test]
    async fn language_defaults_to_spanish() {
        let service = test_service().await;
        assert_eq!(service.language().await.unwrap(), "es");
    }

    #[tokio::test]
    async fn language_round_trips() {
        let service = test_service().await;
        service.set_language("en").await.unwrap();
        assert_eq!(service.language().await.unwrap(), "en");
    }

    #[tokio::test]
    async fn user_avatar_path_defaults_to_none() {
        let service = test_service().await;
        assert!(service.user_avatar_path().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn user_avatar_path_round_trips() {
        let service = test_service().await;
        service.set_user_avatar_path(Some("avatars/user/avatar.png")).await.unwrap();
        assert_eq!(service.user_avatar_path().await.unwrap().as_deref(), Some("avatars/user/avatar.png"));
    }

    #[tokio::test]
    async fn user_avatar_path_can_be_cleared() {
        let service = test_service().await;
        service.set_user_avatar_path(Some("avatars/user/avatar.png")).await.unwrap();
        service.set_user_avatar_path(None).await.unwrap();
        assert!(service.user_avatar_path().await.unwrap().is_none());
    }
}
