use serde::Serialize;

use aivt_core::AppError;
use aivt_provider_stt::WhisperModelSize;
use aivt_service_settings::{DictationSettings, SttModelInput, SttModelSettings, SttModelSize};

use crate::state::AppState;

fn to_whisper_model_size(size: SttModelSize) -> WhisperModelSize {
    match size {
        SttModelSize::Tiny => WhisperModelSize::Tiny,
        SttModelSize::Base => WhisperModelSize::Base,
        SttModelSize::Small => WhisperModelSize::Small,
        SttModelSize::Medium => WhisperModelSize::Medium,
    }
}

fn from_whisper_model_size(size: WhisperModelSize) -> SttModelSize {
    match size {
        WhisperModelSize::Tiny => SttModelSize::Tiny,
        WhisperModelSize::Base => SttModelSize::Base,
        WhisperModelSize::Small => SttModelSize::Small,
        WhisperModelSize::Medium => SttModelSize::Medium,
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SttModelStatus {
    pub size: SttModelSize,
    pub downloaded: bool,
    pub approx_size_mb: u32,
}

#[tauri::command]
pub async fn stt_get_model(state: tauri::State<'_, AppState>) -> Result<SttModelSettings, AppError> {
    state.settings.stt_model().await.map_err(|err| AppError::from_display("stt", "STT_MODEL_READ_FAILED", err))
}

#[tauri::command]
pub async fn stt_set_model(state: tauri::State<'_, AppState>, input: SttModelInput) -> Result<SttModelSettings, AppError> {
    state.settings.set_stt_model(input).await.map_err(|err| AppError::from_display("stt", "STT_MODEL_WRITE_FAILED", err))
}

#[tauri::command]
pub async fn stt_list_model_status(state: tauri::State<'_, AppState>) -> Result<Vec<SttModelStatus>, AppError> {
    Ok(state
        .stt_models
        .list_status()
        .into_iter()
        .map(|status| SttModelStatus {
            size: from_whisper_model_size(status.size),
            downloaded: status.downloaded,
            approx_size_mb: status.size.approx_size_mb(),
        })
        .collect())
}

/// Kicks off the download in the background and returns immediately — the
/// frontend follows progress via `AppEvent::SttModelDownloadProgress`/
/// `Finished`/`Error` instead of blocking this call for however long a
/// 466MB-1.5GB download takes (same "spawn + rely on events" shape as
/// `ChatService::send_message`'s generation task).
#[tauri::command]
pub async fn stt_download_model(state: tauri::State<'_, AppState>, size: SttModelSize) -> Result<(), AppError> {
    let stt_models = state.stt_models.clone();
    tokio::spawn(async move {
        if let Err(err) = stt_models.download(to_whisper_model_size(size)).await {
            tracing::warn!(target: "system", "stt: la descarga del modelo falló: {err}");
        }
    });
    Ok(())
}

#[tauri::command]
pub async fn stt_delete_model(state: tauri::State<'_, AppState>, size: SttModelSize) -> Result<(), AppError> {
    state
        .stt_models
        .delete(to_whisper_model_size(size))
        .await
        .map_err(|err| AppError::from_display("stt", "STT_MODEL_DELETE_FAILED", err))
}

// ---- Dictado por voz (botón de micrófono del chat) ----------------------

#[tauri::command]
pub async fn dictation_get_settings(state: tauri::State<'_, AppState>) -> Result<DictationSettings, AppError> {
    state.settings.dictation_settings().await.map_err(|err| AppError::from_display("stt", "DICTATION_SETTINGS_READ_FAILED", err))
}

#[tauri::command]
pub async fn dictation_set_settings(
    state: tauri::State<'_, AppState>,
    settings: DictationSettings,
) -> Result<DictationSettings, AppError> {
    state.settings.set_dictation_settings(settings).await.map_err(|err| AppError::from_display("stt", "DICTATION_SETTINGS_WRITE_FAILED", err))
}

/// Empieza a grabar el micrófono. Falla con un mensaje claro si hay una
/// llamada en curso o no hay un modelo de transcripción descargado.
#[tauri::command]
pub async fn dictation_start(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    state.call.dictation_start().await.map_err(|err| AppError::from_display("stt", "DICTATION_START_FAILED", err))
}

/// Termina la grabación y devuelve el texto transcrito (vacío si no se
/// entendió nada).
#[tauri::command]
pub async fn dictation_stop(state: tauri::State<'_, AppState>, profile_id: String) -> Result<String, AppError> {
    state.call.dictation_stop(&profile_id).await.map_err(|err| AppError::from_display("stt", "DICTATION_STOP_FAILED", err))
}

#[tauri::command]
pub async fn dictation_cancel(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    state.call.dictation_cancel().await;
    Ok(())
}
