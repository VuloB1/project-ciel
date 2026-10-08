//! Proveedores de IA (LLM): lista, proveedor en uso, rotación automática y migración desde la conexión única.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use aivt_eventbus::{AppEvent, EventBus};
use aivt_provider_llm::{
    AnthropicProvider, FailoverLLMProvider, FailoverReason, FailoverReport, GeminiProvider, LLMProvider, LlmPoolEntry, OpenAiCompatibleProvider,
    PrivacyGuardProvider, PrivacyMode,
};

use crate::service::{SettingsService, SECRET_KEY_API_KEY};
use crate::usage::UsageRecordingProvider;
use crate::SettingsError;

/// Reemplazada por `KEY_LLM_PROVIDERS` (Fase de multi-proveedor) — se deja
/// solo como origen de la migración de instalaciones existentes, ver
/// `SettingsService::load_stored_llm_providers`. Nunca se vuelve a escribir.
const KEY_LLM_CONNECTION: &str = "llm_connection";

/// v1: reemplaza la conexión única de arriba por una lista de proveedores +
/// cuál está "en uso" — key nueva (no se muta `llm_connection` in place)
/// porque el shape cambió de un registro a una lista+puntero, mismo
/// criterio que `KEY_APPEARANCE_SETTINGS`'s `_v2`.
const KEY_LLM_PROVIDERS: &str = "llm_providers_v1";

const SECRET_SERVICE_LLM: &str = "aivt.llm_provider";

pub(crate) const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

const ANTHROPIC_BASE_URL: &str = "https://api.anthropic.com/v1";

const GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

/// What's actually saved in `app_settings` for the LLM connection — never
/// the API key itself, only whether one is on file.
///
/// `#[serde(default)]` on `vision_enabled` matters: it lets settings blobs
/// saved before Fase 3 (which don't have the field at all) keep
/// deserializing instead of erroring on load.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredLlmConnection {
    base_url: String,
    model: String,
    temperature: f32,
    max_tokens: u32,
    has_api_key: bool,
    #[serde(default)]
    vision_enabled: bool,
}

/// What the frontend reads — same shape, kept as a separate type so it's
/// obvious at a glance that this never carries a secret.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmConnectionSettings {
    pub base_url: String,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub has_api_key: bool,
    /// Manual override, not auto-detected: generic OpenAI-compatible
    /// servers don't reliably expose whether the loaded model actually
    /// supports image input, so the user says so explicitly instead of the
    /// app guessing (sección 84-85 del brief — "nunca se asume, se puede
    /// corregir"). Gates the attach-image control in the chat composer.
    pub vision_enabled: bool,
}

/// Qué tipo de proveedor es una entrada. `OpenAi`/`Anthropic`/`Gemini`
/// hablan con un endpoint fijo y propio (no piden `base_url` al usuario);
/// `Generic` es el formulario de siempre — cualquier servidor
/// OpenAI-compatible (LM Studio, Ollama, uno propio), con `base_url` a
/// mano. Un único proveedor por kind salvo `Generic` (sin límite) — la
/// regla vive en el comando Tauri, no acá (ver `commands::settings`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmProviderKind {
    #[serde(rename = "openai")]
    OpenAi,
    Anthropic,
    Gemini,
    /// Desde acá hasta `DeepSeek`: servicios que hablan exactamente el
    /// protocolo de OpenAI contra una URL fija y propia — no necesitan
    /// adaptador, solo su endpoint (ver `openai_compatible_base_url`).
    Groq,
    #[serde(rename = "openrouter")]
    OpenRouter,
    Cerebras,
    Mistral,
    #[serde(rename = "deepseek")]
    DeepSeek,
    #[serde(rename = "together")]
    Together,
    #[serde(rename = "xai")]
    Xai,
    #[serde(rename = "sambanova")]
    SambaNova,
    Generic,
}

impl LlmProviderKind {
    /// El endpoint fijo de los kinds que hablan el protocolo de OpenAI sin
    /// pedirle la URL al usuario; `None` para `Anthropic`/`Gemini` (adaptador
    /// propio) y `Generic` (URL a mano). Fuente: la documentación oficial de
    /// cada servicio.
    pub fn openai_compatible_base_url(self) -> Option<&'static str> {
        match self {
            LlmProviderKind::OpenAi => Some(OPENAI_BASE_URL),
            LlmProviderKind::Groq => Some("https://api.groq.com/openai/v1"),
            LlmProviderKind::OpenRouter => Some("https://openrouter.ai/api/v1"),
            LlmProviderKind::Cerebras => Some("https://api.cerebras.ai/v1"),
            LlmProviderKind::Mistral => Some("https://api.mistral.ai/v1"),
            LlmProviderKind::DeepSeek => Some("https://api.deepseek.com/v1"),
            LlmProviderKind::Together => Some("https://api.together.ai/v1"),
            LlmProviderKind::Xai => Some("https://api.x.ai/v1"),
            LlmProviderKind::SambaNova => Some("https://api.sambanova.ai/v1"),
            LlmProviderKind::Anthropic | LlmProviderKind::Gemini | LlmProviderKind::Generic => None,
        }
    }
}

fn default_label_for_kind(kind: LlmProviderKind) -> &'static str {
    match kind {
        LlmProviderKind::OpenAi => "OpenAI",
        LlmProviderKind::Anthropic => "Anthropic Claude",
        LlmProviderKind::Gemini => "Google Gemini",
        LlmProviderKind::Groq => "Groq",
        LlmProviderKind::OpenRouter => "OpenRouter",
        LlmProviderKind::Cerebras => "Cerebras",
        LlmProviderKind::Mistral => "Mistral",
        LlmProviderKind::DeepSeek => "DeepSeek",
        LlmProviderKind::Together => "Together AI",
        LlmProviderKind::Xai => "xAI Grok",
        LlmProviderKind::SambaNova => "SambaNova",
        LlmProviderKind::Generic => "Servidor local",
    }
}

/// El endpoint real al que este provider habla — fijo para los 3 kinds
/// cloud, lo que el usuario escribió para `Generic`.
fn resolved_base_url(entry: &StoredLlmProviderEntry) -> String {
    match entry.kind {
        LlmProviderKind::Anthropic => ANTHROPIC_BASE_URL.to_string(),
        LlmProviderKind::Gemini => GEMINI_BASE_URL.to_string(),
        LlmProviderKind::Generic => entry.base_url.clone().unwrap_or_default(),
        kind => kind.openai_compatible_base_url().unwrap_or_default().to_string(),
    }
}

/// Un proveedor fijado pero todavía a medio configurar (sin modelo, o de
/// nube sin API key) no puede responder: el router lo saltaría igual al
/// fallar, pero antes le mandaría una petición condenada (a la API real de
/// OpenAI/Anthropic/Gemini, con la key vacía), así que ni entra al pool. Solo
/// aplica a los de reserva; el proveedor "en uso" siempre se intenta, y si
/// falla su error se le muestra al usuario tal cual. `Generic` no exige key
/// (LM Studio, Ollama).
fn is_usable_fallback(entry: &StoredLlmProviderEntry) -> bool {
    if entry.model.trim().is_empty() {
        return false;
    }
    entry.kind == LlmProviderKind::Generic || entry.has_api_key
}

/// Proveedores a los que no se les hace caso un rato después de fallar. Sin esto,
/// con el preferido apagado cada mensaje empezaría esperándolo (y un servidor
/// *colgado* haría esperar 45 s por mensaje). Estático y no un campo porque hay
/// instancias de `SettingsService` que se crean al vuelo en cada llamada.
static LLM_COOLDOWNS: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, std::time::Instant>>> =
    std::sync::OnceLock::new();

