//! Búsqueda web (Brave): ajustes y API key.

use serde::{Deserialize, Serialize};

use crate::service::SettingsService;
use crate::SettingsError;

const KEY_WEB_SEARCH: &str = "web_search";

const SECRET_SERVICE_WEB_SEARCH: &str = "aivt.web_search";

const SECRET_KEY_BRAVE_API_KEY: &str = "brave_api_key";

/// Fase 10b — búsqueda web como herramienta del personaje. Opt-in y
/// apagada por defecto, coherente con `docs/security.md` ("la app es
/// local-first... Chat, Memoria, TTS/STT locales, Avatar y Llamada local
/// deben funcionar sin conexión a Internet"): activar esto es la señal
/// explícita del usuario de que esos canales pueden necesitar red. Un solo
/// backend (Brave Search API) por ahora, así que a diferencia de
/// `TtsProviderEntry` no hace falta un enum de "kind" ni múltiples
/// slots de secreto — ver docs/DEVELOPMENT_STATUS.md, Fase 10b.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredWebSearchSettings {
    enabled: bool,
    has_api_key: bool,
    /// Lets the character read whole pages (`read_url`, via the optional Obscura download).
    #[serde(default)]
    read_pages: bool,
}

impl Default for StoredWebSearchSettings {
    fn default() -> Self {
        Self { enabled: false, has_api_key: false, read_pages: false }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchSettings {
    pub enabled: bool,
    pub has_api_key: bool,
    pub read_pages: bool,
}

/// No `Debug` derive — same reasoning as `UpdateTtsProviderInput`: this
/// carries a plaintext API key, and not deriving `Debug` at all means a
/// future accidental `{:?}`/`?field` log call fails to compile instead of
/// silently leaking the key to disk.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchInput {
    pub enabled: bool,
    /// `None`-keeps/`Some("")`-clears/`Some(key)`-sets — same contract as
    /// `UpdateTtsProviderInput::api_key`.
    pub api_key: Option<String>,
    /// `None` keeps the saved value.
    #[serde(default)]
    pub read_pages: Option<bool>,
}

impl SettingsService {
    pub async fn web_search_settings(&self) -> Result<WebSearchSettings, SettingsError> {
        let stored: StoredWebSearchSettings = self.get_json(KEY_WEB_SEARCH).await?.unwrap_or_default();
        Ok(WebSearchSettings { enabled: stored.enabled, has_api_key: stored.has_api_key, read_pages: stored.read_pages })
    }

    pub async fn set_web_search_settings(&self, input: WebSearchInput) -> Result<WebSearchSettings, SettingsError> {
        let existing: StoredWebSearchSettings = self.get_json(KEY_WEB_SEARCH).await?.unwrap_or_default();

        let has_api_key = match input.api_key.as_deref() {
            Some(key) if !key.is_empty() => {
                self.secrets.set(SECRET_SERVICE_WEB_SEARCH, SECRET_KEY_BRAVE_API_KEY, key).await?;
                true
            }
            Some(_) => {
                self.secrets.delete(SECRET_SERVICE_WEB_SEARCH, SECRET_KEY_BRAVE_API_KEY).await?;
                false
            }
            None => existing.has_api_key,
        };

        let stored = StoredWebSearchSettings { enabled: input.enabled, has_api_key, read_pages: input.read_pages.unwrap_or(existing.read_pages) };
        self.set_json(KEY_WEB_SEARCH, &stored).await?;

        Ok(WebSearchSettings { enabled: stored.enabled, has_api_key: stored.has_api_key, read_pages: stored.read_pages })
    }

    /// The raw secret — only for internal use when actually building the
    /// search tool (`aivt_provider_tools::BraveSearchTool`), never sent to
    /// the frontend.
    pub async fn web_search_api_key(&self) -> Result<Option<String>, SettingsError> {
        Ok(self.secrets.get(SECRET_SERVICE_WEB_SEARCH, SECRET_KEY_BRAVE_API_KEY).await?)
    }
}

#[cfg(test)]
mod tests {
    use aivt_database::AppSettingsRepository;
    

    use super::*;
    use crate::service::test_service;

    #[tokio::test]
    async fn web_search_defaults_to_disabled_with_no_key() {
        let service = test_service().await;
        let settings = service.web_search_settings().await.unwrap();
        assert!(!settings.enabled);
        assert!(!settings.has_api_key);
        assert!(service.web_search_api_key().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn web_search_round_trips_and_never_stores_the_key_in_plaintext() {
        let service = test_service().await;

        let saved = service.set_web_search_settings(WebSearchInput { enabled: true, api_key: Some("brave-secret".to_string()), read_pages: None }).await.unwrap();

        assert!(saved.enabled);
        assert!(saved.has_api_key);
        assert_eq!(service.web_search_api_key().await.unwrap().as_deref(), Some("brave-secret"));

        let raw = AppSettingsRepository::new(&service.pool).get(KEY_WEB_SEARCH).await.unwrap().unwrap();
        assert!(!raw.contains("brave-secret"));
    }

    #[tokio::test]
    async fn resaving_web_search_without_an_api_key_keeps_the_existing_one() {
        let service = test_service().await;
        service.set_web_search_settings(WebSearchInput { enabled: true, api_key: Some("brave-secret".to_string()), read_pages: None }).await.unwrap();

        // Toggling `enabled` off shouldn't require re-pasting the key.
        let saved = service.set_web_search_settings(WebSearchInput { enabled: false, api_key: None, read_pages: None }).await.unwrap();

        assert!(!saved.enabled);
        assert!(saved.has_api_key);
        assert_eq!(service.web_search_api_key().await.unwrap().as_deref(), Some("brave-secret"));
    }

    #[tokio::test]
    async fn reading_pages_is_off_by_default_and_survives_other_saves() {
        let service = test_service().await;
        assert!(!service.web_search_settings().await.unwrap().read_pages);

        let saved = service.set_web_search_settings(WebSearchInput { enabled: false, api_key: None, read_pages: Some(true) }).await.unwrap();
        assert!(saved.read_pages);

        let saved = service.set_web_search_settings(WebSearchInput { enabled: true, api_key: None, read_pages: None }).await.unwrap();
        assert!(saved.read_pages);
    }

    #[tokio::test]
    async fn clearing_the_web_search_api_key_with_an_empty_string() {
        let service = test_service().await;
        service.set_web_search_settings(WebSearchInput { enabled: true, api_key: Some("brave-secret".to_string()), read_pages: None }).await.unwrap();

        let saved = service.set_web_search_settings(WebSearchInput { enabled: true, api_key: Some(String::new()), read_pages: None }).await.unwrap();

        assert!(!saved.has_api_key);
        assert!(service.web_search_api_key().await.unwrap().is_none());
    }
}
