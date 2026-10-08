use aivt_core::AppError;
use aivt_database::ProfileMood;

use crate::commands::memory::map_err;
use crate::state::AppState;

#[tauri::command]
pub async fn mood_get(state: tauri::State<'_, AppState>, profile_id: String) -> Result<ProfileMood, AppError> {
    state.memory.get_mood(&profile_id).await.map_err(map_err)
}

#[tauri::command]
pub async fn mood_reset(state: tauri::State<'_, AppState>, profile_id: String) -> Result<ProfileMood, AppError> {
    state.memory.reset_mood(&profile_id).await.map_err(map_err)
}
