use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServiceManagerError {
    #[error("no service registered with name '{0}'")]
    UnknownService(String),

    #[error("service '{service}' failed to start: {reason}")]
    StartFailed { service: String, reason: String },

    #[error("service '{service}' failed to stop: {reason}")]
    StopFailed { service: String, reason: String },
}
