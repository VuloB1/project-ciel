//! Proveedores de voz (TTS): lista, proveedor en uso y migración desde la conexión única.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use aivt_provider_tts::TtsBackend;

use crate::llm::OPENAI_BASE_URL;
use crate::service::{SettingsService, SECRET_KEY_API_KEY};
use crate::SettingsError;

/// Reemplazada por `KEY_TTS_PROVIDERS` (Motor de voz, multi-proveedor) — se
/// deja solo como origen de la migración de instalaciones existentes, ver
/// `SettingsService::load_stored_tts_providers`. Nunca se vuelve a escribir.
const KEY_TTS_CONNECTION: &str = "tts_connection";

/// v1: mismo criterio que `KEY_LLM_PROVIDERS` — reemplaza la conexión única
/// de arriba por una lista de proveedores + cuál está "en uso".
const KEY_TTS_PROVIDERS: &str = "tts_providers_v1";

const SECRET_SERVICE_TTS: &str = "aivt.tts_provider";

const SECRET_KEY_ELEVENLABS_API_KEY: &str = "elevenlabs_api_key";

const NATIVE_TTS_PROVIDER_ID: &str = "native";

/// El shape viejo de una sola conexión, bajo `KEY_TTS_CONNECTION` —
/// reemplazado por `StoredTtsProviderEntry`/`StoredTtsProviders`. Se
/// mantiene solo como lectura de compatibilidad para
/// `load_stored_tts_providers`, igual que `StoredLlmConnection` arriba.
/// `has_api_key`/`has_elevenlabs_api_key` eran dos slots fijos separados
/// (ver `SECRET_KEY_API_KEY`/`SECRET_KEY_ELEVENLABS_API_KEY`) — el
/// precedente que dio origen a `secret_key_for_tts_provider`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredTtsConnection {
    backend: TtsBackendKind,
    base_url: String,
    has_api_key: bool,
    #[serde(default)]
    has_elevenlabs_api_key: bool,
}

/// Qué motor de voz es una entrada. `Native` es un singleton especial (ver
/// `NATIVE_TTS_PROVIDER_ID`): existe siempre, sin configuración, no se
/// puede eliminar ni desactivar. `ElevenLabs`/`OpenAi`/`Google` hablan con
/// un endpoint fijo propio; `Azure` también, pero necesita además
/// `region` (ver `StoredTtsProviderEntry`). `Http` es el formulario de
/// siempre — cualquier servidor OpenAI-compatible, con `base_url` a mano.
/// Un único proveedor por kind salvo `Http` (sin límite) y `Native`
/// (siempre exactamente uno) — la regla vive en el comando Tauri, mismo
/// criterio que `LlmProviderKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TtsBackendKind {
    Native,
    /// `#[serde(rename)]` needed because `rename_all = "snake_case"` would
    /// otherwise produce `eleven_labs` (splitting at the capital `L`) —
    /// the frontend (and ElevenLabs' own branding) treats it as one word.
    #[serde(rename = "elevenlabs")]
    ElevenLabs,
    #[serde(rename = "openai")]
    OpenAi,
    Google,
    Azure,
    #[serde(rename = "fishaudio")]
    FishAudio,
    Deepgram,
    Groq,
    Together,
    Http,
}

fn default_label_for_tts_kind(kind: TtsBackendKind) -> &'static str {
    match kind {
        TtsBackendKind::Native => "Voces nativas del sistema",
        TtsBackendKind::ElevenLabs => "ElevenLabs",
        TtsBackendKind::OpenAi => "OpenAI",
        TtsBackendKind::Google => "Google Cloud",
        TtsBackendKind::Azure => "Azure Speech",
        TtsBackendKind::FishAudio => "Fish Audio",
        TtsBackendKind::Deepgram => "Deepgram",
        TtsBackendKind::Groq => "Groq",
        TtsBackendKind::Together => "Together AI",
        TtsBackendKind::Http => "Servidor genérico",
    }
}

/// Mismo generalizador que `secret_key_for_provider` (LLM), aplicado al
/// servicio de secretos de TTS — reemplaza los dos slots fijos que tenía
/// antes (`SECRET_KEY_API_KEY`/`SECRET_KEY_ELEVENLABS_API_KEY`, ver
/// `load_stored_tts_providers` para la migración de esos dos).
fn secret_key_for_tts_provider(id: &str) -> String {
    format!("api_key.{id}")
}

