use serde::Serialize;

use aivt_core::AppError;

use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingModelStatus {
    pub downloaded: bool,
    pub approx_size_mb: u32,
}

#[tauri::command]
pub async fn embedding_get_model_status(state: tauri::State<'_, AppState>) -> Result<EmbeddingModelStatus, AppError> {
    Ok(EmbeddingModelStatus {
        downloaded: state.embedding_models.is_downloaded(),
        approx_size_mb: aivt_provider_embedding::APPROX_SIZE_MB,
    })
}

/// Kicks off the download in the background and returns immediately — the
/// frontend follows progress via `AppEvent::EmbeddingModelDownloadProgress`/
/// `Finished`/`Error` instead of blocking this call for however long the
/// ~449MB download takes. Same "spawn + rely on events" shape as
/// `stt_download_model`.
#[tauri::command]
pub async fn embedding_download_model(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    let embedding_models = state.embedding_models.clone();
    tokio::spawn(async move {
        if let Err(err) = embedding_models.download().await {
            tracing::warn!(target: "system", "embedding: la descarga del modelo falló: {err}");
        }
    });
    Ok(())
}

#[tauri::command]
pub async fn embedding_delete_model(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    state.embedding_models.delete().await.map_err(|err| AppError::from_display("embedding", "EMBEDDING_MODEL_DELETE_FAILED", err))
}
