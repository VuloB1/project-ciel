//! Serves the reactive PNG avatar (idle/thinking/talking, same 3-state
//! behavior as the old native-window overlay it replaces) as a self-hosted
//! OBS Browser Source: a small local HTTP+WebSocket server (see
//! `service.rs`/`routes.rs`), loopback-only, driven by the same `AppEvent`s
//! the rest of the app already publishes (see `reducer.rs`, a Rust port of
//! the frontend's `avatarStore.ts` state machine — a plain OBS/CEF browser
//! tab has no Tauri IPC bridge, so this can't reuse that bridge directly).

mod overlay_html;
mod reducer;
mod routes;
mod service;

pub use reducer::VisualState;
pub use service::AvatarServerService;