fn llm_cooldowns() -> &'static std::sync::Mutex<std::collections::HashMap<String, std::time::Instant>> {
    LLM_COOLDOWNS.get_or_init(Default::default)
}

fn mark_llm_provider_cooling_down(id: &str, duration: std::time::Duration) {
    if duration.is_zero() {
        return;
    }
    llm_cooldowns().lock().expect("llm cooldowns poisoned").insert(id.to_string(), std::time::Instant::now() + duration);
}

fn clear_llm_provider_cooldown(id: &str) {
    llm_cooldowns().lock().expect("llm cooldowns poisoned").remove(id);
}

fn llm_provider_is_cooling_down(id: &str) -> bool {
    let mut cooldowns = llm_cooldowns().lock().expect("llm cooldowns poisoned");
    match cooldowns.get(id) {
        Some(until) if *until > std::time::Instant::now() => true,
        Some(_) => {
            cooldowns.remove(id);
            false
        }
        None => false,
    }
}

/// Cuánto se espera antes de volver a probar un proveedor que falló, según por
/// qué falló: un servidor apagado puede volver pronto; uno que no responde
/// cuesta 45 s cada intento, así que se le da más margen; sin crédito no se
/// arregla solo en minutos.
fn cooldown_for(reason: FailoverReason) -> std::time::Duration {
    use std::time::Duration;
    match reason {
        FailoverReason::Unreachable => Duration::from_secs(60),
        FailoverReason::RateLimited => Duration::from_secs(90),
        FailoverReason::ServerError => Duration::from_secs(120),
        FailoverReason::Timeout => Duration::from_secs(300),
        FailoverReason::QuotaExceeded => Duration::from_secs(600),
        FailoverReason::Restored => Duration::ZERO,
    }
}

/// El orden en que se prueban los proveedores para una petición: primero el
/// preferido del usuario, luego el que esté en uso (si es otro, un temporal), y
/// después las reservas activas, fijadas y en condiciones de responder. Los que
/// están en pausa tras un fallo reciente pasan al final (sin quitarlos: si todo lo
/// demás falla, mejor intentarlos que no responder).
///
/// Poner al preferido primero es la recuperación automática: cuando vuelve a
/// responder, la siguiente petición lo usa y se avisa del cambio.
fn order_llm_pool<'a>(stored: &'a StoredLlmProviders, cooling_down: &dyn Fn(&str) -> bool) -> Vec<&'a StoredLlmProviderEntry> {
    let find = |id: &str| stored.providers.iter().find(|p| p.id == id);
    let mut ordered: Vec<&StoredLlmProviderEntry> = Vec::new();
    let push = |entry: &'a StoredLlmProviderEntry, ordered: &mut Vec<&'a StoredLlmProviderEntry>| {
        if !ordered.iter().any(|e| e.id == entry.id) {
            ordered.push(entry);
        }
    };

    if let Some(entry) = stored.effective_preferred_id().and_then(find) {
        push(entry, &mut ordered);
    }
    if let Some(entry) = stored.current_provider_id.as_deref().and_then(find) {
        push(entry, &mut ordered);
    }
    for entry in stored.providers.iter().filter(|p| p.enabled && p.pinned && is_usable_fallback(p)) {
        push(entry, &mut ordered);
    }

    let (healthy, resting): (Vec<_>, Vec<_>) = ordered.into_iter().partition(|e| !cooling_down(&e.id));
    healthy.into_iter().chain(resting).collect()
}

/// Un slot de secreto por proveedor en vez del único `SECRET_KEY_API_KEY`
/// de antes — extiende el precedente ya usado por TTS (`api_key` vs
/// `elevenlabs_api_key`, dos slots fijos) a N slots dinámicos, uno por
/// `id` de proveedor, para que agregar/editar uno nunca pise la key de
/// otro.
fn secret_key_for_provider(id: &str) -> String {
    format!("api_key.{id}")
}

/// Lo que persiste `app_settings` bajo `KEY_LLM_PROVIDERS` para una sola
/// entrada — nunca el secreto en sí, solo `has_api_key` (mismo principio
/// que `StoredLlmConnection`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredLlmProviderEntry {
    id: String,
    kind: LlmProviderKind,
    label: String,
    base_url: Option<String>,
    model: String,
    temperature: f32,
    max_tokens: u32,
    has_api_key: bool,
    vision_enabled: bool,
    /// "Activar" — disponible para elegir a mano en cualquier momento.
    enabled: bool,
    /// "Fijar" — además de activo, entra en la rotación automática que
    /// recorre `FailoverLLMProvider` cuando el proveedor en uso se queda
    /// sin créditos. Solo tiene efecto real si `enabled` también es
    /// `true` (ver `SettingsService::set_llm_provider_enabled/pinned`).
    pinned: bool,
    /// Qué datos del usuario recibe este proveedor además de la conversación.
    /// `#[serde(default)]` = `Full` en datos guardados antes de existir el campo:
    /// nada cambia para quien ya tenía proveedores.
    #[serde(default)]
    privacy_mode: PrivacyMode,
}

/// La lista completa + cuál está "en uso" ahora mismo — ese puntero lo
/// mueve tanto una selección manual ("usar ahora") como, más adelante, la
/// rotación automática por falta de crédito.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct StoredLlmProviders {
    #[serde(default)]
    providers: Vec<StoredLlmProviderEntry>,
    /// El que está respondiendo ahora. Puede ser uno temporal: cuando el preferido
    /// falla, el cambio automático mueve SOLO este puntero.
    #[serde(default)]
    current_provider_id: Option<String>,
    /// El que el usuario eligió ("usar ahora") — al que se vuelve solo cuando
    /// responde de nuevo. `None` en datos guardados antes de existir este campo:
    /// equivale a "el que esté en uso".
    #[serde(default)]
    preferred_provider_id: Option<String>,
}

impl StoredLlmProviders {
    /// El proveedor preferido real: el elegido si todavía existe y sigue activo;
    /// si no, el que esté en uso.
    fn effective_preferred_id(&self) -> Option<&str> {
        self.preferred_provider_id
            .as_deref()
            .filter(|id| self.providers.iter().any(|p| p.id == *id && p.enabled))
            .or(self.current_provider_id.as_deref())
    }
}

/// Lo que ve el frontend — mismo shape que `StoredLlmProviderEntry`, tipo
/// separado por la misma razón que `LlmConnectionSettings`: deja obvio a
/// simple vista que nunca lleva un secreto.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmProviderEntry {
    pub id: String,
    pub kind: LlmProviderKind,
    pub label: String,
    pub base_url: Option<String>,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub has_api_key: bool,
    pub vision_enabled: bool,
    pub enabled: bool,
    pub pinned: bool,
    pub privacy_mode: PrivacyMode,
    /// `true` si este proveedor corre en tu propia red (LM Studio, Ollama en
    /// `localhost`, un equipo de tu casa): lo que se le manda no sale a internet.
    /// Todo proveedor en la nube y cualquier servidor remoto da `false`.
    pub is_local: bool,
}

/// `true` si la URL apunta a esta máquina o a una dirección de red privada. Solo
/// los servidores genéricos pueden serlo: OpenAI, Anthropic, Gemini y el resto
/// tienen un endpoint fijo en internet.
fn base_url_is_local(base_url: &str) -> bool {
    let without_scheme = base_url.split("://").nth(1).unwrap_or(base_url);
    let authority = without_scheme.split(['/', '?', '#']).next().unwrap_or("");
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    let host = if let Some(rest) = authority.strip_prefix('[') {
        rest.split(']').next().unwrap_or("")
    } else {
        authority.split(':').next().unwrap_or("")
    }
    .to_ascii_lowercase();

    if host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local") || host == "::1" || host == "0.0.0.0" {
        return true;
    }
    let octets: Vec<u8> = host.split('.').filter_map(|part| part.parse::<u8>().ok()).collect();
    if octets.len() != 4 || host.split('.').count() != 4 {
        return false;
    }
    matches!(octets[..], [127, ..] | [10, ..] | [192, 168, ..] | [169, 254, ..]) || (octets[0] == 172 && (16..=31).contains(&octets[1]))
}

