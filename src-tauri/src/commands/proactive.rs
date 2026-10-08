use aivt_core::AppError;
use aivt_database::ProactiveSettings;

use crate::commands::memory::map_err;
use crate::state::AppState;

#[tauri::command]
pub async fn proactive_get_settings(state: tauri::State<'_, AppState>, profile_id: String) -> Result<ProactiveSettings, AppError> {
    state.memory.proactive_settings(&profile_id).await.map_err(map_err)
}

#[tauri::command]
pub async fn proactive_set_enabled(
    state: tauri::State<'_, AppState>,
    profile_id: String,
    enabled: bool,
) -> Result<ProactiveSettings, AppError> {
    state.memory.set_proactive_enabled(&profile_id, enabled).await.map_err(map_err)
}

/// Igual que `call_set_active_profile`/`twitch_set_active_profile` — el
/// motor de decisión corre en segundo plano y no tiene otra forma de saber
/// qué personaje está activo en la ventana principal.
#[tauri::command]
pub async fn proactive_set_active_profile(state: tauri::State<'_, AppState>, profile_id: Option<String>) -> Result<(), AppError> {
    state.proactive.set_active_profile(profile_id).await;
    Ok(())
}
