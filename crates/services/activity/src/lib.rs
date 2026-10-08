//! Structured activity log: the data behind the future Actividad tab
//! (Fase 11) and the manual verification query for Fase 0.
//!
//! Two ways to write an event:
//! 1. Call [`ActivityService::record`] directly from application code.
//! 2. Emit a `tracing` event with `target: "activity"` — [`ActivityLayer`]
//!    picks it up and forwards it here. By convention, an activity-target
//!    event must include a `summary` field; `category`, `module`, `action`,
//!    `status`, and the various id fields are optional and default to
//!    something reasonable (see `layer.rs`).
//!
//! Both paths funnel through the same unbounded channel into one background
//! task that owns the actual `sqlx` writes, so a slow disk never blocks the
//! call site (a hot chat/Twitch/Discord loop) on a DB write.

mod layer;
mod service;

pub use aivt_database::{ActivityEventRow, ActivityLevel, NewActivityEvent};
pub use layer::ActivityLayer;
pub use service::ActivityService;