impl LlmProviderEntry {
    /// Si este proveedor está en condiciones de responder: tiene modelo y, si es de
    /// nube, API key. Misma regla que usa el cambio automático para admitir una
    /// reserva en el pool (`is_usable_fallback`); el centro de diagnóstico la usa para
    /// avisar de un proveedor a medio configurar.
    pub fn is_ready(&self) -> bool {
        !self.model.trim().is_empty() && (self.kind == LlmProviderKind::Generic || self.has_api_key)
    }
}

impl From<StoredLlmProviderEntry> for LlmProviderEntry {
    fn from(e: StoredLlmProviderEntry) -> Self {
        let is_local = e.kind == LlmProviderKind::Generic && e.base_url.as_deref().is_some_and(base_url_is_local);
        Self {
            id: e.id,
            kind: e.kind,
            label: e.label,
            base_url: e.base_url,
            model: e.model,
            temperature: e.temperature,
            max_tokens: e.max_tokens,
            has_api_key: e.has_api_key,
            vision_enabled: e.vision_enabled,
            enabled: e.enabled,
            pinned: e.pinned,
            privacy_mode: e.privacy_mode,
            is_local,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmProvidersSettings {
    pub providers: Vec<LlmProviderEntry>,
    /// El que está respondiendo ahora (puede ser uno temporal).
    pub current_provider_id: Option<String>,
    /// El que el usuario eligió; si es distinto del anterior, se está usando uno
    /// temporal y se vuelve al preferido solo, cuando responda de nuevo.
    pub preferred_provider_id: Option<String>,
}

impl From<StoredLlmProviders> for LlmProvidersSettings {
    fn from(s: StoredLlmProviders) -> Self {
        let preferred_provider_id = s.effective_preferred_id().map(str::to_string);
        Self { providers: s.providers.into_iter().map(Into::into).collect(), current_provider_id: s.current_provider_id, preferred_provider_id }
    }
}

/// Lo que escribe el frontend al agregar un proveedor nuevo. `base_url`
/// solo se usa (y se guarda) para `Generic` — los otros 3 kinds hablan
/// siempre con su endpoint fijo, lo manden o no.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddLlmProviderInput {
    pub kind: LlmProviderKind,
    pub label: Option<String>,
    pub base_url: Option<String>,
}

/// Lo que escribe el frontend al editar una entrada existente. Mismo
/// contrato de `api_key` que `LlmConnectionInput` tenía: `None` deja la key
/// guardada sin tocar, `Some("")` la borra, `Some(key)` la guarda.
///
/// No `Debug` derive — misma razón que `UpdateTtsProviderInput`: carga un
/// `api_key` en texto plano, y no derivar `Debug` hace que un futuro
/// `{:?}`/`?field` de log no compile en vez de filtrarlo en silencio.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLlmProviderInput {
    pub label: String,
    pub base_url: Option<String>,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub api_key: Option<String>,
    pub vision_enabled: bool,
    /// `None` deja el modo como estaba. Con `Minimal` la visión se apaga sola:
    /// ese modo no manda imágenes, y dejar el interruptor encendido haría que el
    /// chat ofreciera adjuntar una imagen que el modelo nunca vería.
    #[serde(default)]
    pub privacy_mode: Option<PrivacyMode>,
}

impl SettingsService {
    /// Segundos que le faltan a un proveedor para salir de su pausa tras fallar
    /// (`None` si no está en pausa). Solo lectura, para el centro de diagnóstico.
    pub fn llm_provider_cooldown_remaining_secs(id: &str) -> Option<u64> {
        let cooldowns = llm_cooldowns().lock().expect("llm cooldowns poisoned");
        cooldowns.get(id).and_then(|until| until.checked_duration_since(std::time::Instant::now())).map(|left| left.as_secs().max(1))
    }

    /// Lee la lista completa de proveedores configurados + cuál está en
    /// uso. Migra sola, la primera vez que hace falta, desde la vieja
    /// conexión única (`KEY_LLM_CONNECTION`) — ver `load_stored_llm_providers`.
    pub async fn llm_providers(&self) -> Result<LlmProvidersSettings, SettingsError> {
        Ok(self.load_stored_llm_providers().await?.into())
    }

    /// Agrega una entrada nueva. La regla de "un solo OpenAI/Anthropic/
    /// Gemini a la vez" vive en el comando Tauri (`commands::settings`),
    /// no acá — este método siempre agrega. La primera entrada agregada
    /// queda automáticamente "en uso".
    pub async fn add_llm_provider(&self, input: AddLlmProviderInput) -> Result<LlmProviderEntry, SettingsError> {
        let mut stored = self.load_stored_llm_providers().await?;
        let id = Uuid::new_v4().to_string();
        let label = input.label.filter(|l| !l.is_empty()).unwrap_or_else(|| default_label_for_kind(input.kind).to_string());
        let entry = StoredLlmProviderEntry {
            id: id.clone(),
            kind: input.kind,
            label,
            base_url: (input.kind == LlmProviderKind::Generic).then_some(input.base_url).flatten(),
            model: String::new(),
            temperature: 0.7,
            max_tokens: 1024,
            has_api_key: false,
            vision_enabled: false,
            enabled: true,
            pinned: false,
            privacy_mode: PrivacyMode::Full,
        };
        stored.providers.push(entry.clone());
        if stored.current_provider_id.is_none() {
            stored.current_provider_id = Some(id);
        }
        self.set_json(KEY_LLM_PROVIDERS, &stored).await?;
        Ok(entry.into())
    }

    /// `Ok(None)` si `id` no corresponde a ninguna entrada — el llamador
    /// (comando Tauri) lo traduce a un 404 propio.
    pub async fn update_llm_provider(&self, id: &str, input: UpdateLlmProviderInput) -> Result<Option<LlmProviderEntry>, SettingsError> {
        let mut stored = self.load_stored_llm_providers().await?;
        let has_api_key = match input.api_key.as_deref() {
            Some(key) if !key.is_empty() => {
                self.secrets.set(SECRET_SERVICE_LLM, &secret_key_for_provider(id), key).await?;
                true
            }
            Some(_) => {
                self.secrets.delete(SECRET_SERVICE_LLM, &secret_key_for_provider(id)).await?;
                false
            }
            None => {
                let Some(entry) = stored.providers.iter().find(|p| p.id == id) else { return Ok(None) };
                entry.has_api_key
            }
        };

        let Some(entry) = stored.providers.iter_mut().find(|p| p.id == id) else { return Ok(None) };
        entry.label = input.label;
        entry.base_url = (entry.kind == LlmProviderKind::Generic).then_some(input.base_url).flatten();
        entry.model = input.model;
        entry.temperature = input.temperature;
        entry.max_tokens = input.max_tokens;
        entry.has_api_key = has_api_key;
        if let Some(mode) = input.privacy_mode {
            entry.privacy_mode = mode;
        }
        entry.vision_enabled = input.vision_enabled && entry.privacy_mode.allows_images();
        let updated = entry.clone();

        self.set_json(KEY_LLM_PROVIDERS, &stored).await?;
        Ok(Some(updated.into()))
    }

