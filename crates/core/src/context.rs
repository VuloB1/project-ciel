use serde::{Deserialize, Serialize};

/// Where an interaction with the active AI profile originated from.
///
/// This is the type that enforces the project's "single identity" principle
/// (see docs/architecture.md): Chat, Call, Twitch and Discord are all just
/// different `source` values feeding the *same* `AIOrchestrator::respond`.
/// No channel-specific service is allowed to build its own prompt or talk to
/// a provider directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionSource {
    Chat,
    Call,
    Twitch,
    Discord,
}

/// Identifies the channel-specific place an interaction happened in
/// (a conversation id for Chat, a Twitch channel, a Discord guild+channel...).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelRef {
    pub id: String,
    pub label: Option<String>,
}

/// The end user on the other side of the interaction, when known.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRef {
    pub id: String,
    pub display_name: String,
}

/// Everything the `AIOrchestrator` needs to answer, regardless of which
/// channel it came from. Built by the calling `Service` (ChatService,
/// TwitchService, ...), never by the UI directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractionContext {
    pub source: InteractionSource,
    pub profile_id: String,
    pub channel: ChannelRef,
    pub user: Option<UserRef>,
    #[serde(default)]
    pub metadata: serde_json::Value,
    pub allow_memory: bool,
    pub allow_tts: bool,
    pub allow_tools: bool,
}
