use aivt_core::AppError;
use aivt_database::ActivityEventRow;

use crate::state::AppState;

const RECENT_ACTIVITY_LIMIT: i64 = 200;

#[tauri::command]
pub async fn activity_recent(state: tauri::State<'_, AppState>) -> Result<Vec<ActivityEventRow>, AppError> {
    state.activity.recent(RECENT_ACTIVITY_LIMIT).await.map_err(|err| AppError::from_display("activity", "ACTIVITY_RECENT_FAILED", err))
}

#[tauri::command]
pub async fn activity_clear(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    state.activity.clear().await.map_err(|err| AppError::from_display("activity", "ACTIVITY_CLEAR_FAILED", err))
}
