use std::path::Path;
use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

use crate::DatabaseError;

pub type DbPool = sqlx::SqlitePool;

/// Opens (creating if missing) the SQLite database at `db_path`, in WAL mode
/// with foreign keys enforced. `db_path` is resolved by the caller (the
/// `src-tauri` bootstrap) from the OS app-data directory — this crate never
/// hardcodes a path itself, so it stays testable with a throwaway path or
/// `:memory:`.
pub async fn connect(db_path: &Path) -> Result<DbPool, DatabaseError> {
    if let Some(parent) = db_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path.display()))
        .map_err(sqlx::Error::from)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await?;

    Ok(pool)
}

/// Runs every migration under `crates/database/migrations` that hasn't been
/// applied yet. Safe to call on every app start — sqlx tracks applied
/// versions in its own `_sqlx_migrations` table.
pub async fn migrate(pool: &DbPool) -> Result<(), DatabaseError> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

/// Última versión de migración que conoce ESTA versión de la app. Sirve para
/// rechazar restaurar una copia hecha por una app más nueva (su esquema podría
/// tener tablas o columnas que esta no entiende).
pub fn latest_migration_version() -> i64 {
    sqlx::migrate!("./migrations").iter().map(|m| m.version).max().unwrap_or(0)
}

/// Copia consistente de la base EN VIVO a `dest` (`VACUUM INTO`): incluye lo que
/// todavía esté en el WAL y no bloquea a quien escribe. `dest` no debe existir.
pub async fn vacuum_into(pool: &DbPool, dest: &Path) -> Result<(), DatabaseError> {
    // `VACUUM INTO` no acepta parámetros enlazados en todas las versiones de
    // SQLite: se escapa la comilla simple a mano.
    let literal = dest.to_string_lossy().replace('\'', "''");
    sqlx::query(&format!("VACUUM INTO '{literal}'")).execute(pool).await?;
    Ok(())
}

/// `PRAGMA integrity_check` sobre una base ya abierta. Devuelve `Integrity` con
/// lo que reporte SQLite si algo está mal.
pub async fn integrity_check(pool: &DbPool) -> Result<(), DatabaseError> {
    let rows: Vec<String> = sqlx::query_scalar("PRAGMA integrity_check").fetch_all(pool).await?;
    if rows.len() == 1 && rows[0] == "ok" {
        Ok(())
    } else {
        Err(DatabaseError::Integrity(rows.into_iter().take(5).collect::<Vec<_>>().join("; ")))
    }
}

/// Abre `path` SOLO LECTURA (sin crearlo ni tocar su WAL), comprueba su
/// integridad y devuelve la versión de migración más alta que trae. Es lo que se
/// usa para validar una copia antes de dejar que reemplace a la base real.
pub async fn inspect_backup_file(path: &Path) -> Result<i64, DatabaseError> {
    let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
        .map_err(sqlx::Error::from)?
        .read_only(true)
        .immutable(true);
    let pool = SqlitePoolOptions::new().max_connections(1).connect_with(options).await?;
    let result = async {
        integrity_check(&pool).await?;
        let version: Option<i64> = sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations").fetch_one(&pool).await?;
        Ok(version.unwrap_or(0))
    }
    .await;
    pool.close().await;
    result
}

/// Cuántas filas tiene `table`. Solo para diagnóstico y pruebas: el nombre se
/// valida (solo letras, números y `_`) porque no se puede enlazar como parámetro.
pub async fn row_count(pool: &DbPool, table: &str) -> Result<i64, DatabaseError> {
    if table.is_empty() || !table.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(DatabaseError::NotFound(format!("invalid table name '{table}'")));
    }
    Ok(sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}")).fetch_one(pool).await?)
}

/// Versión de migración más alta ya aplicada a esta base (0 si ninguna).
pub async fn applied_schema_version(pool: &DbPool) -> Result<i64, DatabaseError> {
    let version: Option<i64> = sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations").fetch_one(pool).await?;
    Ok(version.unwrap_or(0))
}
