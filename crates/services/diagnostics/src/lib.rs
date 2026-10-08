//! Centro de diagnóstico: una foto del estado de la app en un solo informe.
//!
//! Este crate no depende de los servicios: recibe datos ya simples (estados de
//! servicios, proveedores, modelos, problemas recientes) y añade lo que se puede
//! medir desde aquí (base de datos, almacenamiento). De ahí calcula una lista de
//! comprobaciones con severidad. Las comprobaciones llevan un **código** y
//! parámetros, no texto: la interfaz las traduce a los 6 idiomas.
//!
//! El informe **nunca contiene secretos**: de los proveedores solo se dice si tienen
//! API key (sí/no), no su valor, y todo texto que viene de logs pasa por
//! `redact_secrets` (ver `docs/security.md`, "Diagnóstico y exports").

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use aivt_database::{integrity_check, latest_migration_version, row_count, DbPool};

/// Tablas cuyo número de filas se muestra: dan una idea del tamaño de los datos sin
/// exponer su contenido.
const COUNTED_TABLES: [&str; 6] = ["profiles", "conversations", "messages", "memories", "people", "activity_events"];

/// Cuántos problemas recientes se muestran.
const MAX_PROBLEMS: usize = 15;

#[derive(Debug, thiserror::Error)]
pub enum DiagnosticsError {
    #[error("la tarea de diagnóstico falló: {0}")]
    Join(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Ok,
    /// Dato útil que no es un problema (p. ej. un modelo opcional sin descargar).
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    /// Código estable que la interfaz traduce (`database_integrity`, `llm_not_ready`...).
    pub id: String,
    pub severity: Severity,
    /// Valores para rellenar el texto traducido (`{{label}}`, `{{seconds}}`...).
    pub params: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub app_version: String,
    pub os: String,
    pub arch: String,
    pub data_dir: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableCount {
    pub table: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseInfo {
    pub schema_version: i64,
    pub latest_schema_version: i64,
    pub size_bytes: u64,
    pub integrity_ok: bool,
    pub integrity_message: Option<String>,
    pub counts: Vec<TableCount>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageInfo {
    pub attachments_bytes: u64,
    pub avatars_bytes: u64,
    pub models_bytes: u64,
    pub logs_bytes: u64,
    /// Datos de antes de una restauración: se pueden borrar a mano para liberar espacio.
    pub pre_restore_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceInfo {
    pub name: String,
    /// `stopped`, `starting`, `running`, `error`, `stopping`.
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmProviderInfo {
    pub label: String,
    pub kind: String,
    pub enabled: bool,
    pub pinned: bool,
    pub has_api_key: bool,
    /// Con modelo y, si es de nube, con API key: en condiciones de responder.
    pub ready: bool,
    /// El que está respondiendo ahora.
    pub current: bool,
    /// El que el usuario eligió.
    pub preferred: bool,
    /// Segundos que le faltan para salir de la pausa tras fallar, si está en pausa.
    pub cooldown_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsProviderInfo {
    pub label: String,
    pub kind: String,
    pub enabled: bool,
    pub has_api_key: bool,
    pub current: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SttModelInfo {
    pub size: String,
    pub downloaded: bool,
    pub selected: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiInfo {
    pub llm: Vec<LlmProviderInfo>,
    pub tts: Vec<TtsProviderInfo>,
    pub stt_models: Vec<SttModelInfo>,
    pub embedding_downloaded: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    pub timestamp: String,
    pub level: String,
    pub category: String,
    pub module: String,
    /// Ya redactado (sin secretos).
    pub summary: String,
}

/// Lo que se puede medir sin conocer los servicios: se obtiene con [`collect_local`].
#[derive(Debug, Clone)]
pub struct LocalFacts {
    pub system: SystemInfo,
    pub database: DatabaseInfo,
    pub storage: StorageInfo,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsReport {
    /// La peor severidad entre las comprobaciones (`info` cuenta como `ok`).
    pub overall: Severity,
    pub generated_at: String,
    pub system: SystemInfo,
    pub database: DatabaseInfo,
    pub storage: StorageInfo,
    pub services: Vec<ServiceInfo>,
    pub ai: AiInfo,
    pub problems: Vec<Problem>,
    pub checks: Vec<Check>,
}

/// Mide la base de datos y el almacenamiento. Nunca falla por un problema de la base:
/// si la comprobación de integridad no se puede ni ejecutar, se informa como no
/// íntegra (con el motivo) en vez de abortar el informe, que es justo cuando más se
/// necesita.
pub async fn collect_local(pool: &DbPool, data_dir: &Path) -> Result<LocalFacts, DiagnosticsError> {
    let (integrity_ok, integrity_message) = match integrity_check(pool).await {
        Ok(()) => (true, None),
        Err(err) => (false, Some(err.to_string())),
    };

    let schema_version: i64 = sqlx_max_version(pool).await;
    let mut counts = Vec::new();
    for table in COUNTED_TABLES {
        // Una tabla que no se puede contar (p. ej. base dañada) se omite, no rompe el informe.
        if let Ok(count) = row_count(pool, table).await {
            counts.push(TableCount { table: table.to_string(), count });
        }
    }

    let data_dir_owned = data_dir.to_path_buf();
    let (database_size, storage) = tokio::task::spawn_blocking(move || {
        let db_size = ["aivt.db", "aivt.db-wal", "aivt.db-shm"].iter().map(|name| file_size(&data_dir_owned.join(name))).sum();
        let storage = StorageInfo {
            attachments_bytes: dir_size(&data_dir_owned.join("attachments")),
            avatars_bytes: dir_size(&data_dir_owned.join("avatars")),
            models_bytes: dir_size(&data_dir_owned.join("models")),
            logs_bytes: dir_size(&data_dir_owned.join("logs")),
            pre_restore_bytes: dir_size(&data_dir_owned.join("pre-restore")),
        };
        (db_size, storage)
    })
    .await
    .map_err(|err| DiagnosticsError::Join(err.to_string()))?;

    Ok(LocalFacts {
        system: SystemInfo {
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            data_dir: data_dir.display().to_string(),
        },
        database: DatabaseInfo {
            schema_version,
            latest_schema_version: latest_migration_version(),
            size_bytes: database_size,
            integrity_ok,
            integrity_message,
            counts,
        },
        storage,
    })
}

async fn sqlx_max_version(pool: &DbPool) -> i64 {
    // `aivt-database` expone solo helpers ya hechos; la versión aplicada se lee
    // con una consulta mínima. Si falla, 0 (se reportará como esquema distinto).
    aivt_database::applied_schema_version(pool).await.unwrap_or(0)
}

fn file_size(path: &Path) -> u64 {
    fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

/// Suma recursiva del tamaño de los archivos de una carpeta. Los enlaces simbólicos
/// no se siguen (podrían apuntar fuera de la carpeta de datos o formar ciclos).
fn dir_size(dir: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else { return 0 };
    let mut total = 0;
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else { continue };
        if file_type.is_symlink() {
            continue;
        }
        let path: PathBuf = entry.path();
        if file_type.is_dir() {
            total += dir_size(&path);
        } else if file_type.is_file() {
            total += file_size(&path);
        }
    }
    total
}

/// Arma el informe: junta los datos y calcula las comprobaciones.
pub fn build_report(
    local: LocalFacts,
    services: Vec<ServiceInfo>,
    ai: AiInfo,
    problems: Vec<Problem>,
    generated_at: String,
) -> DiagnosticsReport {
    let mut problems = problems;
    problems.truncate(MAX_PROBLEMS);
    let checks = compute_checks(&local, &services, &ai, &problems);
    let overall = checks.iter().map(|c| c.severity).filter(|s| *s != Severity::Info).max().unwrap_or(Severity::Ok);
    DiagnosticsReport {
        overall,
        generated_at,
        system: local.system,
        database: local.database,
        storage: local.storage,
        services,
        ai,
        problems,
        checks,
    }
}

fn check(id: &str, severity: Severity, params: &[(&str, String)]) -> Check {
    Check { id: id.to_string(), severity, params: params.iter().map(|(k, v)| (k.to_string(), v.clone())).collect() }
}

fn compute_checks(local: &LocalFacts, services: &[ServiceInfo], ai: &AiInfo, problems: &[Problem]) -> Vec<Check> {
    let mut checks = Vec::new();

    // Base de datos.
    if local.database.integrity_ok {
        checks.push(check("database_integrity", Severity::Ok, &[]));
    } else {
        let message = local.database.integrity_message.clone().unwrap_or_default();
        checks.push(check("database_integrity", Severity::Error, &[("message", message)]));
    }
    if local.database.schema_version == local.database.latest_schema_version {
        checks.push(check("database_schema", Severity::Ok, &[]));
    } else {
        checks.push(check(
            "database_schema",
            Severity::Warning,
            &[("version", local.database.schema_version.to_string()), ("latest", local.database.latest_schema_version.to_string())],
        ));
    }

    // Proveedores de IA.
    let ready: Vec<&LlmProviderInfo> = ai.llm.iter().filter(|p| p.enabled && p.ready).collect();
    if ready.is_empty() {
        checks.push(check("llm_not_ready", Severity::Warning, &[]));
    } else {
        match ai.llm.iter().find(|p| p.current) {
            Some(current) if !(current.enabled && current.ready) => {
                checks.push(check("llm_current_not_ready", Severity::Warning, &[("label", current.label.clone())]));
            }
            Some(current) if current.cooldown_secs.is_some() => {
                checks.push(check(
                    "llm_cooling_down",
                    Severity::Warning,
                    &[("label", current.label.clone()), ("seconds", current.cooldown_secs.unwrap_or(0).to_string())],
                ));
            }
            Some(_) => checks.push(check("llm_ready", Severity::Ok, &[])),
            None => checks.push(check("llm_no_current", Severity::Warning, &[])),
        }
        // Con un solo proveedor listo el cambio automático no tiene a dónde pasar.
        if ready.iter().filter(|p| p.pinned).count() < 2 {
            checks.push(check("llm_no_fallback", Severity::Info, &[]));
        }
    }

    // Voz: un motor de nube sin key no puede hablar.
    if let Some(tts) = ai.tts.iter().find(|p| p.current) {
        if tts.kind != "native" && !tts.has_api_key && tts.kind != "http" {
            checks.push(check("tts_missing_key", Severity::Warning, &[("label", tts.label.clone())]));
        } else {
            checks.push(check("tts_ready", Severity::Ok, &[]));
        }
    }

    // Servicios.
    for service in services {
        if service.status == "error" {
            checks.push(check("service_error", Severity::Error, &[("name", service.name.clone())]));
        }
    }

    // Modelos locales opcionales.
    if let Some(selected) = ai.stt_models.iter().find(|m| m.selected) {
        if !selected.downloaded {
            checks.push(check("stt_model_missing", Severity::Info, &[("size", selected.size.clone())]));
        }
    }
    if !ai.embedding_downloaded {
        checks.push(check("embedding_missing", Severity::Info, &[]));
    }

    // Problemas recientes en la actividad.
    let recent_errors = problems.iter().filter(|p| p.level == "ERROR" || p.level == "CRITICAL").count();
    if recent_errors > 0 {
        checks.push(check("recent_errors", Severity::Warning, &[("count", recent_errors.to_string())]));
    }

    // Espacio recuperable.
    if local.storage.pre_restore_bytes > 0 {
        checks.push(check("pre_restore_leftover", Severity::Info, &[("bytes", local.storage.pre_restore_bytes.to_string())]));
    }

    checks
}

fn human_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let units = ["KB", "MB", "GB", "TB"];
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit < units.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", units[unit])
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "sí"
    } else {
        "no"
    }
}

/// El informe como texto plano para copiar o guardar y compartir (por ejemplo, al
/// pedir ayuda). Siempre pasa por `redact_secrets`: es la última barrera si algún
/// texto de un log llevara una credencial.
pub fn render_text(report: &DiagnosticsReport) -> String {
    let mut out = String::new();
    let mut line = |text: String| {
        out.push_str(&text);
        out.push('\n');
    };

    line("== Diagnóstico de Project Ciel ==".to_string());
    line(format!("Generado: {}", report.generated_at));
    line(format!("Estado general: {:?}", report.overall));
    line(String::new());

    line("-- Sistema --".to_string());
    line(format!("Versión de la app: {}", report.system.app_version));
    line(format!("Sistema: {} ({})", report.system.os, report.system.arch));
    line(format!("Datos en: {}", report.system.data_dir));
    line(String::new());

    line("-- Base de datos --".to_string());
    line(format!("Integridad: {}", if report.database.integrity_ok { "correcta".to_string() } else { format!("FALLA: {}", report.database.integrity_message.clone().unwrap_or_default()) }));
    line(format!("Esquema: {} (última conocida: {})", report.database.schema_version, report.database.latest_schema_version));
    line(format!("Tamaño: {}", human_bytes(report.database.size_bytes)));
    for count in &report.database.counts {
        line(format!("  {}: {}", count.table, count.count));
    }
    line(String::new());

    line("-- Almacenamiento --".to_string());
    line(format!("Adjuntos: {}", human_bytes(report.storage.attachments_bytes)));
    line(format!("Avatares: {}", human_bytes(report.storage.avatars_bytes)));
    line(format!("Modelos: {}", human_bytes(report.storage.models_bytes)));
    line(format!("Registros (logs): {}", human_bytes(report.storage.logs_bytes)));
    line(format!("Datos previos a una restauración: {}", human_bytes(report.storage.pre_restore_bytes)));
    line(String::new());

    line("-- Servicios --".to_string());
    for service in &report.services {
        line(format!("{}: {}", service.name, service.status));
    }
    line(String::new());

    line("-- Proveedores de IA --".to_string());
    for p in &report.ai.llm {
        let mut tags = Vec::new();
        if p.current {
            tags.push("en uso".to_string());
        }
        if p.preferred {
            tags.push("preferido".to_string());
        }
        if let Some(secs) = p.cooldown_secs {
            tags.push(format!("en pausa {secs} s"));
        }
        line(format!(
            "{} [{}]: activado {}, fijado {}, key {}, listo {}{}",
            p.label,
            p.kind,
            yes_no(p.enabled),
            yes_no(p.pinned),
            yes_no(p.has_api_key),
            yes_no(p.ready),
            if tags.is_empty() { String::new() } else { format!(" ({})", tags.join(", ")) }
        ));
    }
    line("-- Voz --".to_string());
    for p in &report.ai.tts {
        line(format!(
            "{} [{}]: activado {}, key {}{}",
            p.label,
            p.kind,
            yes_no(p.enabled),
            yes_no(p.has_api_key),
            if p.current { " (en uso)" } else { "" }
        ));
    }
    line("-- Modelos locales --".to_string());
    for m in &report.ai.stt_models {
        line(format!("Transcripción {}: {}{}", m.size, if m.downloaded { "descargado" } else { "no descargado" }, if m.selected { " (elegido)" } else { "" }));
    }
    line(format!("Búsqueda semántica: {}", if report.ai.embedding_downloaded { "descargado" } else { "no descargado" }));
    line(String::new());

    line("-- Comprobaciones --".to_string());
    for c in &report.checks {
        let params = c.params.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(", ");
        line(format!("[{:?}] {}{}", c.severity, c.id, if params.is_empty() { String::new() } else { format!(" ({params})") }));
    }
    line(String::new());

    line("-- Problemas recientes --".to_string());
    if report.problems.is_empty() {
        line("(ninguno)".to_string());
    }
    for p in &report.problems {
        line(format!("{} {} {}/{}: {}", p.timestamp, p.level, p.category, p.module, p.summary));
    }

    aivt_core::redact_secrets(&out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local() -> LocalFacts {
        LocalFacts {
            system: SystemInfo { app_version: "0.1.0".into(), os: "windows".into(), arch: "x86_64".into(), data_dir: "C:/datos".into() },
            database: DatabaseInfo {
                schema_version: 19,
                latest_schema_version: 19,
                size_bytes: 1024,
                integrity_ok: true,
                integrity_message: None,
                counts: vec![TableCount { table: "profiles".into(), count: 1 }],
            },
            storage: StorageInfo { attachments_bytes: 0, avatars_bytes: 0, models_bytes: 0, logs_bytes: 0, pre_restore_bytes: 0 },
        }
    }

    fn llm(label: &str) -> LlmProviderInfo {
        LlmProviderInfo {
            label: label.into(),
            kind: "generic".into(),
            enabled: true,
            pinned: true,
            has_api_key: false,
            ready: true,
            current: false,
            preferred: false,
            cooldown_secs: None,
        }
    }

    fn healthy_ai() -> AiInfo {
        AiInfo {
            llm: vec![LlmProviderInfo { current: true, preferred: true, ..llm("LM Studio") }, llm("Groq")],
            tts: vec![TtsProviderInfo { label: "Nativo".into(), kind: "native".into(), enabled: true, has_api_key: false, current: true }],
            stt_models: vec![SttModelInfo { size: "base".into(), downloaded: true, selected: true }],
            embedding_downloaded: true,
        }
    }

    fn report(local: LocalFacts, services: Vec<ServiceInfo>, ai: AiInfo, problems: Vec<Problem>) -> DiagnosticsReport {
        build_report(local, services, ai, problems, "2026-10-02T00:00:00Z".into())
    }

    fn find<'a>(report: &'a DiagnosticsReport, id: &str) -> Option<&'a Check> {
        report.checks.iter().find(|c| c.id == id)
    }

    #[test]
    fn a_healthy_setup_reports_ok_overall() {
        let r = report(local(), vec![], healthy_ai(), vec![]);
        assert_eq!(r.overall, Severity::Ok);
        assert_eq!(find(&r, "llm_ready").unwrap().severity, Severity::Ok);
        assert!(find(&r, "llm_no_fallback").is_none(), "con dos proveedores fijados sí hay reserva");
    }

    #[test]
    fn a_failed_integrity_check_is_an_error_with_the_reason() {
        let mut facts = local();
        facts.database.integrity_ok = false;
        facts.database.integrity_message = Some("fila 3 dañada".into());
        let r = report(facts, vec![], healthy_ai(), vec![]);
        assert_eq!(r.overall, Severity::Error);
        assert_eq!(find(&r, "database_integrity").unwrap().params["message"], "fila 3 dañada");
    }

    #[test]
    fn a_schema_mismatch_is_a_warning() {
        let mut facts = local();
        facts.database.schema_version = 17;
        let r = report(facts, vec![], healthy_ai(), vec![]);
        assert_eq!(find(&r, "database_schema").unwrap().severity, Severity::Warning);
        assert_eq!(r.overall, Severity::Warning);
    }

    #[test]
    fn no_provider_ready_to_answer_is_a_warning() {
        let mut ai = healthy_ai();
        for p in &mut ai.llm {
            p.ready = false;
        }
        let r = report(local(), vec![], ai, vec![]);
        assert_eq!(find(&r, "llm_not_ready").unwrap().severity, Severity::Warning);
    }

    #[test]
    fn the_current_provider_being_paused_is_reported_with_its_wait() {
        let mut ai = healthy_ai();
        ai.llm[0].cooldown_secs = Some(42);
        let r = report(local(), vec![], ai, vec![]);
        let c = find(&r, "llm_cooling_down").unwrap();
        assert_eq!(c.severity, Severity::Warning);
        assert_eq!(c.params["label"], "LM Studio");
        assert_eq!(c.params["seconds"], "42");
    }

    #[test]
    fn a_single_ready_provider_is_only_informational_because_failover_has_nowhere_to_go() {
        let mut ai = healthy_ai();
        ai.llm.truncate(1);
        let r = report(local(), vec![], ai, vec![]);
        assert_eq!(find(&r, "llm_no_fallback").unwrap().severity, Severity::Info);
        assert_eq!(r.overall, Severity::Ok, "una nota informativa no vuelve el estado general un problema");
    }

    #[test]
    fn a_cloud_voice_without_a_key_is_a_warning_but_a_local_server_is_not() {
        let mut ai = healthy_ai();
        ai.tts = vec![TtsProviderInfo { label: "Fish Audio".into(), kind: "fishaudio".into(), enabled: true, has_api_key: false, current: true }];
        assert_eq!(find(&report(local(), vec![], ai.clone(), vec![]), "tts_missing_key").unwrap().severity, Severity::Warning);

        ai.tts[0].kind = "http".into();
        assert!(find(&report(local(), vec![], ai, vec![]), "tts_missing_key").is_none());
    }

    #[test]
    fn a_service_in_error_is_an_error_naming_it() {
        let services = vec![ServiceInfo { name: "twitch".into(), status: "error".into() }, ServiceInfo { name: "call".into(), status: "running".into() }];
        let r = report(local(), services, healthy_ai(), vec![]);
        let c = find(&r, "service_error").unwrap();
        assert_eq!(c.params["name"], "twitch");
        assert_eq!(r.overall, Severity::Error);
    }

    #[test]
    fn missing_optional_models_are_informational() {
        let mut ai = healthy_ai();
        ai.stt_models[0].downloaded = false;
        ai.embedding_downloaded = false;
        let r = report(local(), vec![], ai, vec![]);
        assert_eq!(find(&r, "stt_model_missing").unwrap().severity, Severity::Info);
        assert_eq!(find(&r, "embedding_missing").unwrap().severity, Severity::Info);
        assert_eq!(r.overall, Severity::Ok);
    }

    #[test]
    fn recent_errors_are_counted_and_warn() {
        let problem = |level: &str| Problem { timestamp: "t".into(), level: level.into(), category: "SYSTEM".into(), module: "m".into(), summary: "algo".into() };
        let r = report(local(), vec![], healthy_ai(), vec![problem("ERROR"), problem("WARNING"), problem("CRITICAL")]);
        assert_eq!(find(&r, "recent_errors").unwrap().params["count"], "2");
    }

    #[test]
    fn leftover_pre_restore_data_is_flagged_as_reclaimable_space() {
        let mut facts = local();
        facts.storage.pre_restore_bytes = 5000;
        let r = report(facts, vec![], healthy_ai(), vec![]);
        assert_eq!(find(&r, "pre_restore_leftover").unwrap().params["bytes"], "5000");
    }

    #[test]
    fn the_text_report_masks_secrets_that_slipped_into_a_log_line() {
        let problem = Problem {
            timestamp: "t".into(),
            level: "ERROR".into(),
            category: "LLM".into(),
            module: "m".into(),
            summary: "falló con Authorization: Bearer abcdefghijklmnopqrstuvwx y api_key=sk-ABCDEFGHIJKLMNOP".into(),
        };
        let text = render_text(&report(local(), vec![], healthy_ai(), vec![problem]));
        assert!(!text.contains("abcdefghijklmnopqrstuvwx"), "{text}");
        assert!(!text.contains("sk-ABCDEFGHIJKLMNOP"), "{text}");
        assert!(text.contains("Bearer ******"));
    }

    #[test]
    fn the_report_never_carries_api_key_values_only_whether_one_exists() {
        let json = serde_json::to_string(&report(local(), vec![], healthy_ai(), vec![])).unwrap();
        assert!(json.contains("hasApiKey"));
        assert!(!json.to_lowercase().contains("apikey\":\""), "no debe haber un valor de key: {json}");
    }

    #[test]
    fn problems_are_capped() {
        let problems = (0..40)
            .map(|i| Problem { timestamp: i.to_string(), level: "ERROR".into(), category: "c".into(), module: "m".into(), summary: "s".into() })
            .collect();
        assert_eq!(report(local(), vec![], healthy_ai(), problems).problems.len(), MAX_PROBLEMS);
    }

    #[test]
    fn directory_sizes_add_up_recursively_and_a_missing_folder_is_zero() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("a/b")).unwrap();
        fs::write(dir.path().join("a/x.bin"), vec![0u8; 100]).unwrap();
        fs::write(dir.path().join("a/b/y.bin"), vec![0u8; 50]).unwrap();
        assert_eq!(dir_size(&dir.path().join("a")), 150);
        assert_eq!(dir_size(&dir.path().join("no-existe")), 0);
    }

    #[tokio::test]
    async fn collecting_local_facts_on_a_real_database_reports_integrity_schema_and_sizes() {
        let dir = tempfile::tempdir().unwrap();
        let pool = aivt_database::connect(&dir.path().join("aivt.db")).await.unwrap();
        aivt_database::migrate(&pool).await.unwrap();
        fs::create_dir_all(dir.path().join("attachments/ab")).unwrap();
        fs::write(dir.path().join("attachments/ab/f.png"), vec![0u8; 2048]).unwrap();
        fs::create_dir_all(dir.path().join("pre-restore/x")).unwrap();
        fs::write(dir.path().join("pre-restore/x/old.db"), vec![0u8; 10]).unwrap();

        let facts = collect_local(&pool, dir.path()).await.unwrap();

        assert!(facts.database.integrity_ok);
        assert_eq!(facts.database.schema_version, facts.database.latest_schema_version);
        assert!(facts.database.size_bytes > 0);
        assert!(facts.database.counts.iter().any(|c| c.table == "profiles" && c.count == 0));
        assert_eq!(facts.storage.attachments_bytes, 2048);
        assert_eq!(facts.storage.pre_restore_bytes, 10);
        assert_eq!(facts.storage.models_bytes, 0);
    }
}