/// Lo que persiste `app_settings` bajo `KEY_TTS_PROVIDERS` para una sola
/// entrada — nunca el secreto en sí, solo `has_api_key`. `base_url` solo
/// se usa (y guarda) para `Http`; `region` solo para `Azure` (ej.
/// "eastus" — su endpoint real depende de ella, ver
/// `SettingsService::build_tts_backend`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredTtsProviderEntry {
    id: String,
    kind: TtsBackendKind,
    label: String,
    base_url: Option<String>,
    region: Option<String>,
    /// Solo `FishAudio`/`Groq`/`Together`: modelo a usar (cabecera `model` en
    /// Fish Audio). `None` = el que cada servicio trae por defecto.
    #[serde(default)]
    model: Option<String>,
    has_api_key: bool,
    /// "Activar" — a diferencia de `StoredLlmProviderEntry` no hay campo
    /// "fijar": Motor de voz no tiene rotación automática, el usuario
    /// elige a mano con "usar ahora" (ver el plan). Siempre `true` para la
    /// entrada `Native`, forzado por los comandos Tauri.
    enabled: bool,
}

/// La lista completa + cuál está "en uso" ahora mismo.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct StoredTtsProviders {
    #[serde(default)]
    providers: Vec<StoredTtsProviderEntry>,
    #[serde(default)]
    current_provider_id: Option<String>,
}

/// Lo que ve el frontend — mismo shape que `StoredTtsProviderEntry`, tipo
/// separado por la misma razón que `LlmProviderEntry`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsProviderEntry {
    pub id: String,
    pub kind: TtsBackendKind,
    pub label: String,
    pub base_url: Option<String>,
    pub region: Option<String>,
    pub model: Option<String>,
    pub has_api_key: bool,
    pub enabled: bool,
}

impl From<StoredTtsProviderEntry> for TtsProviderEntry {
    fn from(e: StoredTtsProviderEntry) -> Self {
        Self { id: e.id, kind: e.kind, label: e.label, base_url: e.base_url, region: e.region, model: e.model, has_api_key: e.has_api_key, enabled: e.enabled }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsProvidersSettings {
    pub providers: Vec<TtsProviderEntry>,
    pub current_provider_id: Option<String>,
}

impl From<StoredTtsProviders> for TtsProvidersSettings {
    fn from(s: StoredTtsProviders) -> Self {
        Self { providers: s.providers.into_iter().map(Into::into).collect(), current_provider_id: s.current_provider_id }
    }
}

/// Lo que escribe el frontend al agregar un proveedor nuevo. `base_url`
/// solo se usa para `Http`; `region` solo para `Azure`. Agregar `Native`
/// se rechaza en el comando Tauri — ya existe siempre.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddTtsProviderInput {
    pub kind: TtsBackendKind,
    pub label: Option<String>,
    pub base_url: Option<String>,
    pub region: Option<String>,
}

/// No `Debug` derive — mismo motivo que `UpdateLlmProviderInput`: carga un
/// `api_key` en texto plano, y un `TtsBackend` real ya filtró una key de
/// ElevenLabs a un log una vez por esto mismo (ver `aivt_provider_tts::types`).
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTtsProviderInput {
    pub label: String,
    pub base_url: Option<String>,
    pub region: Option<String>,
    /// Solo `FishAudio`.
    #[serde(default)]
    pub model: Option<String>,
    pub api_key: Option<String>,
}

impl SettingsService {
    /// Igual que `llm_provider_api_key`, para un proveedor de voz.
    pub async fn tts_provider_api_key(&self, id: &str) -> Result<Option<String>, SettingsError> {
        Ok(self.secrets.get(SECRET_SERVICE_TTS, &secret_key_for_tts_provider(id)).await?)
    }

    /// Lee la lista completa de proveedores de voz configurados + cuál está
    /// en uso. Migra sola, la primera vez que hace falta, desde la vieja
    /// conexión única — ver `load_stored_tts_providers`.
    pub async fn tts_providers(&self) -> Result<TtsProvidersSettings, SettingsError> {
        Ok(self.load_stored_tts_providers().await?.into())
    }