    /// Borra la entrada y su secreto. Si era la "en uso", cae a otra
    /// activa si hay, o a la primera que quede, o a ninguna.
    pub async fn remove_llm_provider(&self, id: &str) -> Result<(), SettingsError> {
        let mut stored = self.load_stored_llm_providers().await?;
        stored.providers.retain(|p| p.id != id);
        self.secrets.delete(SECRET_SERVICE_LLM, &secret_key_for_provider(id)).await?;
        aivt_database::LlmUsageRepository::new(&self.pool).delete_for_provider(id).await?;
        if stored.current_provider_id.as_deref() == Some(id) {
            stored.current_provider_id =
                stored.providers.iter().find(|p| p.enabled).or_else(|| stored.providers.first()).map(|p| p.id.clone());
        }
        if stored.preferred_provider_id.as_deref() == Some(id) {
            stored.preferred_provider_id = None;
        }
        self.set_json(KEY_LLM_PROVIDERS, &stored).await
    }

    /// "Activar". Apagarlo también apaga "fijar" — fijado sin activado no
    /// tiene sentido (ver doc de `StoredLlmProviderEntry::pinned`).
    pub async fn set_llm_provider_enabled(&self, id: &str, enabled: bool) -> Result<Option<LlmProviderEntry>, SettingsError> {
        let mut stored = self.load_stored_llm_providers().await?;
        let Some(entry) = stored.providers.iter_mut().find(|p| p.id == id) else { return Ok(None) };
        entry.enabled = enabled;
        if !enabled {
            entry.pinned = false;
        }
        let updated = entry.clone();
        self.set_json(KEY_LLM_PROVIDERS, &stored).await?;
        Ok(Some(updated.into()))
    }

    /// "Fijar" — solo tiene efecto si la entrada ya está activa.
    pub async fn set_llm_provider_pinned(&self, id: &str, pinned: bool) -> Result<Option<LlmProviderEntry>, SettingsError> {
        let mut stored = self.load_stored_llm_providers().await?;
        let Some(entry) = stored.providers.iter_mut().find(|p| p.id == id) else { return Ok(None) };
        entry.pinned = pinned && entry.enabled;
        let updated = entry.clone();
        self.set_json(KEY_LLM_PROVIDERS, &stored).await?;
        Ok(Some(updated.into()))
    }

    /// "Usar ahora" — selección manual. El comando Tauri exige `enabled`
    /// antes de llamar esto; este método en sí no lo valida.
    pub async fn set_current_llm_provider(&self, id: &str) -> Result<Option<LlmProvidersSettings>, SettingsError> {
        let mut stored = self.load_stored_llm_providers().await?;
        if !stored.providers.iter().any(|p| p.id == id) {
            return Ok(None);
        }
        // Una elección manual es la nueva preferencia: se usa ya y es a la que se
        // vuelve después de cualquier cambio automático. Y se la deja de dar por
        // caída: si el usuario la pide, se intenta.
        stored.current_provider_id = Some(id.to_string());
        stored.preferred_provider_id = Some(id.to_string());
        clear_llm_provider_cooldown(id);
        self.set_json(KEY_LLM_PROVIDERS, &stored).await?;
        Ok(Some(stored.into()))
    }

    /// Mueve solo el "en uso" (lo que usa el cambio automático): el preferido del
    /// usuario no se toca, así se vuelve a él cuando responda de nuevo.
    async fn set_active_llm_provider(&self, id: &str) -> Result<(), SettingsError> {
        let mut stored = self.load_stored_llm_providers().await?;
        if !stored.providers.iter().any(|p| p.id == id) {
            return Ok(());
        }
        // Congela la preferencia antes de mover el puntero: en datos viejos
        // (`preferred_provider_id: None`) "preferido" era "el que esté en uso", y moverlo
        // la perdería.
        if stored.preferred_provider_id.is_none() {
            stored.preferred_provider_id = stored.current_provider_id.clone();
        }
        stored.current_provider_id = Some(id.to_string());
        self.set_json(KEY_LLM_PROVIDERS, &stored).await
    }

    /// Lee la lista guardada, migrando desde la vieja conexión única la
    /// primera vez que hace falta: si `KEY_LLM_PROVIDERS` todavía no
    /// existe pero hay un `KEY_LLM_CONNECTION` viejo, se convierte en una
    /// sola entrada `Generic`, activa y fijada (mismo comportamiento
    /// efectivo que tenía antes: la única conexión que había siempre se
    /// usaba), moviendo su key al nuevo slot por-id. La key vieja queda
    /// huérfana sin borrar — mismo espíritu que `KEY_APPEARANCE_SETTINGS`.
    async fn load_stored_llm_providers(&self) -> Result<StoredLlmProviders, SettingsError> {
        if let Some(stored) = self.get_json::<StoredLlmProviders>(KEY_LLM_PROVIDERS).await? {
            return Ok(stored);
        }
        if let Some(old) = self.get_json::<StoredLlmConnection>(KEY_LLM_CONNECTION).await? {
            let id = Uuid::new_v4().to_string();
            if let Some(key) = self.secrets.get(SECRET_SERVICE_LLM, SECRET_KEY_API_KEY).await? {
                self.secrets.set(SECRET_SERVICE_LLM, &secret_key_for_provider(&id), &key).await?;
            }
            let entry = StoredLlmProviderEntry {
                id: id.clone(),
                kind: LlmProviderKind::Generic,
                label: default_label_for_kind(LlmProviderKind::Generic).to_string(),
                base_url: Some(old.base_url),
                model: old.model,
                temperature: old.temperature,
                max_tokens: old.max_tokens,
                has_api_key: old.has_api_key,
                vision_enabled: old.vision_enabled,
                enabled: true,
                pinned: true,
                privacy_mode: PrivacyMode::Full,
            };
            let stored = StoredLlmProviders { providers: vec![entry], current_provider_id: Some(id.clone()), preferred_provider_id: Some(id) };
            self.set_json(KEY_LLM_PROVIDERS, &stored).await?;
            return Ok(stored);
        }
        Ok(StoredLlmProviders::default())
    }

    /// Vista de compatibilidad de solo lectura sobre el proveedor *en uso*
    /// ahora mismo, bajo el shape viejo de una sola conexión — preserva el
    /// contrato de antes de la Fase de multi-proveedor para los sitios que
    /// todavía arman su propio `OpenAiCompatibleProvider` a mano
    /// (`call`/`discord`/`twitch`/`proactive`) hasta que se migran a
    /// `build_llm_provider`. Si el proveedor actual es `Anthropic`/`Gemini`,
    /// el `base_url` devuelto es el suyo real — un llamador que siga
    /// armando un `OpenAiCompatibleProvider` a partir de esto no hablaría
    /// el protocolo correcto con esos dos kinds; aceptable mientras dure
    /// la migración de esos 4 sitios, no antes de que termine.
    pub async fn llm_connection(&self) -> Result<Option<LlmConnectionSettings>, SettingsError> {
        let stored = self.load_stored_llm_providers().await?;
        let Some(current_id) = &stored.current_provider_id else { return Ok(None) };
        Ok(stored.providers.iter().find(|p| &p.id == current_id).map(|entry| LlmConnectionSettings {
            base_url: resolved_base_url(entry),
            model: entry.model.clone(),
            temperature: entry.temperature,
            max_tokens: entry.max_tokens,
            has_api_key: entry.has_api_key,
            vision_enabled: entry.vision_enabled,
        }))
    }

    /// La key real del proveedor en uso — misma salvedad que `llm_connection`
    /// sobre `Anthropic`/`Gemini` durante la migración.
    /// La key guardada de un proveedor concreto (no el "en uso") — para que
    /// "Probar conexión" use la ya guardada cuando el campo se deja vacío.
    pub async fn llm_provider_api_key(&self, id: &str) -> Result<Option<String>, SettingsError> {
        Ok(self.secrets.get(SECRET_SERVICE_LLM, &secret_key_for_provider(id)).await?)
    }

