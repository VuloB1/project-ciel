use thiserror::Error;

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error(transparent)]
    Database(#[from] aivt_database::DatabaseError),

    #[error(transparent)]
    Llm(#[from] aivt_provider_llm::ProviderError),

    #[error("no se pudo interpretar la respuesta del modelo como memorias válidas")]
    ExtractionParseFailed,
}