    /// Agrega una entrada nueva. Las reglas de "un solo X a la vez" (todos
    /// los kinds salvo `Http`) y "`Native` no se puede agregar de nuevo"
    /// viven en el comando Tauri, no acá — este método siempre agrega. La
    /// primera entrada agregada queda automáticamente "en uso".
    pub async fn add_tts_provider(&self, input: AddTtsProviderInput) -> Result<TtsProviderEntry, SettingsError> {
        let mut stored = self.load_stored_tts_providers().await?;
        let id = Uuid::new_v4().to_string();
        let label = input.label.filter(|l| !l.is_empty()).unwrap_or_else(|| default_label_for_tts_kind(input.kind).to_string());
        let entry = StoredTtsProviderEntry {
            id: id.clone(),
            kind: input.kind,
            label,
            base_url: (input.kind == TtsBackendKind::Http).then_some(input.base_url).flatten(),
            region: (input.kind == TtsBackendKind::Azure).then_some(input.region).flatten(),
            model: None,
            has_api_key: false,
            enabled: true,
        };
        stored.providers.push(entry.clone());
        if stored.current_provider_id.is_none() {
            stored.current_provider_id = Some(id);
        }
        self.set_json(KEY_TTS_PROVIDERS, &stored).await?;
        Ok(entry.into())
    }

    /// `Ok(None)` si `id` no corresponde a ninguna entrada — el llamador
    /// (comando Tauri) lo traduce a un 404 propio.
    pub async fn update_tts_provider(&self, id: &str, input: UpdateTtsProviderInput) -> Result<Option<TtsProviderEntry>, SettingsError> {
        let mut stored = self.load_stored_tts_providers().await?;
        let has_api_key = match input.api_key.as_deref() {
            Some(key) if !key.is_empty() => {
                self.secrets.set(SECRET_SERVICE_TTS, &secret_key_for_tts_provider(id), key).await?;
                true
            }
            Some(_) => {
                self.secrets.delete(SECRET_SERVICE_TTS, &secret_key_for_tts_provider(id)).await?;
                false
            }
            None => {
                let Some(entry) = stored.providers.iter().find(|p| p.id == id) else { return Ok(None) };
                entry.has_api_key
            }
        };

        let Some(entry) = stored.providers.iter_mut().find(|p| p.id == id) else { return Ok(None) };
        entry.label = input.label;
        entry.base_url = (entry.kind == TtsBackendKind::Http).then_some(input.base_url).flatten();
        entry.region = (entry.kind == TtsBackendKind::Azure).then_some(input.region).flatten();
        entry.model = matches!(entry.kind, TtsBackendKind::FishAudio | TtsBackendKind::Groq | TtsBackendKind::Together).then_some(input.model).flatten().filter(|m| !m.trim().is_empty());
        entry.has_api_key = has_api_key;
        let updated = entry.clone();

        self.set_json(KEY_TTS_PROVIDERS, &stored).await?;
        Ok(Some(updated.into()))
    }

    /// Borra la entrada y su secreto (`Native` se rechaza en el comando
    /// Tauri). Si era la "en uso", cae a otra activa si hay, o si no a
    /// `Native`, que siempre existe.
    pub async fn remove_tts_provider(&self, id: &str) -> Result<(), SettingsError> {
        let mut stored = self.load_stored_tts_providers().await?;
        stored.providers.retain(|p| p.id != id);
        self.secrets.delete(SECRET_SERVICE_TTS, &secret_key_for_tts_provider(id)).await?;
        if stored.current_provider_id.as_deref() == Some(id) {
            stored.current_provider_id = stored
                .providers
                .iter()
                .find(|p| p.enabled)
                .or_else(|| stored.providers.iter().find(|p| p.id == NATIVE_TTS_PROVIDER_ID))
                .map(|p| p.id.clone());
        }
        self.set_json(KEY_TTS_PROVIDERS, &stored).await
    }

    /// "Activar" (`Native` no se puede desactivar — rechazado en el
    /// comando Tauri). Sin campo "fijar" acá: no hay rotación automática.
    pub async fn set_tts_provider_enabled(&self, id: &str, enabled: bool) -> Result<Option<TtsProviderEntry>, SettingsError> {
        let mut stored = self.load_stored_tts_providers().await?;
        let Some(entry) = stored.providers.iter_mut().find(|p| p.id == id) else { return Ok(None) };
        entry.enabled = enabled;
        let updated = entry.clone();
        self.set_json(KEY_TTS_PROVIDERS, &stored).await?;
        Ok(Some(updated.into()))
    }