    pub async fn llm_api_key(&self) -> Result<Option<String>, SettingsError> {
        let stored = self.load_stored_llm_providers().await?;
        let Some(current_id) = &stored.current_provider_id else { return Ok(None) };
        Ok(self.secrets.get(SECRET_SERVICE_LLM, &secret_key_for_provider(current_id)).await?)
    }

    /// Arma el proveedor de IA "en uso" ahora mismo — reemplaza la
    /// construcción manual que se repetía en 6 sitios (`chat`/`memory`/
    /// `call`/`discord`/`twitch` ×2/`proactive`). Si hay más de un
    /// proveedor `enabled && pinned` en el pool (el actual siempre
    /// primero), lo envuelve en `FailoverLLMProvider` para que "nunca deje
    /// de funcionar" si el proveedor en uso se queda sin crédito — ver el
    /// plan de Proveedores IA.
    ///
    /// Necesita `&EventBus` (a diferencia de `build_tts_backend`, que no lo
    /// pide: Motor de voz no tiene rotación) para publicar
    /// `AppEvent::LlmProviderSwitched` cuando el router de verdad cambia de
    /// proveedor a mitad de una generación.
    pub async fn build_llm_provider(&self, event_bus: &EventBus) -> Result<Arc<dyn LLMProvider>, SettingsError> {
        let stored = self.load_stored_llm_providers().await?;
        let current_id = stored
            .current_provider_id
            .clone()
            .ok_or_else(|| SettingsError::Unavailable("no hay ningún proveedor de IA configurado".to_string()))?;
        let current_entry = stored
            .providers
            .iter()
            .find(|p| p.id == current_id)
            .ok_or_else(|| SettingsError::Unavailable("el proveedor de IA en uso ya no existe".to_string()))?;

        let _ = current_entry; // el orden lo decide `order_llm_pool` (preferido, en uso, reservas).
        let mut pool = Vec::new();
        for entry in order_llm_pool(&stored, &llm_provider_is_cooling_down) {
            pool.push(self.build_llm_pool_entry(entry).await?);
        }

        if pool.len() == 1 {
            return Ok(pool.into_iter().next().expect("el pool tiene exactamente 1 elemento acá").provider);
        }

        // Solo se arma el canal + la tarea de fondo cuando de verdad hay
        // rotación posible — un pool de un solo proveedor (el caso más
        // común, sin nada fijado además del actual) nunca paga ese costo.
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<FailoverReport>();
        let settings = self.clone();
        let event_bus = event_bus.clone();
        tokio::spawn(async move {
            while let Some(report) = rx.recv().await {
                settings.apply_failover_report(&event_bus, report).await;
            }
        });

        Ok(Arc::new(FailoverLLMProvider::new(pool, Some(tx)).with_in_use(current_id)))
    }

    /// Lo que el router cuenta de una petición atendida con novedad: marca como
    /// "caídos" un rato a los que fallaron (para no volver a esperarlos en cada
    /// mensaje), y si cambió el proveedor en uso — por un fallo, o porque volvió el
    /// preferido — mueve el puntero y avisa a la interfaz con el motivo.
    async fn apply_failover_report(&self, event_bus: &EventBus, report: FailoverReport) {
        for (id, reason) in &report.failed {
            mark_llm_provider_cooling_down(id, cooldown_for(*reason));
        }
        clear_llm_provider_cooldown(&report.responder_id);

        if report.responder_id != report.in_use_id {
            // Varias llamadas de una misma respuesta (p. ej. la consolidación de
            // memoria) comparten el mismo pool y reportan el mismo cambio: si el
            // proveedor en uso ya es el que respondió, no se avisa otra vez.
            let already_current = self.load_stored_llm_providers().await.ok().and_then(|s| s.current_provider_id).as_deref() == Some(report.responder_id.as_str());
            if already_current {
                return;
            }
            // Best-effort: si esto falla, el badge "En uso" queda desactualizado
            // hasta el próximo cambio, pero la respuesta ya se sirvió bien.
            let _ = self.set_active_llm_provider(&report.responder_id).await;
            event_bus.publish(AppEvent::LlmProviderSwitched {
                from_provider_id: report.in_use_id,
                to_provider_id: report.responder_id,
                reason: report.reason.as_str().to_string(),
            });
        }
    }

    async fn build_llm_pool_entry(&self, entry: &StoredLlmProviderEntry) -> Result<LlmPoolEntry, SettingsError> {
        let api_key = self.secrets.get(SECRET_SERVICE_LLM, &secret_key_for_provider(&entry.id)).await?;
        let provider: Arc<dyn LLMProvider> = match entry.kind {
            LlmProviderKind::Anthropic => Arc::new(AnthropicProvider::new(ANTHROPIC_BASE_URL, api_key.unwrap_or_default())),
            LlmProviderKind::Gemini => Arc::new(GeminiProvider::new(GEMINI_BASE_URL, api_key.unwrap_or_default())),
            _ => Arc::new(OpenAiCompatibleProvider::new(resolved_base_url(entry), api_key)),
        };
        // Siempre envuelto, también en modo `Full`: el envoltorio es quien anexa el
        // contexto (recuerdos, ánimo) que el orquestador manda aparte.
        // El registro de uso va por dentro del filtro: cuenta lo que de verdad se manda.
        let provider: Arc<dyn LLMProvider> = Arc::new(UsageRecordingProvider::new(provider, self.pool.clone(), entry.id.clone()));
        let provider: Arc<dyn LLMProvider> = Arc::new(PrivacyGuardProvider::new(provider, entry.privacy_mode));
        Ok(LlmPoolEntry { id: entry.id.clone(), model: entry.model.clone(), provider })
    }
}

#[cfg(test)]
mod tests {
    use aivt_database::AppSettingsRepository;
    

    use super::*;
    use crate::service::test_service;

    #[tokio::test]
    async fn llm_connection_defaults_to_none() {
        let service = test_service().await;
        assert!(service.llm_connection().await.unwrap().is_none());
        assert!(service.llm_providers().await.unwrap().providers.is_empty());
    }

    fn generic_update(model: &str, api_key: Option<&str>) -> UpdateLlmProviderInput {
        UpdateLlmProviderInput {
            label: "Servidor local".to_string(),
            base_url: Some("http://localhost:1234/v1".to_string()),
            model: model.to_string(),
            temperature: 0.7,
            max_tokens: 1024,
            api_key: api_key.map(str::to_string),
            vision_enabled: false,
            privacy_mode: None,
        }
    }

    #[tokio::test]
    async fn adding_the_first_provider_makes_it_current() {
        let service = test_service().await;
        let added = service
            .add_llm_provider(AddLlmProviderInput { kind: LlmProviderKind::Generic, label: None, base_url: Some("http://localhost:1234/v1".to_string()) })
            .await
            .unwrap();

        assert_eq!(added.label, "Servidor local");
        assert!(added.enabled);
        assert!(!added.pinned);
        let providers = service.llm_providers().await.unwrap();
        assert_eq!(providers.current_provider_id.as_deref(), Some(added.id.as_str()));
    }

    #[tokio::test]
    async fn non_generic_kinds_ignore_a_submitted_base_url() {
        let service = test_service().await;
        let added = service
            .add_llm_provider(AddLlmProviderInput { kind: LlmProviderKind::OpenAi, label: None, base_url: Some("http://ignored".to_string()) })
            .await
            .unwrap();

        assert_eq!(added.label, "OpenAI");
        assert!(added.base_url.is_none());
    }

