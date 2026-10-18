use aivt_core::AppError;
use aivt_database::{ProfileVoice, ProfileVoiceUpdate};
use aivt_provider_tts::{SpeakRequest, TtsBackend, TtsError, VoiceInfo};

use crate::state::AppState;

fn map_err(err: TtsError) -> AppError {
    let code = match &err {
        TtsError::Unavailable(_) => "VOICE_UNAVAILABLE",
        TtsError::Native(_) => "VOICE_NATIVE_ERROR",
        TtsError::Network(_) => "VOICE_NETWORK_ERROR",
        TtsError::Server(_) => "VOICE_SERVER_ERROR",
        TtsError::Playback(_) => "VOICE_PLAYBACK_ERROR",
    };
    AppError::from_display("voice", code, err)
}

async fn resolve_backend(state: &AppState) -> Result<TtsBackend, AppError> {
    state.settings.build_tts_backend().await.map_err(|err| AppError::from_display("voice", "VOICE_SETTINGS_READ_FAILED", err))
}

/// Only ever populated for the native backend — see `VoiceInfo`'s doc
/// comment for why an HTTP backend can't be enumerated.
#[tauri::command]
pub async fn voice_list_voices(state: tauri::State<'_, AppState>) -> Result<Vec<VoiceInfo>, AppError> {
    let backend = resolve_backend(&state).await?;
    state.tts.list_voices(&backend).await.map_err(map_err)
}

/// Búsqueda por nombre en el catálogo de voces del servicio en uso (solo Fish
/// Audio tiene un catálogo tan grande como para necesitarla).
#[tauri::command]
pub async fn voice_search_voices(state: tauri::State<'_, AppState>, query: String) -> Result<Vec<VoiceInfo>, AppError> {
    let backend = resolve_backend(&state).await?;
    state.tts.search_voices(&backend, &query).await.map_err(map_err)
}