    /// "Usar ahora" — selección manual. El comando Tauri exige `enabled`
    /// antes de llamar esto; este método en sí no lo valida.
    pub async fn set_current_tts_provider(&self, id: &str) -> Result<Option<TtsProvidersSettings>, SettingsError> {
        let mut stored = self.load_stored_tts_providers().await?;
        if !stored.providers.iter().any(|p| p.id == id) {
            return Ok(None);
        }
        stored.current_provider_id = Some(id.to_string());
        self.set_json(KEY_TTS_PROVIDERS, &stored).await?;
        Ok(Some(stored.into()))
    }

    /// Lee la lista guardada, sembrando siempre el singleton `Native`
    /// (existe desde el primer arranque, haya o no una conexión vieja) y
    /// migrando desde `tts_connection` la primera vez que hace falta —
    /// mismo criterio que `load_stored_llm_providers`, con el paso extra
    /// de sembrar `Native`. La key vieja queda huérfana sin borrar.
    async fn load_stored_tts_providers(&self) -> Result<StoredTtsProviders, SettingsError> {
        if let Some(stored) = self.get_json::<StoredTtsProviders>(KEY_TTS_PROVIDERS).await? {
            return Ok(stored);
        }

        let native_entry = StoredTtsProviderEntry {
            id: NATIVE_TTS_PROVIDER_ID.to_string(),
            kind: TtsBackendKind::Native,
            label: default_label_for_tts_kind(TtsBackendKind::Native).to_string(),
            base_url: None,
            region: None,
            model: None,
            has_api_key: false,
            enabled: true,
        };

        let stored = match self.get_json::<StoredTtsConnection>(KEY_TTS_CONNECTION).await? {
            None => StoredTtsProviders { providers: vec![native_entry], current_provider_id: Some(NATIVE_TTS_PROVIDER_ID.to_string()) },
            Some(old) if old.backend == TtsBackendKind::Native => {
                StoredTtsProviders { providers: vec![native_entry], current_provider_id: Some(NATIVE_TTS_PROVIDER_ID.to_string()) }
            }
            Some(old) if old.backend == TtsBackendKind::ElevenLabs => {
                let id = Uuid::new_v4().to_string();
                if let Some(key) = self.secrets.get(SECRET_SERVICE_TTS, SECRET_KEY_ELEVENLABS_API_KEY).await? {
                    self.secrets.set(SECRET_SERVICE_TTS, &secret_key_for_tts_provider(&id), &key).await?;
                }
                let entry = StoredTtsProviderEntry {
                    id: id.clone(),
                    kind: TtsBackendKind::ElevenLabs,
                    label: default_label_for_tts_kind(TtsBackendKind::ElevenLabs).to_string(),
                    base_url: None,
                    region: None,
                    model: None,
                    has_api_key: old.has_elevenlabs_api_key,
                    enabled: true,
                };
                StoredTtsProviders { providers: vec![native_entry, entry], current_provider_id: Some(id) }
            }
            // El resto (`Http`, y los kinds nuevos que un blob viejo nunca
            // pudo tener como `backend`: `OpenAi`/`Google`/`Azure`) se
            // migra como `Http` — preserva el `base_url` que hubiera.
            Some(old) => {
                let id = Uuid::new_v4().to_string();
                if let Some(key) = self.secrets.get(SECRET_SERVICE_TTS, SECRET_KEY_API_KEY).await? {
                    self.secrets.set(SECRET_SERVICE_TTS, &secret_key_for_tts_provider(&id), &key).await?;
                }
                let entry = StoredTtsProviderEntry {
                    id: id.clone(),
                    kind: TtsBackendKind::Http,
                    label: default_label_for_tts_kind(TtsBackendKind::Http).to_string(),
                    base_url: Some(old.base_url),
                    region: None,
                    model: None,
                    has_api_key: old.has_api_key,
                    enabled: true,
                };
                StoredTtsProviders { providers: vec![native_entry, entry], current_provider_id: Some(id) }
            }
        };

        self.set_json(KEY_TTS_PROVIDERS, &stored).await?;
        Ok(stored)
    }

