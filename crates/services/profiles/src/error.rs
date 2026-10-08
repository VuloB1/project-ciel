use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProfileError {
    #[error(transparent)]
    Database(#[from] aivt_database::DatabaseError),

    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),

    #[error("profile not found")]
    NotFound,

    #[error("could not read exported profile: {0}")]
    InvalidExport(#[from] serde_json::Error),

    #[error("could not process avatar image: {0}")]
    Image(#[from] image::ImageError),

    #[error(transparent)]
    ExportFormat(#[from] crate::export::ExportFormatError),
}
