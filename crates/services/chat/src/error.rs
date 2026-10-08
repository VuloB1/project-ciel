use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChatError {
    #[error(transparent)]
    Database(#[from] aivt_database::DatabaseError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Memory(#[from] aivt_memory::MemoryError),

    #[error("conversation not found")]
    ConversationNotFound,

    /// Concurrency policy for sección 88 del brief: a second message can't
    /// be sent to the same conversation while one is still generating —
    /// reject rather than queue or interrupt. The frontend disables the
    /// composer while streaming so this should be rare in practice; this
    /// is the server-side backstop.
    #[error("ya hay una respuesta generándose en esta conversación")]
    GenerationInProgress,

    #[error("adjunto no encontrado")]
    AttachmentNotFound,
}
