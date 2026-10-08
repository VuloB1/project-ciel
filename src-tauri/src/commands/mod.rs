//! Thin Tauri command layer: each command validates its input and delegates
//! to the matching service crate. No business logic lives here.
//!
//! `tauri::generate_handler!` needs the *actual* definition path of each
//! `#[tauri::command]` function (it wires up hidden sibling items the macro
//! generates alongside it) — a `pub use` re-export here is not enough, so
//! `lib.rs` references `commands::settings::*` directly rather than
//! `commands::*`.

pub mod activity;
pub mod avatar;
pub mod backup;
pub mod call;
pub mod chat;
pub mod diagnostics;
pub mod discord;
pub mod embedding;
pub mod memory;
pub mod mood;
pub mod notifications;
pub mod people;
pub mod proactive;
pub mod profiles;
pub mod settings;
pub mod stt;
pub mod twitch;
pub mod voice;
pub mod web_search;
pub mod window;
