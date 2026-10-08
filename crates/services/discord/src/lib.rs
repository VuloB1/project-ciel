//! `DiscordService`: a dedicated bot account (Developer Portal token, no
//! OAuth — see `DiscordService::connect_bot`), `twilight-gateway` for
//! reading messages, `DiscordPolicy` (via `policy::should_respond`) for
//! deciding when to reply, and `twilight-http` for posting the character's
//! replies back.

mod error;
mod gateway;
mod http;
mod policy;
mod service;
mod types;

pub use error::DiscordError;
pub use service::DiscordService;
pub use types::DiscordChatMessage;