    /// Arma el `TtsBackend` real (motor + credenciales) del proveedor "en
    /// uso" ahora mismo — reemplaza la lectura manual + `match` que se
    /// repetía en 4 sitios (`chat`/`voice`/`call`/`twitch`). A diferencia
    /// de `build_llm_provider` (todavía pendiente), acá no hace falta
    /// ningún parámetro extra: sin rotación automática, no hay nada que
    /// publicar por el `EventBus`.
    pub async fn build_tts_backend(&self) -> Result<TtsBackend, SettingsError> {
        let stored = self.load_stored_tts_providers().await?;
        let current_id = stored
            .current_provider_id
            .as_deref()
            .ok_or_else(|| SettingsError::Unavailable("no hay ningún proveedor de voz configurado".to_string()))?;
        let entry = stored
            .providers
            .iter()
            .find(|p| p.id == current_id)
            .ok_or_else(|| SettingsError::Unavailable("el proveedor de voz en uso ya no existe".to_string()))?;

        Ok(match entry.kind {
            TtsBackendKind::Native => TtsBackend::Native,
            TtsBackendKind::ElevenLabs => {
                let api_key = self.secrets.get(SECRET_SERVICE_TTS, &secret_key_for_tts_provider(&entry.id)).await?.unwrap_or_default();
                TtsBackend::ElevenLabs { api_key }
            }
            TtsBackendKind::OpenAi => {
                let api_key = self.secrets.get(SECRET_SERVICE_TTS, &secret_key_for_tts_provider(&entry.id)).await?;
                TtsBackend::Http { base_url: OPENAI_BASE_URL.to_string(), api_key }
            }
            TtsBackendKind::Http => {
                let api_key = self.secrets.get(SECRET_SERVICE_TTS, &secret_key_for_tts_provider(&entry.id)).await?;
                TtsBackend::Http { base_url: entry.base_url.clone().unwrap_or_default(), api_key }
            }
            TtsBackendKind::Google => {
                let api_key = self.secrets.get(SECRET_SERVICE_TTS, &secret_key_for_tts_provider(&entry.id)).await?.unwrap_or_default();
                TtsBackend::Google { api_key }
            }
            TtsBackendKind::FishAudio => {
                let api_key = self.secrets.get(SECRET_SERVICE_TTS, &secret_key_for_tts_provider(&entry.id)).await?.unwrap_or_default();
                TtsBackend::FishAudio { api_key, model: entry.model.clone() }
            }
            TtsBackendKind::Groq => {
                let api_key = self.secrets.get(SECRET_SERVICE_TTS, &secret_key_for_tts_provider(&entry.id)).await?.unwrap_or_default();
                TtsBackend::Groq { api_key, model: entry.model.clone() }
            }
            TtsBackendKind::Together => {
                let api_key = self.secrets.get(SECRET_SERVICE_TTS, &secret_key_for_tts_provider(&entry.id)).await?.unwrap_or_default();
                TtsBackend::Together { api_key, model: entry.model.clone() }
            }
            TtsBackendKind::Deepgram => {
                let api_key = self.secrets.get(SECRET_SERVICE_TTS, &secret_key_for_tts_provider(&entry.id)).await?.unwrap_or_default();
                TtsBackend::Deepgram { api_key }
            }
            TtsBackendKind::Azure => {
                let api_key = self.secrets.get(SECRET_SERVICE_TTS, &secret_key_for_tts_provider(&entry.id)).await?.unwrap_or_default();
                let region = entry
                    .region
                    .clone()
                    .ok_or_else(|| SettingsError::Unavailable("falta configurar la región de Azure Speech".to_string()))?;
                TtsBackend::Azure { api_key, region }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use aivt_database::AppSettingsRepository;
    

    use super::*;
    use crate::service::test_service;

    /// Locks in the exact wire value the frontend's Zod schema and
    /// `<Select>` option both hardcode — plain `rename_all = "snake_case"`
    /// would otherwise produce `eleven_labs`, which is what actually broke
    /// this live (`unknown variant 'elevenlabs', expected ... 'eleven_labs'`).
    #[test]
    fn tts_backend_kind_serializes_elevenlabs_and_openai_as_one_word() {
        assert_eq!(serde_json::to_string(&TtsBackendKind::ElevenLabs).unwrap(), "\"elevenlabs\"");
        assert_eq!(serde_json::from_str::<TtsBackendKind>("\"elevenlabs\"").unwrap(), TtsBackendKind::ElevenLabs);
        assert_eq!(serde_json::to_string(&TtsBackendKind::OpenAi).unwrap(), "\"openai\"");
        assert_eq!(serde_json::from_str::<TtsBackendKind>("\"openai\"").unwrap(), TtsBackendKind::OpenAi);
    }

    #[tokio::test]
    async fn tts_providers_defaults_to_just_the_native_singleton() {
        let service = test_service().await;
        let providers = service.tts_providers().await.unwrap();
        assert_eq!(providers.providers.len(), 1);
        assert_eq!(providers.providers[0].id, NATIVE_TTS_PROVIDER_ID);
        assert_eq!(providers.providers[0].kind, TtsBackendKind::Native);
        assert!(providers.providers[0].enabled);
        assert_eq!(providers.current_provider_id.as_deref(), Some(NATIVE_TTS_PROVIDER_ID));
    }

    #[tokio::test]
    async fn build_tts_backend_defaults_to_native() {
        let service = test_service().await;
        assert!(matches!(service.build_tts_backend().await.unwrap(), TtsBackend::Native));
    }

    fn tts_update(api_key: Option<&str>) -> UpdateTtsProviderInput {
        UpdateTtsProviderInput { label: "ElevenLabs".to_string(), base_url: None, region: None, model: None, api_key: api_key.map(str::to_string) }
    }

    /// A diferencia de `add_llm_provider` (que sí mueve el puntero cuando
    /// agrega la primera entrada), acá `current_provider_id` ya apunta a
    /// `Native` desde el arranque — agregar un proveedor nuevo no cambia
    /// la selección actual, hace falta "usar ahora" explícito.
    #[tokio::test]
    async fn adding_a_provider_does_not_change_the_current_selection() {
        let service = test_service().await;
        let added = service
            .add_tts_provider(AddTtsProviderInput { kind: TtsBackendKind::ElevenLabs, label: None, base_url: None, region: None })
            .await
            .unwrap();

        assert_eq!(added.label, "ElevenLabs");
        let providers = service.tts_providers().await.unwrap();
        assert_eq!(providers.providers.len(), 2);
        assert_eq!(providers.current_provider_id.as_deref(), Some(NATIVE_TTS_PROVIDER_ID));
    }

    #[tokio::test]
    async fn only_http_kind_keeps_a_submitted_base_url() {
        let service = test_service().await;
        let added = service
            .add_tts_provider(AddTtsProviderInput { kind: TtsBackendKind::OpenAi, label: None, base_url: Some("http://ignored".to_string()), region: None })
            .await
            .unwrap();
        assert_eq!(added.label, "OpenAI");
        assert!(added.base_url.is_none());
    }

    #[tokio::test]
    async fn only_azure_kind_keeps_a_submitted_region() {
        let service = test_service().await;
        let google = service
            .add_tts_provider(AddTtsProviderInput { kind: TtsBackendKind::Google, label: None, base_url: None, region: Some("eastus".to_string()) })
            .await
            .unwrap();
        assert!(google.region.is_none());

        let azure = service
            .add_tts_provider(AddTtsProviderInput { kind: TtsBackendKind::Azure, label: None, base_url: None, region: Some("eastus".to_string()) })
            .await
            .unwrap();
        assert_eq!(azure.region.as_deref(), Some("eastus"));
    }

    #[tokio::test]
    async fn update_tts_provider_with_api_key_stores_it_in_secret_store_not_settings() {
        let service = test_service().await;
        let added = service
            .add_tts_provider(AddTtsProviderInput { kind: TtsBackendKind::ElevenLabs, label: None, base_url: None, region: None })
            .await
            .unwrap();
        service.set_current_tts_provider(&added.id).await.unwrap();

        let saved = service.update_tts_provider(&added.id, tts_update(Some("sk-el-secret"))).await.unwrap().unwrap();

        assert!(saved.has_api_key);
        match service.build_tts_backend().await.unwrap() {
            TtsBackend::ElevenLabs { api_key } => assert_eq!(api_key, "sk-el-secret"),
            other => panic!("expected ElevenLabs, got {other:?}"),
        }

        let raw = AppSettingsRepository::new(&service.pool).get(KEY_TTS_PROVIDERS).await.unwrap().unwrap();
        assert!(!raw.contains("sk-el-secret"));
    }

    #[tokio::test]
    async fn resaving_tts_provider_without_an_api_key_keeps_the_existing_one() {
        let service = test_service().await;
        let added = service
            .add_tts_provider(AddTtsProviderInput { kind: TtsBackendKind::ElevenLabs, label: None, base_url: None, region: None })
            .await
            .unwrap();
        service.update_tts_provider(&added.id, tts_update(Some("sk-secret"))).await.unwrap();

        let saved = service.update_tts_provider(&added.id, tts_update(None)).await.unwrap().unwrap();

        assert!(saved.has_api_key);
    }

    #[tokio::test]
    async fn saving_an_empty_tts_api_key_clears_it() {
        let service = test_service().await;
        let added = service
            .add_tts_provider(AddTtsProviderInput { kind: TtsBackendKind::ElevenLabs, label: None, base_url: None, region: None })
            .await
            .unwrap();
        service.update_tts_provider(&added.id, tts_update(Some("sk-secret"))).await.unwrap();

        let saved = service.update_tts_provider(&added.id, tts_update(Some(""))).await.unwrap().unwrap();

        assert!(!saved.has_api_key);
    }

    #[tokio::test]
    async fn two_tts_providers_have_independent_secret_slots() {
        let service = test_service().await;
        let a = service
            .add_tts_provider(AddTtsProviderInput { kind: TtsBackendKind::ElevenLabs, label: None, base_url: None, region: None })
            .await
            .unwrap();
        let b = service
            .add_tts_provider(AddTtsProviderInput { kind: TtsBackendKind::Google, label: None, base_url: None, region: None })
            .await
            .unwrap();

        service.update_tts_provider(&a.id, tts_update(Some("sk-a"))).await.unwrap();
        service.update_tts_provider(&b.id, tts_update(Some("sk-b"))).await.unwrap();

        let providers = service.tts_providers().await.unwrap().providers;
        assert!(providers.iter().filter(|p| p.id == a.id || p.id == b.id).all(|p| p.has_api_key));
    }

    #[tokio::test]
    async fn removing_the_current_tts_provider_falls_back_to_another_enabled_one() {
        let service = test_service().await;
        let a = service
            .add_tts_provider(AddTtsProviderInput { kind: TtsBackendKind::ElevenLabs, label: None, base_url: None, region: None })
            .await
            .unwrap();
        let b = service
            .add_tts_provider(AddTtsProviderInput { kind: TtsBackendKind::Google, label: None, base_url: None, region: None })
            .await
            .unwrap();
        service.set_current_tts_provider(&a.id).await.unwrap();

        service.remove_tts_provider(&a.id).await.unwrap();

        let providers = service.tts_providers().await.unwrap();
        assert_ne!(providers.current_provider_id.as_deref(), Some(a.id.as_str()));
        assert!(providers.providers.iter().any(|p| p.id == b.id));
    }

    #[tokio::test]
    async fn removing_the_last_real_provider_falls_back_to_native() {
        let service = test_service().await;
        let added = service
            .add_tts_provider(AddTtsProviderInput { kind: TtsBackendKind::ElevenLabs, label: None, base_url: None, region: None })
            .await
            .unwrap();
        service.set_current_tts_provider(&added.id).await.unwrap();

        service.remove_tts_provider(&added.id).await.unwrap();

        let providers = service.tts_providers().await.unwrap();
        assert_eq!(providers.current_provider_id.as_deref(), Some(NATIVE_TTS_PROVIDER_ID));
        assert_eq!(providers.providers.len(), 1);
    }

    #[tokio::test]
    async fn set_current_tts_provider_moves_the_pointer() {
        let service = test_service().await;
        let added = service
            .add_tts_provider(AddTtsProviderInput { kind: TtsBackendKind::ElevenLabs, label: None, base_url: None, region: None })
            .await
            .unwrap();
        assert_eq!(service.tts_providers().await.unwrap().current_provider_id.as_deref(), Some(NATIVE_TTS_PROVIDER_ID));

        service.set_current_tts_provider(&added.id).await.unwrap();

        assert_eq!(service.tts_providers().await.unwrap().current_provider_id.as_deref(), Some(added.id.as_str()));
    }

    #[tokio::test]
    async fn migrating_with_no_old_connection_just_seeds_native() {
        let service = test_service().await;
        let providers = service.tts_providers().await.unwrap();
        assert_eq!(providers.providers.len(), 1);
        assert_eq!(providers.providers[0].kind, TtsBackendKind::Native);
    }

    #[tokio::test]
    async fn migrating_an_old_native_connection_keeps_native_current() {
        let service = test_service().await;
        service.set_json(KEY_TTS_CONNECTION, &serde_json::json!({"backend": "native", "baseUrl": "", "hasApiKey": false})).await.unwrap();

        let providers = service.tts_providers().await.unwrap();
        assert_eq!(providers.providers.len(), 1);
        assert_eq!(providers.current_provider_id.as_deref(), Some(NATIVE_TTS_PROVIDER_ID));
    }

    /// Preserva exactamente el comportamiento de antes de este plan: la
    /// conexión ElevenLabs que el usuario ya tenía en uso no se pierde ni
    /// cambia de comportamiento, y su key migra a su propio slot.
    #[tokio::test]
    async fn migrating_an_old_elevenlabs_connection_preserves_its_key() {
        let service = test_service().await;
        service
            .set_json(KEY_TTS_CONNECTION, &serde_json::json!({"backend": "elevenlabs", "baseUrl": "", "hasApiKey": false, "hasElevenlabsApiKey": true}))
            .await
            .unwrap();
        service.secrets.set(SECRET_SERVICE_TTS, SECRET_KEY_ELEVENLABS_API_KEY, "el-secret").await.unwrap();

        let providers = service.tts_providers().await.unwrap();
        assert_eq!(providers.providers.len(), 2);
        let entry = providers.providers.iter().find(|p| p.kind == TtsBackendKind::ElevenLabs).unwrap();
        assert!(entry.has_api_key);
        assert_eq!(providers.current_provider_id.as_deref(), Some(entry.id.as_str()));
        match service.build_tts_backend().await.unwrap() {
            TtsBackend::ElevenLabs { api_key } => assert_eq!(api_key, "el-secret"),
            other => panic!("expected ElevenLabs, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn migrating_an_old_http_connection_preserves_its_base_url_and_key() {
        let service = test_service().await;
        service
            .set_json(KEY_TTS_CONNECTION, &serde_json::json!({"backend": "http", "baseUrl": "http://localhost:8880/v1", "hasApiKey": true}))
            .await
            .unwrap();
        service.secrets.set(SECRET_SERVICE_TTS, SECRET_KEY_API_KEY, "http-secret").await.unwrap();

        let providers = service.tts_providers().await.unwrap();
        assert_eq!(providers.providers.len(), 2);
        let entry = providers.providers.iter().find(|p| p.kind == TtsBackendKind::Http).unwrap();
        assert_eq!(entry.base_url.as_deref(), Some("http://localhost:8880/v1"));
        assert!(entry.has_api_key);
        assert_eq!(providers.current_provider_id.as_deref(), Some(entry.id.as_str()));
    }

    /// A connection saved before `hasElevenlabsApiKey` existed (only the
    /// original three keys in the stored JSON) must still migrate without
    /// crashing.
    #[tokio::test]
    async fn migrating_a_connection_saved_before_elevenlabs_existed_still_works() {
        let service = test_service().await;
        AppSettingsRepository::new(&service.pool)
            .set(KEY_TTS_CONNECTION, r#"{"backend":"http","baseUrl":"http://localhost:8880/v1","hasApiKey":true}"#)
            .await
            .unwrap();

        let providers = service.tts_providers().await.unwrap();
        let entry = providers.providers.iter().find(|p| p.kind == TtsBackendKind::Http).unwrap();
        assert_eq!(entry.base_url.as_deref(), Some("http://localhost:8880/v1"));
        assert!(entry.has_api_key);
    }

    #[test]
    fn fish_audio_and_deepgram_tts_kinds_serialize_as_one_word() {
        assert_eq!(serde_json::to_string(&TtsBackendKind::FishAudio).unwrap(), "\"fishaudio\"");
        assert_eq!(serde_json::to_string(&TtsBackendKind::Deepgram).unwrap(), "\"deepgram\"");
        assert_eq!(serde_json::from_str::<TtsBackendKind>("\"fishaudio\"").unwrap(), TtsBackendKind::FishAudio);
    }
}
