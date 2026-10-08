use aivt_core::AppError;
use aivt_eventbus::ServiceStatus;
use aivt_service_settings::CallPolicy;

use crate::state::AppState;

#[tauri::command]
pub async fn call_list_microphones() -> Result<Vec<String>, AppError> {
    aivt_service_call::list_input_devices().map_err(|err| AppError::from_display("call", "CALL_MIC_LIST_FAILED", err))
}

#[tauri::command]
pub async fn call_get_policy(state: tauri::State<'_, AppState>) -> Result<CallPolicy, AppError> {
    state.settings.call_policy().await.map_err(|err| AppError::from_display("call", "CALL_SETTINGS_READ_FAILED", err))
}

#[tauri::command]
pub async fn call_set_policy(state: tauri::State<'_, AppState>, policy: CallPolicy) -> Result<CallPolicy, AppError> {
    state.settings.set_call_policy(policy).await.map_err(|err| AppError::from_display("call", "CALL_SETTINGS_WRITE_FAILED", err))
}

#[tauri::command]
pub async fn call_start(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    state.service_manager.start("call").await.map_err(|err| AppError::from_display("call", "CALL_START_FAILED", err))
}

#[tauri::command]
pub async fn call_stop(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    state.service_manager.stop("call").await.map_err(|err| AppError::from_display("call", "CALL_STOP_FAILED", err))
}

#[tauri::command]
pub async fn call_get_status(state: tauri::State<'_, AppState>) -> Result<Option<ServiceStatus>, AppError> {
    Ok(state.service_manager.status("call").await)
}

/// The frontend calls this whenever the active profile changes — see
/// `TwitchService`'s doc comment for why the backend has no other way to
/// know which character should answer (same reasoning applies to Call).
#[tauri::command]
pub async fn call_set_active_profile(state: tauri::State<'_, AppState>, profile_id: Option<String>) -> Result<(), AppError> {
    state.call.set_active_profile(profile_id).await;
    Ok(())
}

#[tauri::command]
pub async fn call_set_muted(state: tauri::State<'_, AppState>, muted: bool) -> Result<(), AppError> {
    state.call.set_muted(muted).await;
    Ok(())
}

#[tauri::command]
pub async fn call_is_muted(state: tauri::State<'_, AppState>) -> Result<bool, AppError> {
    Ok(state.call.is_muted().await)
}

/// Abre el micrófono solo para mostrar su nivel (`call_mic_level`) en
/// Configuración > Llamada; no transcribe ni guarda nada.
#[tauri::command]
pub async fn call_mic_test_start(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    state.call.mic_test_start().await.map_err(|err| AppError::from_display("call", "CALL_MIC_TEST_FAILED", err))
}

#[tauri::command]
pub async fn call_mic_test_stop(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    state.call.mic_test_stop().await;
    Ok(())
}
