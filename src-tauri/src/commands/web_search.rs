use aivt_core::AppError;
use aivt_service_settings::{WebSearchInput, WebSearchSettings};

use crate::state::AppState;

#[tauri::command]
pub async fn web_search_get_settings(state: tauri::State<'_, AppState>) -> Result<WebSearchSettings, AppError> {
    state.settings.web_search_settings().await.map_err(|err| AppError::from_display("web_search", "WEB_SEARCH_SETTINGS_READ_FAILED", err))
}

#[tauri::command]
pub async fn web_search_set_settings(state: tauri::State<'_, AppState>, input: WebSearchInput) -> Result<WebSearchSettings, AppError> {
    state.settings.set_web_search_settings(input).await.map_err(|err| AppError::from_display("web_search", "WEB_SEARCH_SETTINGS_WRITE_FAILED", err))
}
