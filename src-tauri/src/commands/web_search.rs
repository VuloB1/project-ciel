use serde::Serialize;

use aivt_core::AppError;
use aivt_provider_tools::{ObscuraManager, OBSCURA_APPROX_SIZE_MB};
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObscuraStatus {
    pub supported: bool,
    pub installed: bool,
    pub approx_size_mb: u32,
}

#[tauri::command]
pub async fn obscura_get_status(state: tauri::State<'_, AppState>) -> Result<ObscuraStatus, AppError> {
    Ok(ObscuraStatus { supported: ObscuraManager::is_supported(), installed: state.obscura.is_installed(), approx_size_mb: OBSCURA_APPROX_SIZE_MB })
}

/// Starts the download in the background; the frontend follows `AppEvent::ObscuraDownload*`.
#[tauri::command]
pub async fn obscura_download(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    let obscura = state.obscura.clone();
    tokio::spawn(async move {
        if let Err(err) = obscura.download().await {
            tracing::warn!(target: "system", "web_search: la descarga de Obscura falló: {err}");
        }
    });
    Ok(())
}

#[tauri::command]
pub async fn obscura_delete(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    state.obscura.delete().await.map_err(|err| AppError::from_display("web_search", "OBSCURA_DELETE_FAILED", err))
}
