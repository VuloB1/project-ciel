use std::sync::atomic::Ordering;
use std::sync::Arc;

use tauri_plugin_opener::OpenerExt;

use aivt_core::AppError;
use aivt_provider_llm::{AnthropicProvider, ConnectionStatus, GeminiProvider, LLMProvider, OpenAiCompatibleProvider};
use aivt_provider_tts::{SpeakRequest, TtsBackend, TtsError};
use aivt_service_settings::{
    AddLlmProviderInput, AddTtsProviderInput, AppearanceSettings, BackgroundSettings, LlmProviderEntry, LlmProviderKind,
    LlmProvidersSettings, LlmUsageEntry, UsagePeriod, TtsBackendKind, TtsProviderEntry, TtsProvidersSettings, UpdateLlmProviderInput, UpdateTtsProviderInput,
};

use crate::state::AppState;

fn not_found() -> AppError {
    AppError::new("settings", "LLM_PROVIDER_NOT_FOUND", "El proveedor ya no existe.")
}

fn map_tts_err(err: TtsError) -> AppError {
    let code = match &err {
        TtsError::Unavailable(_) => "TTS_TEST_UNAVAILABLE",
        TtsError::Native(_) => "TTS_TEST_NATIVE_ERROR",
        TtsError::Network(_) => "TTS_TEST_NETWORK_ERROR",
        TtsError::Server(_) => "TTS_TEST_SERVER_ERROR",
        TtsError::Playback(_) => "TTS_TEST_PLAYBACK_ERROR",
    };
    AppError::from_display("settings", code, err)
}

#[tauri::command]
pub async fn settings_get_language(state: tauri::State<'_, AppState>) -> Result<String, AppError> {
    state
        .settings
        .language()
        .await
        .map_err(|err| AppError::from_display("settings", "LANGUAGE_READ_FAILED", err))
}

#[tauri::command]
pub async fn settings_set_language(
    state: tauri::State<'_, AppState>,
    language: String,
) -> Result<(), AppError> {
    state
        .settings
        .set_language(&language)
        .await
        .map_err(|err| AppError::from_display("settings", "LANGUAGE_WRITE_FAILED", err))
}

#[tauri::command]
pub async fn settings_get_background(state: tauri::State<'_, AppState>) -> Result<BackgroundSettings, AppError> {
    state
        .settings
        .background_settings()
        .await
        .map_err(|err| AppError::from_display("settings", "BACKGROUND_SETTINGS_READ_FAILED", err))
}

#[tauri::command]
pub async fn settings_set_background(
    state: tauri::State<'_, AppState>,
    settings: BackgroundSettings,
) -> Result<BackgroundSettings, AppError> {
    let saved = state
        .settings
        .set_background_settings(settings)
        .await
        .map_err(|err| AppError::from_display("settings", "BACKGROUND_SETTINGS_WRITE_FAILED", err))?;
    // Efecto inmediato sin reiniciar — ver `AppState::minimize_to_tray_enabled`
    // y `TTSProvider::set_voice_while_hidden_enabled`.
    state.minimize_to_tray_enabled.store(saved.minimize_to_tray, Ordering::Relaxed);
    state.tts.set_voice_while_hidden_enabled(saved.voice_while_hidden);
    Ok(saved)
}

#[tauri::command]
pub async fn settings_get_appearance(state: tauri::State<'_, AppState>) -> Result<AppearanceSettings, AppError> {
    state
        .settings
        .appearance_settings()
        .await
        .map_err(|err| AppError::from_display("settings", "APPEARANCE_SETTINGS_READ_FAILED", err))
}

#[tauri::command]
pub async fn settings_set_appearance(
    state: tauri::State<'_, AppState>,
    settings: AppearanceSettings,
) -> Result<AppearanceSettings, AppError> {
    state
        .settings
        .set_appearance_settings(settings)
        .await
        .map_err(|err| AppError::from_display("settings", "APPEARANCE_SETTINGS_WRITE_FAILED", err))
}

