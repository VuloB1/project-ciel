//! Copia de seguridad completa de la app y su restauración.
//!
//! Una copia es un ZIP con: `manifest.json`, `aivt.db` (instantánea consistente
//! de la base, hecha con `VACUUM INTO` mientras la app sigue en marcha),
//! `attachments/` y `avatars/`. Se deja fuera lo que se puede volver a generar o
//! descargar (modelos, logs). **Los secretos (API keys, tokens) no se incluyen**:
//! viven en el almacén seguro del sistema y una copia que los llevara dentro sería
//! un archivo peligroso de compartir o perder.
//!
//! Restaurar no puede reemplazar la base mientras la app la tiene abierta, así que
//! va en dos pasos: [`stage_restore`] valida la copia y la deja en una carpeta de
//! preparación; al reiniciar, [`apply_pending_restore`] (antes de abrir la base)
//! hace el intercambio. Lo que había se conserva en `pre-restore/<fecha>/`: una
//! restauración nunca destruye los datos actuales.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use aivt_database::{inspect_backup_file, latest_migration_version, vacuum_into, DatabaseError, DbPool};

const FORMAT_VERSION: u32 = 1;
const APP_ID: &str = "project-ciel";
const MANIFEST: &str = "manifest.json";
const DB_ENTRY: &str = "aivt.db";
const DATA_DIRS: [&str; 2] = ["attachments", "avatars"];

const TMP_DIR: &str = "backup_tmp";
const PENDING_DIR: &str = "restore_pending";
const READY_MARKER: &str = "READY";
const PRE_RESTORE_DIR: &str = "pre-restore";

