//! Shared domain types used across the AIVT Companion workspace.
//!
//! This crate intentionally has almost no dependencies: every other crate
//! (database, secrets, providers, services...) may depend on `aivt-core`,
//! but `aivt-core` must never depend back on them. That keeps the domain
//! model portable (desktop today, mobile later) and testable in isolation.

mod context;
mod cooldown;
mod error;
mod redact;
mod text_match;

pub use context::{ChannelRef, InteractionContext, InteractionSource, UserRef};
pub use cooldown::CooldownTracker;
pub use error::AppError;
pub use redact::redact_secrets;
pub use text_match::contains_word;
