//! Lifecycle registry for background integrations (Twitch, Discord, Call,
//! memory consolidation timers...). Nothing runs unless it's both enabled in
//! Settings and explicitly started here — see docs/architecture.md §9.

mod error;
mod manager;
mod service;

pub use error::ServiceManagerError;
pub use manager::ServiceManager;
pub use service::ManagedService;