/// Tope de lo que se extrae al restaurar: una copia legítima de uso normal queda
/// muy por debajo, y un ZIP malicioso o corrupto no debe llenar el disco.
const MAX_TOTAL_BYTES: u64 = 32 * 1024 * 1024 * 1024;
const MAX_ENTRIES: usize = 500_000;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error("error de archivo: {0}")]
    Io(#[from] io::Error),
    #[error("archivo ZIP no válido: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("{0}")]
    Invalid(String),
    #[error("la tarea de copia falló: {0}")]
    Join(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    format: u32,
    app: String,
    app_version: String,
    created_at: String,
    schema_version: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSummary {
    pub path: String,
    pub size_bytes: u64,
    /// Archivos de adjuntos y avatares incluidos (sin contar la base).
    pub files: usize,
    pub created_at: String,
}

/// Lo que trae una copia ya validada y lista para restaurar.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePreview {
    pub created_at: String,
    pub app_version: String,
    pub files: usize,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreOutcome {
    /// Dónde quedaron los datos que había antes de restaurar.
    pub previous_data: String,
}

fn join_err(err: tokio::task::JoinError) -> BackupError {
    BackupError::Join(err.to_string())
}

/// Crea una copia completa en `dest`. La base se copia como instantánea
/// consistente y se comprueba su integridad ANTES de empaquetarla: una copia de
/// seguridad de una base dañada no sirve y es peor que no tener ninguna, porque
/// da una falsa seguridad.
pub async fn create_backup(pool: &DbPool, data_dir: &Path, dest: &Path) -> Result<BackupSummary, BackupError> {
    let tmp = data_dir.join(TMP_DIR);
    let _ = tokio::fs::remove_dir_all(&tmp).await;
    tokio::fs::create_dir_all(&tmp).await?;
    let result = create_inner(pool, data_dir, dest, &tmp).await;
    let _ = tokio::fs::remove_dir_all(&tmp).await;
    result
}

async fn create_inner(pool: &DbPool, data_dir: &Path, dest: &Path, tmp: &Path) -> Result<BackupSummary, BackupError> {
    let snapshot = tmp.join(DB_ENTRY);
    vacuum_into(pool, &snapshot).await?;
    let schema_version = inspect_backup_file(&snapshot).await?;

    let manifest = Manifest {
        format: FORMAT_VERSION,
        app: APP_ID.to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        schema_version,
    };

    let data_dir = data_dir.to_path_buf();
    let dest_owned = dest.to_path_buf();
    let manifest_for_zip = manifest.clone();
    let (files, size_bytes) = tokio::task::spawn_blocking(move || write_zip(&data_dir, &snapshot, &dest_owned, &manifest_for_zip))
        .await
        .map_err(join_err)??;

    Ok(BackupSummary { path: dest.display().to_string(), size_bytes, files, created_at: manifest.created_at })
}

fn write_zip(data_dir: &Path, snapshot: &Path, dest: &Path, manifest: &Manifest) -> Result<(usize, u64), BackupError> {
    let partial = PathBuf::from(format!("{}.partial", dest.display()));
    let result = write_zip_to(data_dir, snapshot, &partial, manifest).and_then(|files| {
        verify_zip(&partial)?;
        fs::rename(&partial, dest)?;
        Ok(files)
    });
    if result.is_err() {
        let _ = fs::remove_file(&partial);
    }
    let files = result?;
    Ok((files, fs::metadata(dest)?.len()))
}

fn write_zip_to(data_dir: &Path, snapshot: &Path, target: &Path, manifest: &Manifest) -> Result<usize, BackupError> {
    let mut zip = ZipWriter::new(File::create(target)?);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated).large_file(true);

    zip.start_file(MANIFEST, options)?;
    zip.write_all(&serde_json::to_vec_pretty(manifest).map_err(|e| BackupError::Invalid(e.to_string()))?)?;

    zip.start_file(DB_ENTRY, options)?;
    io::copy(&mut File::open(snapshot)?, &mut zip)?;

    let mut files = 0;
    for dir in DATA_DIRS {
        add_dir(&mut zip, &data_dir.join(dir), dir, options, &mut files)?;
    }
    zip.finish()?;
    Ok(files)
}

fn add_dir(zip: &mut ZipWriter<File>, dir: &Path, prefix: &str, options: SimpleFileOptions, files: &mut usize) -> Result<(), BackupError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err.into()),
    };
    for entry in entries {
        let entry = entry?;
        let file_type = entry.file_type()?;
        // Los enlaces simbólicos se saltan: podrían apuntar fuera de la carpeta de datos.
        if file_type.is_symlink() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            tracing::warn!(target: "system", "backup: se omite un archivo con nombre no válido en {}", dir.display());
            continue;
        };
        let entry_name = format!("{prefix}/{name}");
        if file_type.is_dir() {
            add_dir(zip, &entry.path(), &entry_name, options, files)?;
        } else if file_type.is_file() {
            zip.start_file(&entry_name, options)?;
            io::copy(&mut File::open(entry.path())?, zip)?;
            *files += 1;
        }
    }
    Ok(())
}

/// Relee lo escrito: al leer una entrada hasta el final el ZIP comprueba su CRC,
/// así que esto detecta un disco que escribió mal sin tener que releer todo.
fn verify_zip(path: &Path) -> Result<(), BackupError> {
    let mut archive = ZipArchive::new(File::open(path)?)?;
    for name in [MANIFEST, DB_ENTRY] {
        let mut entry = archive.by_name(name)?;
        io::copy(&mut entry, &mut io::sink())?;
    }
    Ok(())
}

/// Valida `zip_path` y lo extrae a la carpeta de preparación, sin tocar nada de lo
/// que está en uso. Si algo no cuadra (no es una copia, es de una app más nueva,
/// la base está dañada...) falla y no deja nada a medias.
pub async fn stage_restore(zip_path: &Path, data_dir: &Path) -> Result<RestorePreview, BackupError> {
    let pending = data_dir.join(PENDING_DIR);
    let _ = tokio::fs::remove_dir_all(&pending).await;

    let result = stage_inner(zip_path, &pending).await;
    if result.is_err() {
        let _ = tokio::fs::remove_dir_all(&pending).await;
    }
    result
}

