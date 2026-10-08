use thiserror::Error;

#[derive(Debug, Error)]
pub enum SecretError {
    /// No OS keyring backend could be reached (e.g. no Secret Service
    /// running on a headless/minimal Linux install). Deliberately surfaced
    /// as its own variant rather than silently falling back to an
    /// unencrypted store — see docs/security.md and ADR-003.
    #[error("no OS credential store is available on this system: {0}")]
    Unavailable(String),

    #[error("secret store operation failed: {0}")]
    Backend(String),

    #[error("internal error: background task failed: {0}")]
    Task(String),
}
