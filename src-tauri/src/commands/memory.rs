use aivt_core::AppError;
use aivt_database::{Memory, MemoryUpdate, NewMemory};
use aivt_memory::{FusionGroupInput, FusionProposal, MemoryError};

use crate::commands::chat::resolve_llm_connection;
use crate::state::AppState;

/// `pub(crate)` so `commands::mood` (same `MemoryError`, via `state.memory`)
/// can reuse it instead of duplicating the match.
pub(crate) fn map_err(err: MemoryError) -> AppError {
    let code = match &err {
        MemoryError::Database(_) => "MEMORY_DATABASE_ERROR",
        MemoryError::Llm(_) => "MEMORY_LLM_ERROR",
        MemoryError::ExtractionParseFailed => "MEMORY_EXTRACTION_PARSE_FAILED",
    };
    AppError::from_display("memory", code, err)
}

#[tauri::command]
pub async fn memory_list(state: tauri::State<'_, AppState>, profile_id: String) -> Result<Vec<Memory>, AppError> {
    state.memory.list_for_profile(&profile_id).await.map_err(map_err)
}

#[tauri::command]
pub async fn memory_search(
    state: tauri::State<'_, AppState>,
    profile_id: String,
    query: String,
) -> Result<Vec<Memory>, AppError> {
    state.memory.search(&profile_id, &query, 50).await.map_err(map_err)
}

/// Manual creation — the Memoria tab's "add memory" form (sección 32 del
/// brief: el usuario también puede escribir memorias directamente, no solo
/// las que el personaje infiere de la conversación). `summary` is optional
/// here — the form doesn't force the user to fill out three fields for a
/// quick manual note; `title`/`content` are required (enforced by the
/// frontend form).
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryInput {
    pub profile_id: String,
    pub title: String,
    pub content: String,
    pub summary: Option<String>,
    pub category: String,
    pub importance: i64,
}

#[tauri::command]
pub async fn memory_create(state: tauri::State<'_, AppState>, input: MemoryInput) -> Result<Memory, AppError> {
    state
        .memory
        .create(NewMemory {
            profile_id: input.profile_id,
            title: input.title,
            content: input.content,
            summary: input.summary.filter(|s| !s.trim().is_empty()),
            category: input.category,
            importance: input.importance.clamp(0, 100),
            confidence: 100,
            source: "manual".to_string(),
            source_id: None,
        })
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn memory_set_pinned(state: tauri::State<'_, AppState>, id: String, pinned: bool) -> Result<(), AppError> {
    state.memory.set_pinned(&id, pinned).await.map_err(map_err)
}

/// Hace una memoria privada (solo Chat y Llamada) o pública (también
/// Twitch/Discord). `scope` es `"private"` o `"public"`; cualquier otro valor
/// se rechaza en el repositorio.
#[tauri::command]
pub async fn memory_set_scope(state: tauri::State<'_, AppState>, id: String, scope: String) -> Result<(), AppError> {
    state.memory.set_scope(&id, &scope).await.map_err(map_err)
}

/// Lets the user correct a memory's title/text/category/importance after
/// seeing it — the extraction (or their own manual entry) isn't always
/// right.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryUpdateInput {
    pub title: String,
    pub content: String,
    pub summary: Option<String>,
    pub category: String,
    pub importance: i64,
}

#[tauri::command]
pub async fn memory_update(
    state: tauri::State<'_, AppState>,
    id: String,
    input: MemoryUpdateInput,
) -> Result<Memory, AppError> {
    state
        .memory
        .update(
            &id,
            MemoryUpdate {
                title: input.title,
                content: input.content,
                summary: input.summary.filter(|s| !s.trim().is_empty()),
                category: input.category,
                importance: input.importance.clamp(0, 100),
            },
        )
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn memory_delete(state: tauri::State<'_, AppState>, id: String) -> Result<(), AppError> {
    state.memory.delete(&id).await.map_err(map_err)
}

/// Fire-and-forget, mismo patrón que `commands::embedding::embedding_download_model`
/// — la UI sigue el progreso real vía `AppEvent::EmbeddingBackfillProgress`/
/// `Finished`/`Error`, no el retorno de este comando.
#[tauri::command]
pub async fn memory_backfill_embeddings(state: tauri::State<'_, AppState>, profile_id: String) -> Result<(), AppError> {
    let memory = state.memory.clone();
    tokio::spawn(async move {
        match memory.backfill_embeddings(&profile_id).await {
            Ok(count) => tracing::info!(target: "system", "memory: backfill de embeddings completo ({count} memorias)"),
            Err(err) => tracing::warn!(target: "system", "memory: backfill de embeddings falló: {err}"),
        }
    });
    Ok(())
}

/// Botón "Fusionar memorias" (Configuración > Memoria) — pura consulta, no
/// escribe nada todavía. El usuario revisa la propuesta y confirma vía
/// `memory_apply_fusion` antes de que se borre ninguna memoria original.
#[tauri::command]
pub async fn memory_propose_fusion(state: tauri::State<'_, AppState>, profile_id: String) -> Result<Vec<FusionProposal>, AppError> {
    let resolved = resolve_llm_connection(&state).await?;
    let language = state.settings.language().await.unwrap_or_else(|_| "es".to_string());
    state.memory.propose_fusion(&profile_id, resolved.provider, &resolved.model, &language).await.map_err(map_err)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FusionApplyGroupInput {
    pub original_ids: Vec<String>,
    pub title: String,
    pub summary: Option<String>,
    pub content: String,
    pub category: String,
    pub importance: i64,
}

#[tauri::command]
pub async fn memory_apply_fusion(
    state: tauri::State<'_, AppState>,
    profile_id: String,
    groups: Vec<FusionApplyGroupInput>,
) -> Result<Vec<Memory>, AppError> {
    let groups = groups
        .into_iter()
        .map(|g| FusionGroupInput {
            original_ids: g.original_ids,
            title: g.title,
            summary: g.summary.filter(|s| !s.trim().is_empty()),
            content: g.content,
            category: g.category,
            importance: g.importance.clamp(0, 100),
        })
        .collect();
    state.memory.apply_fusion(&profile_id, groups).await.map_err(map_err)
}