async fn stage_inner(zip_path: &Path, pending: &Path) -> Result<RestorePreview, BackupError> {
    let zip_owned = zip_path.to_path_buf();
    let pending_owned = pending.to_path_buf();
    let (manifest, files, size_bytes) =
        tokio::task::spawn_blocking(move || extract_to(&zip_owned, &pending_owned)).await.map_err(join_err)??;

    // La instantánea ya está en disco: se comprueba la base REAL que va a quedar en
    // uso, no solo lo que dice el manifiesto.
    let actual_version = inspect_backup_file(&pending.join(DB_ENTRY)).await?;
    if actual_version > latest_migration_version() {
        return Err(newer_app_error());
    }

    tokio::fs::write(pending.join(READY_MARKER), serde_json::to_vec(&manifest).map_err(|e| BackupError::Invalid(e.to_string()))?).await?;
    Ok(RestorePreview { created_at: manifest.created_at, app_version: manifest.app_version, files, size_bytes })
}

fn newer_app_error() -> BackupError {
    BackupError::Invalid("la copia la hizo una versión más nueva de la app: actualiza la app antes de restaurarla".to_string())
}

fn extract_to(zip_path: &Path, pending: &Path) -> Result<(Manifest, usize, u64), BackupError> {
    let mut archive = ZipArchive::new(File::open(zip_path)?)
        .map_err(|_| BackupError::Invalid("el archivo no es una copia de seguridad válida".to_string()))?;
    if archive.len() > MAX_ENTRIES {
        return Err(BackupError::Invalid("la copia tiene demasiados archivos".to_string()));
    }

    let manifest: Manifest = {
        let entry = archive
            .by_name(MANIFEST)
            .map_err(|_| BackupError::Invalid("el archivo no es una copia de seguridad de Project Ciel (falta el manifiesto)".to_string()))?;
        if entry.size() > MAX_MANIFEST_BYTES {
            return Err(BackupError::Invalid("el manifiesto de la copia es demasiado grande".to_string()));
        }
        serde_json::from_reader(entry.take(MAX_MANIFEST_BYTES))
            .map_err(|_| BackupError::Invalid("el manifiesto de la copia no se puede leer".to_string()))?
    };
    if manifest.app != APP_ID || manifest.format != FORMAT_VERSION {
        return Err(BackupError::Invalid("el archivo no es una copia de seguridad compatible".to_string()));
    }
    if manifest.schema_version > latest_migration_version() {
        return Err(newer_app_error());
    }

    fs::create_dir_all(pending)?;
    let mut total: u64 = 0;
    let mut files = 0usize;
    let mut has_db = false;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if entry.is_dir() {
            continue;
        }
        let relative = entry
            .enclosed_name()
            .ok_or_else(|| BackupError::Invalid("la copia contiene una ruta no permitida".to_string()))?;
        let Some(name) = safe_entry_name(&relative) else {
            return Err(BackupError::Invalid("la copia contiene una ruta no permitida".to_string()));
        };

        let is_db = name == DB_ENTRY;
        let is_data = name.split('/').count() >= 2 && DATA_DIRS.contains(&name.split('/').next().unwrap_or(""));
        // El manifiesto ya se leyó; cualquier otra cosa que no sea la base o los
        // datos conocidos se ignora en vez de escribirse en disco.
        if !is_db && !is_data {
            continue;
        }

        let out_path = pending.join(&relative);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let remaining = MAX_TOTAL_BYTES.saturating_sub(total);
        let copied = io::copy(&mut (&mut entry).take(remaining + 1), &mut File::create(&out_path)?)?;
        total += copied;
        if total > MAX_TOTAL_BYTES {
            return Err(BackupError::Invalid("la copia es demasiado grande".to_string()));
        }
        if is_db {
            has_db = true;
        } else {
            files += 1;
        }
    }

    if !has_db {
        return Err(BackupError::Invalid("la copia no contiene la base de datos".to_string()));
    }
    Ok((manifest, files, total))
}

