use serde::Serialize;
use std::fmt;

/// The single error shape that ever crosses the Tauri IPC boundary.
///
/// Downstream crates each define their own `thiserror` error enum (see
/// `database::DatabaseError`, `secrets::SecretError`, ...). Rather than
/// wiring up `From` conversions across crate boundaries (which would force
/// either a dependency cycle or putting every error variant in this crate),
/// the `src-tauri` command layer maps each crate's error into an `AppError`
/// explicitly at the point it's handled. That keeps this crate dependency-free
/// and keeps the mapping visible at the one place it actually matters: the
/// boundary the user sees.
#[derive(Debug, Clone, Serialize)]
pub struct AppError {
    pub module: String,
    pub code: String,
    pub message: String,
}

impl AppError {
    pub fn new(module: impl Into<String>, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            module: module.into(),
            code: code.into(),
            message: message.into(),
        }
    }

    /// Convenience for wrapping any displayable error without hand-writing
    /// `.to_string()` at every call site.
    pub fn from_display(module: impl Into<String>, code: impl Into<String>, err: impl fmt::Display) -> Self {
        Self::new(module, code, err.to_string())
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}::{}] {}", self.module, self.code, self.message)
    }
}

impl std::error::Error for AppError {}
