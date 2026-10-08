use aivt_core::{redact_secrets, AppError};
use aivt_service_diagnostics::{
    build_report, collect_local, render_text, AiInfo, DiagnosticsReport, LlmProviderInfo, Problem, ServiceInfo, SttModelInfo,
    TtsProviderInfo,
};
use aivt_service_settings::SettingsService;
use serde::Serialize;

use crate::state::AppState;

/// Cuántos eventos de actividad se revisan en busca de avisos y errores.
const ACTIVITY_SCAN_LIMIT: i64 = 200;

fn err(code: &'static str, error: impl std::fmt::Display) -> AppError {
    AppError::from_display("diagnostics", code, error)
}

/// Nombre estable de un enum serializable (`"fishaudio"`, `"running"`...), el mismo
/// que ve el frontend, sin tener que mantener un `match` paralelo.
fn name_of<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

async fn gather(state: &AppState) -> Result<DiagnosticsReport, AppError> {
    let local = collect_local(&state.pool, &state.data_dir).await.map_err(|e| err("DIAGNOSTICS_COLLECT_FAILED", e))?;

    let mut services: Vec<ServiceInfo> = state
        .service_manager
        .statuses()
        .await
        .into_iter()
        .map(|(name, status)| ServiceInfo { name, status: name_of(&status) })
        .collect();
    services.sort_by(|a, b| a.name.cmp(&b.name));

    let llm_settings = state.settings.llm_providers().await.map_err(|e| err("DIAGNOSTICS_SETTINGS_FAILED", e))?;
    let llm = llm_settings
        .providers
        .iter()
        .map(|p| LlmProviderInfo {
            label: p.label.clone(),
            kind: name_of(&p.kind),
            enabled: p.enabled,
            pinned: p.pinned,
            has_api_key: p.has_api_key,
            ready: p.is_ready(),
            current: llm_settings.current_provider_id.as_deref() == Some(p.id.as_str()),
            preferred: llm_settings.preferred_provider_id.as_deref() == Some(p.id.as_str()),
            cooldown_secs: SettingsService::llm_provider_cooldown_remaining_secs(&p.id),
        })
        .collect();

    let tts_settings = state.settings.tts_providers().await.map_err(|e| err("DIAGNOSTICS_SETTINGS_FAILED", e))?;
    let tts = tts_settings
        .providers
        .iter()
        .map(|p| TtsProviderInfo {
            label: p.label.clone(),
            kind: name_of(&p.kind),
            enabled: p.enabled,
            has_api_key: p.has_api_key,
            current: tts_settings.current_provider_id.as_deref() == Some(p.id.as_str()),
        })
        .collect();

    let selected_stt = state.settings.stt_model().await.map(|m| name_of(&m.size)).unwrap_or_default();
    let stt_models = state
        .stt_models
        .list_status()
        .into_iter()
        .map(|status| {
            let size = name_of(&status.size);
            SttModelInfo { selected: size == selected_stt, size, downloaded: status.downloaded }
        })
        .collect();

    let ai = AiInfo { llm, tts, stt_models, embedding_downloaded: state.embedding_models.is_downloaded() };

    // Un fallo al leer la actividad no impide el informe: solo no hay "problemas recientes".
    let problems = state
        .activity
        .recent(ACTIVITY_SCAN_LIMIT)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|row| matches!(row.level.as_str(), "WARNING" | "ERROR" | "CRITICAL"))
        .map(|row| Problem {
            timestamp: row.timestamp,
            level: row.level,
            category: row.category,
            module: row.module,
            summary: redact_secrets(&row.summary),
        })
        .collect();

    Ok(build_report(local, services, ai, problems, chrono::Utc::now().to_rfc3339()))
}

#[tauri::command]
pub async fn diagnostics_run(state: tauri::State<'_, AppState>) -> Result<DiagnosticsReport, AppError> {
    gather(&state).await
}

/// El informe como texto plano (ya sin secretos), para copiar al portapapeles.
#[tauri::command]
pub async fn diagnostics_text(state: tauri::State<'_, AppState>) -> Result<String, AppError> {
    Ok(render_text(&gather(&state).await?))
}

/// Guarda el informe como texto plano en `path`.
#[tauri::command]
pub async fn diagnostics_save(state: tauri::State<'_, AppState>, path: String) -> Result<(), AppError> {
    let text = render_text(&gather(&state).await?);
    tokio::fs::write(&path, text).await.map_err(|e| err("DIAGNOSTICS_WRITE_FAILED", e))
}