/// Nombre de entrada con `/` como separador, o `None` si algún componente no es
/// un nombre normal (`..`, raíz, unidad) o trae `:` (en Windows `archivo:flujo`
/// escribiría en un flujo alterno de datos).
fn safe_entry_name(path: &Path) -> Option<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                let part = part.to_str()?;
                if part.contains(':') || part.contains('\\') {
                    return None;
                }
                parts.push(part.to_string());
            }
            _ => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// Si hay una restauración preparada (y completa), la aplica. Hay que llamarla
/// **antes** de abrir la base de datos. Lo anterior se mueve a
/// `pre-restore/<fecha>/`, no se borra. Si algo falla a medias, se deshace y los
/// datos vuelven a quedar como estaban.
pub fn apply_pending_restore(data_dir: &Path) -> Result<Option<RestoreOutcome>, BackupError> {
    let pending = data_dir.join(PENDING_DIR);
    if !pending.exists() {
        return Ok(None);
    }
    if !pending.join(READY_MARKER).is_file() {
        // Una preparación que se cortó a medias: nunca se aplica.
        let _ = fs::remove_dir_all(&pending);
        return Ok(None);
    }
    if !pending.join(DB_ENTRY).is_file() {
        let _ = fs::remove_dir_all(&pending);
        return Err(BackupError::Invalid("la restauración preparada no tiene base de datos".to_string()));
    }

    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let previous = data_dir.join(PRE_RESTORE_DIR).join(stamp);
    fs::create_dir_all(&previous)?;

    let mut moved_away: Vec<(PathBuf, PathBuf)> = Vec::new();
    for name in [DB_ENTRY, "aivt.db-wal", "aivt.db-shm", DATA_DIRS[0], DATA_DIRS[1]] {
        let source = data_dir.join(name);
        if !source.exists() {
            continue;
        }
        let target = previous.join(name);
        if let Err(err) = rename_with_retry(&source, &target) {
            undo(&moved_away, &[]);
            return Err(err.into());
        }
        moved_away.push((source, target));
    }

    let mut installed: Vec<PathBuf> = Vec::new();
    for name in [DB_ENTRY, DATA_DIRS[0], DATA_DIRS[1]] {
        let source = pending.join(name);
        if !source.exists() {
            continue;
        }
        let target = data_dir.join(name);
        if let Err(err) = rename_with_retry(&source, &target) {
            undo(&moved_away, &installed);
            return Err(err.into());
        }
        installed.push(target);
    }

    let _ = fs::remove_dir_all(&pending);
    Ok(Some(RestoreOutcome { previous_data: previous.display().to_string() }))
}

/// Renombra reintentando unos instantes. En Windows un archivo recién cerrado puede
/// seguir bloqueado un momento (antivirus, indexador, el propio cierre de la
/// conexión) y el renombrado falla con "sharing violation"; casi siempre se libera
/// en menos de un segundo. Un error que no sea de bloqueo se devuelve de inmediato.
fn rename_with_retry(from: &Path, to: &Path) -> io::Result<()> {
    const ATTEMPTS: u32 = 25;
    const WAIT: std::time::Duration = std::time::Duration::from_millis(200);
    let mut last = None;
    for attempt in 0..ATTEMPTS {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(err) if is_locked(&err) => {
                last = Some(err);
                if attempt + 1 < ATTEMPTS {
                    std::thread::sleep(WAIT);
                }
            }
            Err(err) => return Err(err),
        }
    }
    Err(last.unwrap_or_else(|| io::Error::other("renombrado sin intentos")))
}

/// Windows: 32 = ERROR_SHARING_VIOLATION, 33 = ERROR_LOCK_VIOLATION; PermissionDenied
/// también aparece cuando otro proceso tiene el archivo abierto.
fn is_locked(err: &io::Error) -> bool {
    matches!(err.raw_os_error(), Some(32) | Some(33)) || err.kind() == io::ErrorKind::PermissionDenied
}