#[tauri::command]
pub async fn settings_get_onboarding_completed(state: tauri::State<'_, AppState>) -> Result<bool, AppError> {
    state
        .settings
        .onboarding_completed()
        .await
        .map_err(|err| AppError::from_display("settings", "ONBOARDING_READ_FAILED", err))
}

#[tauri::command]
pub async fn settings_set_onboarding_completed(
    state: tauri::State<'_, AppState>,
    completed: bool,
) -> Result<bool, AppError> {
    state
        .settings
        .set_onboarding_completed(completed)
        .await
        .map_err(|err| AppError::from_display("settings", "ONBOARDING_WRITE_FAILED", err))
}

/// Único punto del código que abre un enlace en el navegador del sistema
/// fuera del flujo de OAuth de Twitch (`commands::twitch::twitch_connect_account`,
/// que llama al mismo `opener` directamente porque ahí es parte de un flujo
/// más grande) — usado hoy solo por la pestaña Créditos.
#[tauri::command]
pub async fn open_external_url(app: tauri::AppHandle, url: String) -> Result<(), AppError> {
    app.opener()
        .open_url(&url, None::<&str>)
        .map_err(|err| AppError::from_display("settings", "OPEN_URL_FAILED", err))
}

/// Abre con el programa predeterminado del sistema uno de los documentos
/// legales que viajan con la app (`legal/` en los recursos del instalador):
/// el acuerdo de uso o las licencias de terceros, en el idioma pedido (cae a
/// español si no existe). `kind` y `lang` se validan contra una lista fija:
/// nunca se arma una ruta con texto que llegue del frontend.
#[tauri::command]
pub async fn open_legal_document(app: tauri::AppHandle, kind: String, lang: String) -> Result<(), AppError> {
    use tauri::path::BaseDirectory;
    use tauri::Manager;

    let lang = if ["es", "en", "pt", "ja", "ko", "zh"].contains(&lang.as_str()) { lang.as_str() } else { "es" };
    let file = match kind.as_str() {
        "eula" => format!("legal/EULA.{lang}.txt"),
        "third_party" => format!("legal/THIRD_PARTY_LICENSES.{lang}.txt"),
        _ => return Err(AppError::from_display("settings", "LEGAL_DOCUMENT_UNKNOWN", kind)),
    };
    let path = app
        .path()
        .resolve(&file, BaseDirectory::Resource)
        .map_err(|err| AppError::from_display("settings", "LEGAL_DOCUMENT_NOT_FOUND", err))?;
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|err| AppError::from_display("settings", "OPEN_LEGAL_DOCUMENT_FAILED", err))
}

/// Abre el inspector web (DevTools) de la ventana principal — la entrada
/// "Inspeccionar" del menú contextual propio (`AppContextMenu.tsx`) llama
/// esto en vez de dejar pasar el menú nativo del webview, para no perder el
/// resto de las opciones curadas. `open_devtools` solo existe en builds de
/// debug o con el feature `devtools` (no habilitado); en release este botón
/// no se ofrece — ver `AppContextMenu.tsx`.
#[tauri::command]
pub fn open_devtools(window: tauri::WebviewWindow) {
    #[cfg(debug_assertions)]
    window.open_devtools();
    #[cfg(not(debug_assertions))]
    let _ = window;
}

#[tauri::command]
pub async fn settings_list_llm_providers(state: tauri::State<'_, AppState>) -> Result<LlmProvidersSettings, AppError> {
    state
        .settings
        .llm_providers()
        .await
        .map_err(|err| AppError::from_display("settings", "LLM_PROVIDERS_READ_FAILED", err))
}

