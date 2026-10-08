use thiserror::Error;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("database query failed: {0}")]
    Query(#[from] sqlx::Error),

    #[error("migration failed: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("could not prepare data directory: {0}")]
    Io(#[from] std::io::Error),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("database integrity check failed: {0}")]
    Integrity(String),
}