/// Deshace una restauración a medias: quita lo ya instalado y devuelve lo que se
/// había apartado a su sitio.
fn undo(moved_away: &[(PathBuf, PathBuf)], installed: &[PathBuf]) {
    for path in installed {
        if path.is_dir() {
            let _ = fs::remove_dir_all(path);
        } else {
            let _ = fs::remove_file(path);
        }
    }
    for (original, moved_to) in moved_away.iter().rev() {
        let _ = rename_with_retry(moved_to, original);
    }
}

/// Cancela una restauración ya preparada (el usuario se arrepintió antes de reiniciar).
pub async fn discard_pending_restore(data_dir: &Path) {
    let _ = tokio::fs::remove_dir_all(data_dir.join(PENDING_DIR)).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn app_data(dir: &Path) -> DbPool {
        let pool = aivt_database::connect(&dir.join("aivt.db")).await.unwrap();
        aivt_database::migrate(&pool).await.unwrap();
        pool
    }

    async fn seed_profile(pool: &DbPool, name: &str) -> String {
        aivt_database::ProfilesRepository::new(pool)
            .create(aivt_database::NewProfile { name: name.to_string(), nickname: None, description: None })
            .await
            .unwrap()
            .id
    }

    fn write(path: &Path, bytes: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    fn zip_entries(path: &Path) -> Vec<String> {
        let mut archive = ZipArchive::new(File::open(path).unwrap()).unwrap();
        (0..archive.len()).map(|i| archive.by_index(i).unwrap().name().to_string()).collect()
    }

    /// Un ZIP armado a mano, para probar entradas que `create_backup` jamás produciría.
    fn crafted_zip(path: &Path, manifest: &Manifest, entries: &[(&str, &[u8])]) {
        let mut zip = ZipWriter::new(File::create(path).unwrap());
        let options = SimpleFileOptions::default();
        zip.start_file(MANIFEST, options).unwrap();
        zip.write_all(&serde_json::to_vec(manifest).unwrap()).unwrap();
        for (name, bytes) in entries {
            zip.start_file(*name, options).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
    }

    fn manifest_with(schema_version: i64) -> Manifest {
        Manifest {
            format: FORMAT_VERSION,
            app: APP_ID.to_string(),
            app_version: "0.0.0".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            schema_version,
        }
    }

    #[tokio::test]
    async fn a_backup_holds_the_database_attachments_and_avatars_but_not_models_or_logs() {
        let dir = tempfile::tempdir().unwrap();
        let pool = app_data(dir.path()).await;
        write(&dir.path().join("attachments/ab/foto.png"), b"png");
        write(&dir.path().join("avatars/p1/avatar.png"), b"avatar");
        write(&dir.path().join("models/whisper/modelo.bin"), b"enorme");
        write(&dir.path().join("logs/aivt.log"), b"log");
        let dest = dir.path().join("copia.aivtbackup");

        let summary = create_backup(&pool, dir.path(), &dest).await.unwrap();

        let names = zip_entries(&dest);
        assert!(names.contains(&MANIFEST.to_string()) && names.contains(&DB_ENTRY.to_string()));
        assert!(names.contains(&"attachments/ab/foto.png".to_string()));
        assert!(names.contains(&"avatars/p1/avatar.png".to_string()));
        assert!(names.iter().all(|n| !n.starts_with("models") && !n.starts_with("logs")), "{names:?}");
        assert_eq!(summary.files, 2);
        assert!(summary.size_bytes > 0);
        assert!(!dir.path().join(TMP_DIR).exists(), "no deja la carpeta temporal");
        assert!(!PathBuf::from(format!("{}.partial", dest.display())).exists());
    }

    #[tokio::test]
    async fn a_backup_never_contains_a_live_wal_or_shm_file() {
        let dir = tempfile::tempdir().unwrap();
        let pool = app_data(dir.path()).await;
        let dest = dir.path().join("copia.aivtbackup");
        create_backup(&pool, dir.path(), &dest).await.unwrap();

        let names = zip_entries(&dest);
        assert!(names.iter().all(|n| !n.ends_with("-wal") && !n.ends_with("-shm")), "{names:?}");
    }

    #[tokio::test]
    async fn restoring_a_backup_brings_back_data_and_files_in_a_fresh_data_dir() {
        let source = tempfile::tempdir().unwrap();
        let pool = app_data(source.path()).await;
        seed_profile(&pool, "Ciel").await;
        write(&source.path().join("attachments/ab/foto.png"), b"png");
        write(&source.path().join("avatars/p1/avatar.png"), b"avatar");
        let zip = source.path().join("copia.aivtbackup");
        create_backup(&pool, source.path(), &zip).await.unwrap();

        let target = tempfile::tempdir().unwrap();
        let preview = stage_restore(&zip, target.path()).await.unwrap();
        assert_eq!(preview.files, 2);
        assert!(apply_pending_restore(target.path()).unwrap().is_some());

        let restored = aivt_database::connect(&target.path().join("aivt.db")).await.unwrap();
        let profiles = aivt_database::ProfilesRepository::new(&restored).list().await.unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].name, "Ciel");
        assert_eq!(fs::read(target.path().join("attachments/ab/foto.png")).unwrap(), b"png");
        assert_eq!(fs::read(target.path().join("avatars/p1/avatar.png")).unwrap(), b"avatar");
        assert!(!target.path().join(PENDING_DIR).exists());
    }

    #[tokio::test]
    async fn restoring_over_existing_data_keeps_the_old_data_aside() {
        let source = tempfile::tempdir().unwrap();
        let source_pool = app_data(source.path()).await;
        seed_profile(&source_pool, "Copia").await;
        let zip = source.path().join("copia.aivtbackup");
        create_backup(&source_pool, source.path(), &zip).await.unwrap();

        let current = tempfile::tempdir().unwrap();
        let current_pool = app_data(current.path()).await;
        seed_profile(&current_pool, "Actual").await;
        write(&current.path().join("attachments/zz/vieja.png"), b"vieja");
        current_pool.close().await;

        stage_restore(&zip, current.path()).await.unwrap();
        let outcome = apply_pending_restore(current.path()).unwrap().unwrap();

        let previous = PathBuf::from(&outcome.previous_data);
        assert!(previous.join("aivt.db").is_file(), "la base anterior se conserva");
        assert_eq!(fs::read(previous.join("attachments/zz/vieja.png")).unwrap(), b"vieja");
        assert!(!current.path().join("attachments/zz/vieja.png").exists(), "los adjuntos viejos no se mezclan con los restaurados");

        let restored = aivt_database::connect(&current.path().join("aivt.db")).await.unwrap();
        let names: Vec<String> =
            aivt_database::ProfilesRepository::new(&restored).list().await.unwrap().into_iter().map(|p| p.name).collect();
        assert_eq!(names, vec!["Copia".to_string()]);
    }

    #[tokio::test]
    async fn staging_does_not_touch_the_data_in_use() {
        let source = tempfile::tempdir().unwrap();
        let source_pool = app_data(source.path()).await;
        let zip = source.path().join("copia.aivtbackup");
        create_backup(&source_pool, source.path(), &zip).await.unwrap();

        let current = tempfile::tempdir().unwrap();
        let _pool = app_data(current.path()).await;
        write(&current.path().join("attachments/ab/foto.png"), b"png");

        stage_restore(&zip, current.path()).await.unwrap();

        assert!(current.path().join("aivt.db").is_file());
        assert_eq!(fs::read(current.path().join("attachments/ab/foto.png")).unwrap(), b"png");
        assert!(current.path().join(PENDING_DIR).join(READY_MARKER).is_file());
    }

    #[tokio::test]
    async fn discarding_a_staged_restore_leaves_nothing_to_apply() {
        let source = tempfile::tempdir().unwrap();
        let source_pool = app_data(source.path()).await;
        let zip = source.path().join("copia.aivtbackup");
        create_backup(&source_pool, source.path(), &zip).await.unwrap();
        let current = tempfile::tempdir().unwrap();

        stage_restore(&zip, current.path()).await.unwrap();
        discard_pending_restore(current.path()).await;

        assert!(apply_pending_restore(current.path()).unwrap().is_none());
    }

    #[test]
    fn an_incomplete_staging_folder_is_never_applied() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join(PENDING_DIR).join(DB_ENTRY), b"a medias");
        write(&dir.path().join("aivt.db"), b"la base real");

        assert!(apply_pending_restore(dir.path()).unwrap().is_none());

        assert_eq!(fs::read(dir.path().join("aivt.db")).unwrap(), b"la base real");
        assert!(!dir.path().join(PENDING_DIR).exists());
    }

    #[tokio::test]
    async fn a_zip_with_a_path_traversal_entry_is_rejected_and_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("mala.aivtbackup");
        crafted_zip(&zip, &manifest_with(1), &[(DB_ENTRY, b"x"), ("attachments/../../fuera.txt", b"x")]);
        let data = dir.path().join("datos");
        fs::create_dir_all(&data).unwrap();

        assert!(stage_restore(&zip, &data).await.is_err());

        assert!(!dir.path().join("fuera.txt").exists());
        assert!(!data.join(PENDING_DIR).exists(), "no deja una preparación a medias");
    }

    #[tokio::test]
    async fn a_corrupt_database_is_rejected_before_it_can_replace_anything() {
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("rota.aivtbackup");
        crafted_zip(&zip, &manifest_with(1), &[(DB_ENTRY, b"esto no es una base de datos sqlite")]);
        let data = dir.path().join("datos");
        write(&data.join("aivt.db"), b"la base real");

        assert!(stage_restore(&zip, &data).await.is_err());

        assert_eq!(fs::read(data.join("aivt.db")).unwrap(), b"la base real");
        assert!(!data.join(PENDING_DIR).exists());
    }

    #[tokio::test]
    async fn a_backup_from_a_newer_app_is_rejected() {
        let source = tempfile::tempdir().unwrap();
        let pool = app_data(source.path()).await;
        let snapshot = source.path().join("snap.db");
        vacuum_into(&pool, &snapshot).await.unwrap();
        let db_bytes = fs::read(&snapshot).unwrap();
        let zip = source.path().join("nueva.aivtbackup");
        crafted_zip(&zip, &manifest_with(latest_migration_version() + 1), &[(DB_ENTRY, &db_bytes)]);
        let data = tempfile::tempdir().unwrap();

        let err = stage_restore(&zip, data.path()).await.unwrap_err();

        assert!(err.to_string().contains("más nueva"), "{err}");
    }

    #[tokio::test]
    async fn files_that_are_not_backups_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("datos");
        fs::create_dir_all(&data).unwrap();

        let not_a_zip = dir.path().join("texto.aivtbackup");
        fs::write(&not_a_zip, b"hola").unwrap();
        assert!(stage_restore(&not_a_zip, &data).await.is_err());

        let other_zip = dir.path().join("otro.zip");
        let mut zip = ZipWriter::new(File::create(&other_zip).unwrap());
        zip.start_file("algo.txt", SimpleFileOptions::default()).unwrap();
        zip.write_all(b"x").unwrap();
        zip.finish().unwrap();
        let err = stage_restore(&other_zip, &data).await.unwrap_err();
        assert!(err.to_string().contains("manifiesto"), "{err}");

        // El export de un perfil (`.aivtprofile`) también es un ZIP: no debe aceptarse como copia.
        let mut profile_like = manifest_with(1);
        profile_like.app = "otra-cosa".to_string();
        let foreign = dir.path().join("ajena.zip");
        crafted_zip(&foreign, &profile_like, &[(DB_ENTRY, b"x")]);
        assert!(stage_restore(&foreign, &data).await.is_err());
    }

    fn copy_dir(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for entry in fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_dir(&entry.path(), &target);
            } else {
                fs::copy(entry.path(), &target).unwrap();
            }
        }
    }

    /// Prueba manual con datos reales: `AIVT_REAL_DATA_DIR=<carpeta de datos> cargo test -p
    /// aivt-service-backup real_data -- --ignored --nocapture`. Trabaja sobre una COPIA en una
    /// carpeta temporal: nunca toca la carpeta original.
    #[tokio::test]
    #[ignore = "necesita AIVT_REAL_DATA_DIR"]
    async fn real_data_dir_round_trips_with_the_same_content() {
        let Ok(real) = std::env::var("AIVT_REAL_DATA_DIR") else { return };
        let work = tempfile::tempdir().unwrap();
        let live = work.path().join("live");
        fs::create_dir_all(&live).unwrap();
        for name in ["aivt.db", "aivt.db-wal", "aivt.db-shm"] {
            let source = Path::new(&real).join(name);
            if source.exists() {
                fs::copy(&source, live.join(name)).unwrap();
            }
        }
        for dir in DATA_DIRS {
            let source = Path::new(&real).join(dir);
            if source.exists() {
                copy_dir(&source, &live.join(dir));
            }
        }
        let pool = aivt_database::connect(&live.join("aivt.db")).await.unwrap();
        aivt_database::migrate(&pool).await.unwrap();
        let count = |table: &'static str| {
            let pool = pool.clone();
            async move { sqlx_count(&pool, table).await }
        };
        let before = (count("profiles").await, count("conversations").await, count("messages").await, count("memories").await);

        let zip = work.path().join("real.aivtbackup");
        let summary = create_backup(&pool, &live, &zip).await.unwrap();
        println!("copia: {} bytes, {} archivos", summary.size_bytes, summary.files);

        let target = work.path().join("restored");
        fs::create_dir_all(&target).unwrap();
        let preview = stage_restore(&zip, &target).await.unwrap();
        assert_eq!(preview.files, summary.files);
        apply_pending_restore(&target).unwrap().unwrap();

        let restored = aivt_database::connect(&target.join("aivt.db")).await.unwrap();
        aivt_database::integrity_check(&restored).await.unwrap();
        let after = (
            sqlx_count(&restored, "profiles").await,
            sqlx_count(&restored, "conversations").await,
            sqlx_count(&restored, "messages").await,
            sqlx_count(&restored, "memories").await,
        );
        println!("antes {before:?} / después {after:?}");
        assert_eq!(before, after);
    }

    async fn sqlx_count(pool: &DbPool, table: &str) -> i64 {
        aivt_database::row_count(pool, table).await.unwrap()
    }

    #[test]
    fn rename_with_retry_moves_files_and_fails_fast_on_errors_that_are_not_locks() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("a.txt"), b"x");
        rename_with_retry(&dir.path().join("a.txt"), &dir.path().join("b.txt")).unwrap();
        assert!(dir.path().join("b.txt").is_file());

        // Un archivo que no existe no es un bloqueo: debe fallar al instante, no tras 5 s de reintentos.
        let started = std::time::Instant::now();
        let err = rename_with_retry(&dir.path().join("nada.txt"), &dir.path().join("c.txt")).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
    }

    #[test]
    fn entry_names_with_streams_or_backslashes_are_not_safe() {
        assert!(safe_entry_name(Path::new("attachments/ab/foto.png")).is_some());
        assert!(safe_entry_name(Path::new("attachments/foto.png:secreto")).is_none());
        assert!(safe_entry_name(Path::new("attachments\\..\\x")).is_none() || cfg!(windows));
        assert!(safe_entry_name(Path::new("/etc/passwd")).is_none());
        assert!(safe_entry_name(Path::new("../x")).is_none());
    }
}