/// Un solo `OpenAi`/`Anthropic`/`Gemini` a la vez — la tarjeta sugerida
/// correspondiente pasa a mostrar su entrada en vez de "+ Añadir" una vez
/// que existe. `Generic` no tiene límite ("+ Añadir otro proveedor").
#[tauri::command]
pub async fn settings_add_llm_provider(
    state: tauri::State<'_, AppState>,
    input: AddLlmProviderInput,
) -> Result<LlmProviderEntry, AppError> {
    if input.kind != LlmProviderKind::Generic {
        let existing = state
            .settings
            .llm_providers()
            .await
            .map_err(|err| AppError::from_display("settings", "LLM_PROVIDERS_READ_FAILED", err))?;
        if existing.providers.iter().any(|p| p.kind == input.kind) {
            return Err(AppError::new(
                "settings",
                "LLM_PROVIDER_KIND_ALREADY_EXISTS",
                "Ya hay un proveedor de este tipo configurado.",
            ));
        }
    }
    state
        .settings
        .add_llm_provider(input)
        .await
        .map_err(|err| AppError::from_display("settings", "LLM_PROVIDER_ADD_FAILED", err))
}

#[tauri::command]
pub async fn settings_update_llm_provider(
    state: tauri::State<'_, AppState>,
    id: String,
    input: UpdateLlmProviderInput,
) -> Result<LlmProviderEntry, AppError> {
    state
        .settings
        .update_llm_provider(&id, input)
        .await
        .map_err(|err| AppError::from_display("settings", "LLM_PROVIDER_UPDATE_FAILED", err))?
        .ok_or_else(not_found)
}

#[tauri::command]
pub async fn settings_remove_llm_provider(state: tauri::State<'_, AppState>, id: String) -> Result<(), AppError> {
    state
        .settings
        .remove_llm_provider(&id)
        .await
        .map_err(|err| AppError::from_display("settings", "LLM_PROVIDER_REMOVE_FAILED", err))
}