    #[tokio::test]
    async fn update_llm_provider_with_api_key_stores_it_in_secret_store_not_settings() {
        let service = test_service().await;
        let added = service.add_llm_provider(AddLlmProviderInput { kind: LlmProviderKind::Generic, label: None, base_url: None }).await.unwrap();

        let saved = service.update_llm_provider(&added.id, generic_update("local-model", Some("sk-secret"))).await.unwrap().unwrap();

        assert!(saved.has_api_key);
        assert_eq!(service.llm_api_key().await.unwrap().as_deref(), Some("sk-secret"));

        // The raw setting blob never contains the key itself.
        let raw = AppSettingsRepository::new(&service.pool).get(KEY_LLM_PROVIDERS).await.unwrap().unwrap();
        assert!(!raw.contains("sk-secret"));
    }

    #[tokio::test]
    async fn resaving_without_an_api_key_keeps_the_existing_one() {
        let service = test_service().await;
        let added = service.add_llm_provider(AddLlmProviderInput { kind: LlmProviderKind::Generic, label: None, base_url: None }).await.unwrap();
        service.update_llm_provider(&added.id, generic_update("local-model", Some("sk-secret"))).await.unwrap();

        let saved = service.update_llm_provider(&added.id, generic_update("other-model", None)).await.unwrap().unwrap();

        assert!(saved.has_api_key);
        assert_eq!(service.llm_api_key().await.unwrap().as_deref(), Some("sk-secret"));
        assert_eq!(saved.model, "other-model");
    }