#[tauri::command]
pub async fn voice_get_settings(state: tauri::State<'_, AppState>, profile_id: String) -> Result<ProfileVoice, AppError> {
    let profile = state
        .profiles
        .get(&profile_id)
        .await
        .map_err(|err| AppError::from_display("voice", "VOICE_PROFILE_LOOKUP_FAILED", err))?
        .ok_or_else(|| AppError::new("voice", "VOICE_PROFILE_NOT_FOUND", "El perfil ya no existe"))?;
    Ok(profile.voice)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceSettingsInput {
    pub enabled: bool,
    pub voice_id: String,
    pub rate: f64,
    pub pitch: f64,
    pub volume: f64,
}

#[tauri::command]
pub async fn voice_set_settings(
    state: tauri::State<'_, AppState>,
    profile_id: String,
    input: VoiceSettingsInput,
) -> Result<ProfileVoice, AppError> {
    state
        .profiles
        .set_voice(
            &profile_id,
            ProfileVoiceUpdate { enabled: input.enabled, voice_id: input.voice_id, rate: input.rate, pitch: input.pitch, volume: input.volume },
        )
        .await
        .map_err(|err| AppError::from_display("voice", "VOICE_SETTINGS_WRITE_FAILED", err))
}

/// Manual playback — used by the speaker button on a chat message
/// (replaying it regardless of the auto-play toggle, `message_id` set and
/// no override — always the profile's *saved* voice) and the Voz tab's
/// "Probar voz" preview (`message_id: None`, `voice_override` set to
/// whatever's currently in the form so previewing an unsaved slider tweak
/// doesn't require saving first — mirrors `settings_test_llm_provider`
/// testing the typed-but-not-yet-saved connection).
#[tauri::command]
pub async fn voice_speak(
    state: tauri::State<'_, AppState>,
    profile_id: String,
    message_id: Option<String>,
    text: String,
    voice_override: Option<VoiceSettingsInput>,
) -> Result<(), AppError> {
    let voice = match voice_override {
        Some(v) => ProfileVoice { enabled: v.enabled, voice_id: v.voice_id, rate: v.rate, pitch: v.pitch, volume: v.volume },
        None => {
            let profile = state
                .profiles
                .get(&profile_id)
                .await
                .map_err(|err| AppError::from_display("voice", "VOICE_PROFILE_LOOKUP_FAILED", err))?
                .ok_or_else(|| AppError::new("voice", "VOICE_PROFILE_NOT_FOUND", "El perfil ya no existe"))?;
            profile.voice
        }
    };
    let backend = resolve_backend(&state).await?;
    // A chat message listened to for the first time is saved too (when "guardar audio" is on),
    // so listening again replays the same clip instead of generating a slightly different one.
    let save_for = match &message_id {
        Some(id) if state.settings.chat_audio_settings().await.map(|s| s.save_enabled).unwrap_or(false) => Some(id.clone()),
        _ => None,
    };

    let request = SpeakRequest {
        text,
        voice_id: voice.voice_id,
        rate: voice.rate as f32,
        pitch: voice.pitch as f32,
        volume: voice.volume as f32,
        backend,
        profile_id,
        message_id,
    };
    let Some(message_id) = save_for else {
        return state.tts.speak(request).await.map_err(map_err);
    };
    if let Some(bytes) = state.tts.speak_and_capture(request).await.map_err(map_err)? {
        if let Err(err) = state.chat.save_spoken_audio(&message_id, &bytes).await {
            tracing::warn!(target: "system", "no se pudo guardar el audio del mensaje {message_id}: {err}");
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn voice_stop(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    state.tts.stop().await.map_err(map_err)
}

/// Persisted mute state — see `SettingsService::voice_muted`'s doc comment
/// for why this (not `state.tts.is_muted()`) is the source of truth for a
/// getter: it's what a restart re-seeds `AivtTtsProvider` from, so it's
/// the value that stays correct even if the two ever drifted.
#[tauri::command]
pub async fn voice_get_muted(state: tauri::State<'_, AppState>) -> Result<bool, AppError> {
    state.settings.voice_muted().await.map_err(|err| AppError::from_display("voice", "VOICE_SETTINGS_READ_FAILED", err))
}

/// The Sidebar's global mute button (next to "mostrar ventana flotante") —
/// covers every voice-producing channel via `AivtTtsProvider`'s shared
/// `AtomicBool` (see its doc comment), not a per-integration setting.
/// Muting also stops whatever's playing *right now*, not just future
/// calls — a live-testing round found that muting future calls alone
/// still left an already-started reply audible, which read as "the button
/// doesn't work."
#[tauri::command]
pub async fn voice_set_muted(state: tauri::State<'_, AppState>, muted: bool) -> Result<bool, AppError> {
    let saved = state.settings.set_voice_muted(muted).await.map_err(|err| AppError::from_display("voice", "VOICE_SETTINGS_WRITE_FAILED", err))?;
    state.tts.set_muted(saved);
    if saved {
        state.tts.stop().await.map_err(map_err)?;
    }
    Ok(saved)
}

/// Salidas de audio disponibles para el selector de Configuración > Llamada.
#[tauri::command]
pub async fn voice_list_output_devices() -> Result<Vec<String>, AppError> {
    aivt_provider_tts::list_output_devices().map_err(|err| AppError::from_display("voice", "VOICE_OUTPUT_LIST_FAILED", err))
}

#[tauri::command]
pub async fn voice_get_output_device(state: tauri::State<'_, AppState>) -> Result<Option<String>, AppError> {
    state.settings.audio_output_device().await.map_err(|err| AppError::from_display("voice", "VOICE_SETTINGS_READ_FAILED", err))
}

/// Guarda la salida elegida y la aplica al momento (lo que esté sonando se
/// corta: no se puede mover a medias a otro dispositivo).
#[tauri::command]
pub async fn voice_set_output_device(state: tauri::State<'_, AppState>, name: Option<String>) -> Result<Option<String>, AppError> {
    let saved = state
        .settings
        .set_audio_output_device(name)
        .await
        .map_err(|err| AppError::from_display("voice", "VOICE_SETTINGS_WRITE_FAILED", err))?;
    state.tts.set_output_device(saved.clone());
    Ok(saved)
}
