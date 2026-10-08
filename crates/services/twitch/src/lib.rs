//! `TwitchService`: OAuth (Device Code Grant — see `auth`'s doc comment
//! for why, not Authorization Code), EventSub WebSocket chat reading,
//! `TwitchInteractionPolicy` (via `policy::should_respond`), and posting
//! the character's replies back via Helix. Channel points reward
//! redemptions are deliberately out of scope for Fase 8 — see
//! docs/DEVELOPMENT_STATUS.md.

mod auth;
mod context;
mod emotes;
mod error;
mod eventsub;
mod helix;
mod policy;
mod service;
mod types;

pub use auth::DeviceCodeFlow;
pub use error::TwitchError;
pub use service::{effective_broadcaster_id, TwitchService};
pub use types::TwitchChatMessage;

/// AIVT Companion's own Twitch application, registered as a "Público"
/// client (see `auth`'s doc comment) — a Client ID is a public identifier
/// by design in OAuth, meant to travel with the client app itself (every
/// distributed SPA/desktop app ships one the same way), not a secret that
/// needs per-user configuration or `SecretStore`. Pedido explícito del
/// usuario: un único Client ID para todo el producto, en vez de pedirle a
/// cada usuario que registre su propia aplicación en el Twitch Developer
/// Console antes de poder conectar una cuenta.
pub const CLIENT_ID: &str = "eort5ufraiqh2oobwxh8ex8nkbmjs3";