    #[tokio::test]
    async fn saving_an_empty_api_key_clears_it() {
        let service = test_service().await;
        let added = service.add_llm_provider(AddLlmProviderInput { kind: LlmProviderKind::Generic, label: None, base_url: None }).await.unwrap();
        service.update_llm_provider(&added.id, generic_update("local-model", Some("sk-secret"))).await.unwrap();

        let saved = service.update_llm_provider(&added.id, generic_update("local-model", Some(""))).await.unwrap().unwrap();

        assert!(!saved.has_api_key);
        assert!(service.llm_api_key().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn vision_enabled_round_trips_and_defaults_to_false() {
        let service = test_service().await;
        let added = service.add_llm_provider(AddLlmProviderInput { kind: LlmProviderKind::Generic, label: None, base_url: None }).await.unwrap();
        let mut update = generic_update("local-vision-model", None);
        update.vision_enabled = true;

        let saved = service.update_llm_provider(&added.id, update).await.unwrap().unwrap();

        assert!(saved.vision_enabled);
        assert!(service.llm_connection().await.unwrap().unwrap().vision_enabled);
    }

    #[tokio::test]
    async fn two_providers_have_independent_secret_slots() {
        let service = test_service().await;
        let a = service.add_llm_provider(AddLlmProviderInput { kind: LlmProviderKind::OpenAi, label: None, base_url: None }).await.unwrap();
        let b = service.add_llm_provider(AddLlmProviderInput { kind: LlmProviderKind::Anthropic, label: None, base_url: None }).await.unwrap();

        service.update_llm_provider(&a.id, generic_update("gpt", Some("sk-a"))).await.unwrap();
        service.update_llm_provider(&b.id, generic_update("claude", Some("sk-b"))).await.unwrap();

        // Editar `b` no debe pisar la key de `a`.
        let providers = service.llm_providers().await.unwrap().providers;
        assert!(providers.iter().all(|p| p.has_api_key));
    }

    #[tokio::test]
    async fn disabling_a_pinned_provider_also_unpins_it() {
        let service = test_service().await;
        let added = service.add_llm_provider(AddLlmProviderInput { kind: LlmProviderKind::Generic, label: None, base_url: None }).await.unwrap();
        service.set_llm_provider_pinned(&added.id, true).await.unwrap();

        let disabled = service.set_llm_provider_enabled(&added.id, false).await.unwrap().unwrap();

        assert!(!disabled.enabled);
        assert!(!disabled.pinned);
    }

    #[tokio::test]
    async fn pinning_a_disabled_provider_has_no_effect() {
        let service = test_service().await;
        let added = service.add_llm_provider(AddLlmProviderInput { kind: LlmProviderKind::Generic, label: None, base_url: None }).await.unwrap();
        service.set_llm_provider_enabled(&added.id, false).await.unwrap();

        let pinned = service.set_llm_provider_pinned(&added.id, true).await.unwrap().unwrap();

        assert!(!pinned.pinned);
    }

    #[tokio::test]
    async fn removing_the_current_provider_falls_back_to_another_enabled_one() {
        let service = test_service().await;
        let a = service.add_llm_provider(AddLlmProviderInput { kind: LlmProviderKind::OpenAi, label: None, base_url: None }).await.unwrap();
        let b = service.add_llm_provider(AddLlmProviderInput { kind: LlmProviderKind::Anthropic, label: None, base_url: None }).await.unwrap();
        service.set_current_llm_provider(&a.id).await.unwrap();

        service.remove_llm_provider(&a.id).await.unwrap();

        let providers = service.llm_providers().await.unwrap();
        assert_eq!(providers.current_provider_id.as_deref(), Some(b.id.as_str()));
        assert_eq!(providers.providers.len(), 1);
    }

    #[tokio::test]
    async fn set_current_llm_provider_moves_the_pointer() {
        let service = test_service().await;
        let a = service.add_llm_provider(AddLlmProviderInput { kind: LlmProviderKind::OpenAi, label: None, base_url: None }).await.unwrap();
        let b = service.add_llm_provider(AddLlmProviderInput { kind: LlmProviderKind::Anthropic, label: None, base_url: None }).await.unwrap();
        assert_eq!(service.llm_providers().await.unwrap().current_provider_id.as_deref(), Some(a.id.as_str()));

        service.set_current_llm_provider(&b.id).await.unwrap();

        assert_eq!(service.llm_providers().await.unwrap().current_provider_id.as_deref(), Some(b.id.as_str()));
    }

    /// Una instalación existente (de antes de la Fase de multi-proveedor)
    /// tiene una sola conexión guardada bajo la key vieja — debe migrar a
    /// una entrada `Generic`, activa y fijada, sin que el usuario pierda
    /// nada ni tenga que reconfigurar.
    #[tokio::test]
    async fn migrating_an_old_single_connection_preserves_it_as_enabled_and_pinned() {
        let service = test_service().await;
        service
            .set_json(
                KEY_LLM_CONNECTION,
                &serde_json::json!({
                    "baseUrl": "http://localhost:1234/v1",
                    "model": "local-model",
                    "temperature": 0.7,
                    "maxTokens": 1024,
                    "hasApiKey": false,
                    "visionEnabled": false,
                }),
            )
            .await
            .unwrap();

        let providers = service.llm_providers().await.unwrap();

        assert_eq!(providers.providers.len(), 1);
        let entry = &providers.providers[0];
        assert_eq!(entry.kind, LlmProviderKind::Generic);
        assert_eq!(entry.base_url.as_deref(), Some("http://localhost:1234/v1"));
        assert_eq!(entry.model, "local-model");
        assert!(entry.enabled);
        assert!(entry.pinned);
        assert_eq!(providers.current_provider_id.as_deref(), Some(entry.id.as_str()));
    }

    /// A connection saved before Fase 3 (no `visionEnabled` in the stored
    /// JSON at all) must still load — see the `#[serde(default)]` on
    /// `StoredLlmConnection::vision_enabled`.
    #[tokio::test]
    async fn a_connection_saved_before_vision_existed_still_loads() {
        let service = test_service().await;
        AppSettingsRepository::new(&service.pool)
            .set(
                KEY_LLM_CONNECTION,
                r#"{"baseUrl":"http://localhost:1234/v1","model":"m","temperature":0.7,"maxTokens":1024,"hasApiKey":false}"#,
            )
            .await
            .unwrap();

        let loaded = service.llm_connection().await.unwrap().unwrap();
        assert!(!loaded.vision_enabled);
        assert_eq!(loaded.model, "m");
    }

    #[tokio::test]
    async fn a_new_provider_starts_with_full_privacy_and_old_data_without_the_field_loads_as_full() {
        let service = test_service().await;
        let entry = service.add_llm_provider(AddLlmProviderInput { kind: LlmProviderKind::OpenAi, label: None, base_url: None }).await.unwrap();
        assert_eq!(entry.privacy_mode, PrivacyMode::Full);

        // Datos guardados antes de que existieran los modos: sin el campo.
        AppSettingsRepository::new(&service.pool)
            .set(
                KEY_LLM_PROVIDERS,
                r#"{"providers":[{"id":"a","kind":"generic","label":"L","baseUrl":"http://localhost:1234/v1","model":"m","temperature":0.7,"maxTokens":1024,"hasApiKey":false,"visionEnabled":true,"enabled":true,"pinned":false}],"currentProviderId":"a"}"#,
            )
            .await
            .unwrap();
        let loaded = service.llm_providers().await.unwrap();
        assert_eq!(loaded.providers[0].privacy_mode, PrivacyMode::Full);
        assert!(loaded.providers[0].vision_enabled, "la visión que ya tenían no se toca");
    }

    #[tokio::test]
    async fn the_privacy_mode_is_saved_and_a_missing_value_leaves_it_alone() {
        let service = test_service().await;
        let entry = service.add_llm_provider(AddLlmProviderInput { kind: LlmProviderKind::Generic, label: None, base_url: Some("http://localhost:1234/v1".to_string()) }).await.unwrap();

        let mut update = generic_update("m", None);
        update.privacy_mode = Some(PrivacyMode::PublicOnly);
        let saved = service.update_llm_provider(&entry.id, update).await.unwrap().unwrap();
        assert_eq!(saved.privacy_mode, PrivacyMode::PublicOnly);

        // Guardar de nuevo sin indicar modo (p. ej. un formulario viejo) lo conserva.
        let again = service.update_llm_provider(&entry.id, generic_update("m2", None)).await.unwrap().unwrap();
        assert_eq!(again.privacy_mode, PrivacyMode::PublicOnly);
        assert_eq!(service.llm_providers().await.unwrap().providers[0].privacy_mode, PrivacyMode::PublicOnly);
    }

    #[tokio::test]
    async fn minimal_privacy_switches_vision_off_and_it_stays_off() {
        let service = test_service().await;
        let entry = service.add_llm_provider(AddLlmProviderInput { kind: LlmProviderKind::Generic, label: None, base_url: Some("http://localhost:1234/v1".to_string()) }).await.unwrap();

        let mut update = generic_update("m", None);
        update.vision_enabled = true;
        assert!(service.update_llm_provider(&entry.id, update.clone()).await.unwrap().unwrap().vision_enabled);

        update.privacy_mode = Some(PrivacyMode::Minimal);
        let saved = service.update_llm_provider(&entry.id, update.clone()).await.unwrap().unwrap();
        assert!(!saved.vision_enabled, "el modo mínimo no manda imágenes: la visión se apaga");

        // Aunque el formulario la pida otra vez, mientras siga en mínimo no se enciende.
        update.privacy_mode = None;
        assert!(!service.update_llm_provider(&entry.id, update).await.unwrap().unwrap().vision_enabled);
    }

    #[tokio::test]
    async fn the_provider_the_app_builds_filters_by_its_own_mode() {
        use aivt_provider_llm::{ChatMessage, ChatRequest, ContextBlock, MessageRole};
        use futures::StreamExt;

        let service = test_service().await;
        let entry = service.add_llm_provider(AddLlmProviderInput { kind: LlmProviderKind::Generic, label: None, base_url: Some("http://127.0.0.1:9/v1".to_string()) }).await.unwrap();
        let mut update = generic_update("m", None);
        update.base_url = Some("http://127.0.0.1:9/v1".to_string());
        update.privacy_mode = Some(PrivacyMode::Minimal);
        service.update_llm_provider(&entry.id, update).await.unwrap();

        let provider = service.build_llm_provider(&EventBus::new()).await.unwrap();
        let stream = provider.stream_chat(ChatRequest {
            model: "m".to_string(),
            messages: vec![ChatMessage::text(MessageRole::System, "Eres Ciel")],
            temperature: None,
            max_tokens: None,
            tools: Vec::new(),
            context: vec![ContextBlock::Memories { all: "secreto".to_string(), public_only: String::new() }],
            contains_stored_memories: true,
        });
        // Un modo que no permite recuerdos guardados rechaza la petición antes de
        // abrir ninguna conexión: el primer elemento ya es el error.
        let first = stream.collect::<Vec<_>>().await.remove(0);
        assert!(matches!(first, Err(aivt_provider_llm::ProviderError::PrivacyBlocked(_))));
    }

    #[test]
    fn only_servers_on_this_machine_or_a_private_network_count_as_local() {
        for url in ["http://localhost:1234/v1", "http://127.0.0.1:11434", "http://[::1]:8080/v1", "http://192.168.1.20:1234/v1", "http://10.0.0.5/v1", "http://172.20.0.2:9000", "http://mi-pc.local:1234"] {
            assert!(base_url_is_local(url), "{url} debería ser local");
        }
        for url in ["https://api.openai.com/v1", "http://203.0.113.9:1234/v1", "http://172.32.0.1/v1", "https://mi-servidor.com/v1", "http://localhost.evil.com/v1", "http://user@8.8.8.8/v1", ""] {
            assert!(!base_url_is_local(url), "{url} no debería ser local");
        }
    }

    #[tokio::test]
    async fn the_entry_reports_whether_it_is_local() {
        let service = test_service().await;
        let local = service.add_llm_provider(AddLlmProviderInput { kind: LlmProviderKind::Generic, label: None, base_url: Some("http://localhost:1234/v1".to_string()) }).await.unwrap();
        let cloud = service.add_llm_provider(AddLlmProviderInput { kind: LlmProviderKind::OpenAi, label: None, base_url: None }).await.unwrap();
        assert!(local.is_local);
        assert!(!cloud.is_local);
    }

    fn fallback_candidate(kind: LlmProviderKind, model: &str, has_api_key: bool) -> StoredLlmProviderEntry {
        StoredLlmProviderEntry {
            id: "x".to_string(),
            kind,
            label: "x".to_string(),
            base_url: None,
            model: model.to_string(),
            temperature: 0.7,
            max_tokens: 1024,
            has_api_key,
            vision_enabled: false,
            enabled: true,
            pinned: true,
            privacy_mode: PrivacyMode::Full,
        }
    }

    #[test]
    fn a_fallback_without_a_model_is_not_usable() {
        assert!(!is_usable_fallback(&fallback_candidate(LlmProviderKind::Generic, "", false)));
        assert!(!is_usable_fallback(&fallback_candidate(LlmProviderKind::OpenAi, "  ", true)));
    }

    #[test]
    fn a_cloud_fallback_needs_an_api_key_but_a_generic_one_does_not() {
        assert!(!is_usable_fallback(&fallback_candidate(LlmProviderKind::Anthropic, "claude-x", false)));
        assert!(!is_usable_fallback(&fallback_candidate(LlmProviderKind::Gemini, "gemini-x", false)));
        assert!(is_usable_fallback(&fallback_candidate(LlmProviderKind::Anthropic, "claude-x", true)));
        // LM Studio / Ollama no piden key.
        assert!(is_usable_fallback(&fallback_candidate(LlmProviderKind::Generic, "qwen", false)));
    }

    #[test]
    fn openai_compatible_kinds_have_a_fixed_url_and_serialize_as_one_word() {
        let cases = [
            (LlmProviderKind::Groq, "groq", "https://api.groq.com/openai/v1"),
            (LlmProviderKind::OpenRouter, "openrouter", "https://openrouter.ai/api/v1"),
            (LlmProviderKind::Cerebras, "cerebras", "https://api.cerebras.ai/v1"),
            (LlmProviderKind::Mistral, "mistral", "https://api.mistral.ai/v1"),
            (LlmProviderKind::DeepSeek, "deepseek", "https://api.deepseek.com/v1"),
            (LlmProviderKind::Together, "together", "https://api.together.ai/v1"),
            (LlmProviderKind::Xai, "xai", "https://api.x.ai/v1"),
            (LlmProviderKind::SambaNova, "sambanova", "https://api.sambanova.ai/v1"),
        ];
        for (kind, name, url) in cases {
            assert_eq!(serde_json::to_string(&kind).unwrap(), format!("\"{name}\""));
            assert_eq!(serde_json::from_str::<LlmProviderKind>(&format!("\"{name}\"")).unwrap(), kind);
            assert_eq!(kind.openai_compatible_base_url(), Some(url));
        }
        assert_eq!(LlmProviderKind::Generic.openai_compatible_base_url(), None);
        assert_eq!(LlmProviderKind::Anthropic.openai_compatible_base_url(), None);
    }

    fn llm_entry(id: &str, kind: LlmProviderKind, model: &str, has_api_key: bool, pinned: bool) -> StoredLlmProviderEntry {
        StoredLlmProviderEntry {
            id: id.to_string(),
            kind,
            label: id.to_string(),
            base_url: None,
            model: model.to_string(),
            temperature: 0.7,
            max_tokens: 1024,
            has_api_key,
            vision_enabled: false,
            enabled: true,
            pinned,
            privacy_mode: PrivacyMode::Full,
        }
    }

    fn pool_ids(stored: &StoredLlmProviders, cooling_down: &[&str]) -> Vec<String> {
        order_llm_pool(stored, &|id| cooling_down.contains(&id)).iter().map(|e| e.id.clone()).collect()
    }

    fn two_pinned(current: &str, preferred: Option<&str>) -> StoredLlmProviders {
        StoredLlmProviders {
            providers: vec![
                llm_entry("a", LlmProviderKind::Generic, "m", false, true),
                llm_entry("b", LlmProviderKind::Generic, "m", false, true),
            ],
            current_provider_id: Some(current.to_string()),
            preferred_provider_id: preferred.map(str::to_string),
        }
    }

    #[test]
    fn the_preferred_provider_is_tried_first_even_while_a_temporary_one_is_in_use() {
        assert_eq!(pool_ids(&two_pinned("b", Some("a")), &[]), vec!["a", "b"]);
    }

    #[test]
    fn a_provider_that_failed_recently_goes_last_instead_of_being_waited_on_again() {
        assert_eq!(pool_ids(&two_pinned("b", Some("a")), &["a"]), vec!["b", "a"]);
        // y vuelve al frente cuando termina su pausa:
        assert_eq!(pool_ids(&two_pinned("b", Some("a")), &[]), vec!["a", "b"]);
    }

    #[test]
    fn data_saved_before_the_preferred_field_existed_treats_the_current_as_preferred() {
        assert_eq!(pool_ids(&two_pinned("b", None), &[]), vec!["b", "a"]);
    }

    #[test]
    fn a_preferred_provider_that_was_disabled_is_ignored() {
        let mut stored = two_pinned("b", Some("a"));
        stored.providers[0].enabled = false;
        stored.providers[0].pinned = false;
        assert_eq!(pool_ids(&stored, &[]), vec!["b"]);
    }

    #[test]
    fn the_preferred_and_the_current_are_always_in_the_pool_but_unconfigured_reserves_are_not() {
        let stored = StoredLlmProviders {
            providers: vec![
                llm_entry("preferred", LlmProviderKind::Generic, "m", false, false), // sin fijar
                llm_entry("temp", LlmProviderKind::Generic, "m", false, false),
                llm_entry("reserve-ok", LlmProviderKind::Generic, "m", false, true),
                llm_entry("reserve-no-key", LlmProviderKind::Anthropic, "claude", false, true),
                llm_entry("reserve-no-model", LlmProviderKind::Generic, "", false, true),
            ],
            current_provider_id: Some("temp".to_string()),
            preferred_provider_id: Some("preferred".to_string()),
        };
        assert_eq!(pool_ids(&stored, &[]), vec!["preferred", "temp", "reserve-ok"]);
    }

    #[tokio::test]
    async fn choosing_a_provider_by_hand_makes_it_the_preferred_one_too() {
        let service = test_service().await;
        let a = service.add_llm_provider(AddLlmProviderInput { kind: LlmProviderKind::Generic, label: None, base_url: Some("http://a".to_string()) }).await.unwrap();
        let b = service.add_llm_provider(AddLlmProviderInput { kind: LlmProviderKind::Generic, label: None, base_url: Some("http://b".to_string()) }).await.unwrap();

        service.set_current_llm_provider(&b.id).await.unwrap();

        let providers = service.llm_providers().await.unwrap();
        assert_eq!(providers.current_provider_id.as_deref(), Some(b.id.as_str()));
        assert_eq!(providers.preferred_provider_id.as_deref(), Some(b.id.as_str()));
        let _ = a;
    }

    #[tokio::test]
    async fn an_automatic_switch_moves_only_the_current_provider_and_keeps_the_preferred_one() {
        let service = test_service().await;
        let a = service.add_llm_provider(AddLlmProviderInput { kind: LlmProviderKind::Generic, label: None, base_url: Some("http://a".to_string()) }).await.unwrap();
        let b = service.add_llm_provider(AddLlmProviderInput { kind: LlmProviderKind::Generic, label: None, base_url: Some("http://b".to_string()) }).await.unwrap();
        service.set_current_llm_provider(&a.id).await.unwrap();

        service.set_active_llm_provider(&b.id).await.unwrap();

        let providers = service.llm_providers().await.unwrap();
        assert_eq!(providers.current_provider_id.as_deref(), Some(b.id.as_str()));
        assert_eq!(providers.preferred_provider_id.as_deref(), Some(a.id.as_str()), "se vuelve al preferido cuando responda");
    }

    #[tokio::test]
    async fn removing_the_preferred_provider_falls_back_to_the_one_in_use() {
        let service = test_service().await;
        let a = service.add_llm_provider(AddLlmProviderInput { kind: LlmProviderKind::Generic, label: None, base_url: Some("http://a".to_string()) }).await.unwrap();
        let b = service.add_llm_provider(AddLlmProviderInput { kind: LlmProviderKind::Generic, label: None, base_url: Some("http://b".to_string()) }).await.unwrap();
        service.set_current_llm_provider(&a.id).await.unwrap();
        service.set_active_llm_provider(&b.id).await.unwrap();

        service.remove_llm_provider(&a.id).await.unwrap();

        let providers = service.llm_providers().await.unwrap();
        assert_eq!(providers.preferred_provider_id.as_deref(), Some(b.id.as_str()));
    }

    #[test]
    fn cooldowns_are_per_provider_and_expire() {
        let id = "cooldown-test-provider";
        assert!(!llm_provider_is_cooling_down(id));
        mark_llm_provider_cooling_down(id, std::time::Duration::from_secs(60));
        assert!(llm_provider_is_cooling_down(id));
        assert!(!llm_provider_is_cooling_down("someone-else"));
        clear_llm_provider_cooldown(id);
        assert!(!llm_provider_is_cooling_down(id));

        mark_llm_provider_cooling_down(id, std::time::Duration::from_millis(1));
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(!llm_provider_is_cooling_down(id), "una pausa vencida ya no cuenta");
    }
}
