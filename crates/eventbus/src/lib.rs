//! Typed internal pub/sub so modules never depend on each other directly.
//!
//! `src-tauri` subscribes once at startup and re-emits a filtered subset to
//! the frontend via Tauri's `emit`; the frontend consumes it through a typed
//! `useAppEvent` hook. See docs/architecture.md.

mod bus;
mod event;

pub use bus::EventBus;
pub use event::{AppEvent, CallState, ServiceStatus};