/// "Activar". Apagarlo también apaga "fijar" del lado del servicio (ver
/// `SettingsService::set_llm_provider_enabled`) — acá no hace falta
/// duplicar esa regla, solo reportar el resultado.
#[tauri::command]
pub async fn settings_set_llm_provider_enabled(
    state: tauri::State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<LlmProviderEntry, AppError> {
    state
        .settings
        .set_llm_provider_enabled(&id, enabled)
        .await
        .map_err(|err| AppError::from_display("settings", "LLM_PROVIDER_UPDATE_FAILED", err))?
        .ok_or_else(not_found)
}

/// "Fijar" — entra en la rotación automática por falta de crédito. Sin
/// efecto si la entrada no está `enabled` (ver
/// `SettingsService::set_llm_provider_pinned`).
#[tauri::command]
pub async fn settings_set_llm_provider_pinned(
    state: tauri::State<'_, AppState>,
    id: String,
    pinned: bool,
) -> Result<LlmProviderEntry, AppError> {
    state
        .settings
        .set_llm_provider_pinned(&id, pinned)
        .await
        .map_err(|err| AppError::from_display("settings", "LLM_PROVIDER_UPDATE_FAILED", err))?
        .ok_or_else(not_found)
}

/// "Usar ahora" — selección manual. A diferencia del resto, valida acá
/// (no en `SettingsService`) que el destino esté activo, porque es la
/// única regla de negocio real de este comando.
#[tauri::command]
pub async fn settings_set_current_llm_provider(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<LlmProvidersSettings, AppError> {
    let providers = state
        .settings
        .llm_providers()
        .await
        .map_err(|err| AppError::from_display("settings", "LLM_PROVIDERS_READ_FAILED", err))?;
    let target = providers.providers.iter().find(|p| p.id == id).ok_or_else(not_found)?;
    if !target.enabled {
        return Err(AppError::new("settings", "LLM_PROVIDER_NOT_ENABLED", "Activa el proveedor antes de usarlo."));
    }
    state
        .settings
        .set_current_llm_provider(&id)
        .await
        .map_err(|err| AppError::from_display("settings", "LLM_PROVIDER_UPDATE_FAILED", err))?
        .ok_or_else(not_found)
}

/// Lo que se está por guardar, no lo guardado — mismo contrato "typed, not
/// stored" que tenía `settings_test_llm_connection`. `base_url` solo se
/// usa para `Generic`.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestLlmProviderInput {
    /// Si se pasa y `api_key` viene vacío, se usa la key ya guardada de este
    /// proveedor — así probar un proveedor ya configurado no obliga a volver
    /// a pegar la key.
    pub id: Option<String>,
    pub kind: LlmProviderKind,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
}

const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
const ANTHROPIC_BASE_URL: &str = "https://api.anthropic.com/v1";
const GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

/// `OpenAi` reusa `OpenAiCompatibleProvider` tal cual (es el mismo
/// protocolo) contra un endpoint fijo; `Anthropic`/`Gemini` usan sus
/// adaptadores propios, mismo criterio.
#[tauri::command]
pub async fn settings_test_llm_provider(
    state: tauri::State<'_, AppState>,
    mut input: TestLlmProviderInput,
) -> Result<ConnectionStatus, AppError> {
    if input.api_key.as_deref().map_or(true, str::is_empty) {
        if let Some(id) = &input.id {
            input.api_key = state.settings.llm_provider_api_key(id).await.ok().flatten();
        }
    }
    let provider: Arc<dyn LLMProvider> = match input.kind {
        LlmProviderKind::Generic => Arc::new(OpenAiCompatibleProvider::new(input.base_url.unwrap_or_default(), input.api_key)),
        LlmProviderKind::Anthropic => Arc::new(AnthropicProvider::new(ANTHROPIC_BASE_URL, input.api_key.unwrap_or_default())),
        LlmProviderKind::Gemini => Arc::new(GeminiProvider::new(GEMINI_BASE_URL, input.api_key.unwrap_or_default())),
        // OpenAI, Groq, OpenRouter, Cerebras, Mistral, DeepSeek: mismo protocolo, URL fija.
        kind => Arc::new(OpenAiCompatibleProvider::new(kind.openai_compatible_base_url().unwrap_or_default(), input.api_key)),
    };
    provider
        .test_connection()
        .await
        .map_err(|err| AppError::from_display("settings", "LLM_CONNECTION_TEST_FAILED", err))
}

#[tauri::command]
pub async fn settings_list_tts_providers(state: tauri::State<'_, AppState>) -> Result<TtsProvidersSettings, AppError> {
    state.settings.tts_providers().await.map_err(|err| AppError::from_display("settings", "TTS_PROVIDERS_READ_FAILED", err))
}

/// Un solo proveedor por kind salvo `Http` (sin límite). `Native` se
/// rechaza siempre — ya existe desde el primer arranque (ver
/// `SettingsService::load_stored_tts_providers`).
#[tauri::command]
pub async fn settings_add_tts_provider(
    state: tauri::State<'_, AppState>,
    input: AddTtsProviderInput,
) -> Result<TtsProviderEntry, AppError> {
    if input.kind == TtsBackendKind::Native {
        return Err(AppError::new("settings", "TTS_PROVIDER_NATIVE_ALREADY_EXISTS", "La voz nativa ya está disponible siempre."));
    }
    if input.kind != TtsBackendKind::Http {
        let existing = state
            .settings
            .tts_providers()
            .await
            .map_err(|err| AppError::from_display("settings", "TTS_PROVIDERS_READ_FAILED", err))?;
        if existing.providers.iter().any(|p| p.kind == input.kind) {
            return Err(AppError::new(
                "settings",
                "TTS_PROVIDER_KIND_ALREADY_EXISTS",
                "Ya hay un proveedor de este tipo configurado.",
            ));
        }
    }
    state
        .settings
        .add_tts_provider(input)
        .await
        .map_err(|err| AppError::from_display("settings", "TTS_PROVIDER_ADD_FAILED", err))
}

#[tauri::command]
pub async fn settings_update_tts_provider(
    state: tauri::State<'_, AppState>,
    id: String,
    input: UpdateTtsProviderInput,
) -> Result<TtsProviderEntry, AppError> {
    state
        .settings
        .update_tts_provider(&id, input)
        .await
        .map_err(|err| AppError::from_display("settings", "TTS_PROVIDER_UPDATE_FAILED", err))?
        .ok_or_else(not_found)
}

/// Rechaza eliminar `Native` — ver `SettingsService::remove_tts_provider`.
#[tauri::command]
pub async fn settings_remove_tts_provider(state: tauri::State<'_, AppState>, id: String) -> Result<(), AppError> {
    let providers = state
        .settings
        .tts_providers()
        .await
        .map_err(|err| AppError::from_display("settings", "TTS_PROVIDERS_READ_FAILED", err))?;
    if providers.providers.iter().any(|p| p.id == id && p.kind == TtsBackendKind::Native) {
        return Err(AppError::new("settings", "TTS_PROVIDER_NATIVE_NOT_REMOVABLE", "La voz nativa no se puede eliminar."));
    }
    state
        .settings
        .remove_tts_provider(&id)
        .await
        .map_err(|err| AppError::from_display("settings", "TTS_PROVIDER_REMOVE_FAILED", err))
}

/// "Activar" — rechaza desactivar `Native` (siempre disponible).
#[tauri::command]
pub async fn settings_set_tts_provider_enabled(
    state: tauri::State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<TtsProviderEntry, AppError> {
    if !enabled {
        let providers = state
            .settings
            .tts_providers()
            .await
            .map_err(|err| AppError::from_display("settings", "TTS_PROVIDERS_READ_FAILED", err))?;
        if providers.providers.iter().any(|p| p.id == id && p.kind == TtsBackendKind::Native) {
            return Err(AppError::new("settings", "TTS_PROVIDER_NATIVE_ALWAYS_ENABLED", "La voz nativa no se puede desactivar."));
        }
    }
    state
        .settings
        .set_tts_provider_enabled(&id, enabled)
        .await
        .map_err(|err| AppError::from_display("settings", "TTS_PROVIDER_UPDATE_FAILED", err))?
        .ok_or_else(not_found)
}

/// "Usar ahora" — igual que `settings_set_current_llm_provider`, valida
/// acá (no en `SettingsService`) que el destino esté activo.
#[tauri::command]
pub async fn settings_set_current_tts_provider(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<TtsProvidersSettings, AppError> {
    let providers = state
        .settings
        .tts_providers()
        .await
        .map_err(|err| AppError::from_display("settings", "TTS_PROVIDERS_READ_FAILED", err))?;
    let target = providers.providers.iter().find(|p| p.id == id).ok_or_else(not_found)?;
    if !target.enabled {
        return Err(AppError::new("settings", "TTS_PROVIDER_NOT_ENABLED", "Activa el proveedor antes de usarlo."));
    }
    state
        .settings
        .set_current_tts_provider(&id)
        .await
        .map_err(|err| AppError::from_display("settings", "TTS_PROVIDER_UPDATE_FAILED", err))?
        .ok_or_else(not_found)
}

/// `voices_found` is only populated for the kinds with a real "list
/// voices" endpoint (`ElevenLabs`/`Google`/`Azure`); `OpenAi`/`Http` have
/// no equivalent (`VoiceInfo`'s Rust doc comment), so testing them instead
/// synthesizes and plays `text` out loud — proving the whole pipeline
/// works, not just that something answers on `base_url`.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsConnectionTestResult {
    pub voices_found: Option<usize>,
}

/// Lo que se está por guardar, no lo guardado — mismo contrato "typed, not
/// stored" que `TestLlmProviderInput`. `base_url` solo se usa para `Http`;
/// `region` solo para `Azure`.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestTtsProviderInput {
    /// Ver `TestLlmProviderInput::id`.
    pub id: Option<String>,
    pub kind: TtsBackendKind,
    pub base_url: Option<String>,
    pub region: Option<String>,
    /// Solo `FishAudio`.
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub text: String,
}

/// Tests a (possibly not-yet-saved) TTS provider directly, same "typed,
/// not stored" contract as `settings_test_llm_provider` above — nothing
/// here reads or depends on `SettingsService::tts_providers()`. Unlike
/// `settings_test_llm_provider`, every non-`Native` kind is actually
/// probable from day one — `Google`/`Azure` have real adapters, not a
/// `_KIND_NOT_SUPPORTED` stub.
#[tauri::command]
pub async fn settings_test_tts_provider(
    state: tauri::State<'_, AppState>,
    mut input: TestTtsProviderInput,
) -> Result<TtsConnectionTestResult, AppError> {
    if input.api_key.as_deref().map_or(true, str::is_empty) {
        if let Some(id) = &input.id {
            input.api_key = state.settings.tts_provider_api_key(id).await.ok().flatten();
        }
    }
    match input.kind {
        TtsBackendKind::Native => {
            Err(AppError::new("settings", "TTS_TEST_NOT_APPLICABLE", "La voz nativa no necesita probar conexión"))
        }
        TtsBackendKind::ElevenLabs => {
            let backend = TtsBackend::ElevenLabs { api_key: input.api_key.unwrap_or_default() };
            let voices = state.tts.list_voices(&backend).await.map_err(map_tts_err)?;
            Ok(TtsConnectionTestResult { voices_found: Some(voices.len()) })
        }
        TtsBackendKind::Google => {
            let backend = TtsBackend::Google { api_key: input.api_key.unwrap_or_default() };
            let voices = state.tts.list_voices(&backend).await.map_err(map_tts_err)?;
            Ok(TtsConnectionTestResult { voices_found: Some(voices.len()) })
        }
        TtsBackendKind::Azure => {
            let backend = TtsBackend::Azure { api_key: input.api_key.unwrap_or_default(), region: input.region.unwrap_or_default() };
            let voices = state.tts.list_voices(&backend).await.map_err(map_tts_err)?;
            Ok(TtsConnectionTestResult { voices_found: Some(voices.len()) })
        }
        // Sin endpoint de voces: se prueba sintetizando y reproduciendo `text`.
        TtsBackendKind::OpenAi | TtsBackendKind::Http | TtsBackendKind::FishAudio | TtsBackendKind::Deepgram | TtsBackendKind::Groq | TtsBackendKind::Together => {
            let api_key_text = input.api_key.clone().unwrap_or_default();
            let backend = match input.kind {
                TtsBackendKind::FishAudio => TtsBackend::FishAudio { api_key: api_key_text, model: input.model.clone() },
                TtsBackendKind::Deepgram => TtsBackend::Deepgram { api_key: api_key_text },
                TtsBackendKind::Groq => TtsBackend::Groq { api_key: api_key_text, model: input.model.clone() },
                TtsBackendKind::Together => TtsBackend::Together { api_key: api_key_text, model: input.model.clone() },
                TtsBackendKind::OpenAi => TtsBackend::Http { base_url: OPENAI_BASE_URL.to_string(), api_key: input.api_key },
                _ => TtsBackend::Http { base_url: input.base_url.unwrap_or_default(), api_key: input.api_key },
            };
            let request = SpeakRequest {
                text: input.text,
                voice_id: String::new(),
                rate: 1.0,
                pitch: 1.0,
                volume: 1.0,
                backend,
                profile_id: "settings-tts-test".to_string(),
                message_id: None,
            };
            state.tts.speak(request).await.map_err(map_tts_err)?;
            Ok(TtsConnectionTestResult { voices_found: None })
        }
    }
}

/// Panel de uso de Configuración > Proveedores de IA: una fila por proveedor configurado.
#[tauri::command]
pub async fn settings_get_llm_usage(state: tauri::State<'_, AppState>, period: UsagePeriod) -> Result<Vec<LlmUsageEntry>, AppError> {
    state.settings.llm_usage(period).await.map_err(|err| AppError::from_display("settings", "LLM_USAGE_READ_FAILED", err))
}

#[tauri::command]
pub async fn settings_reset_llm_usage(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    state.settings.clear_llm_usage().await.map_err(|err| AppError::from_display("settings", "LLM_USAGE_RESET_FAILED", err))
}
