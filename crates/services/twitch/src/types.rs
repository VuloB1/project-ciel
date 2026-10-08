use serde::{Deserialize, Serialize};

/// One message read from EventSub's `channel.chat.message` event —
/// trimmed down to what the interaction policy and the character's
/// response actually need, not the full Twitch payload shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwitchChatMessage {
    pub message_id: String,
    pub chatter_user_id: String,
    pub chatter_login: String,
    pub chatter_display_name: String,
    pub text: String,
    /// Whether the message `@`-mentions the channel this message came from
    /// — Twitch's payload includes structured "fragments" that already
    /// identify mentions, so this doesn't need to be guessed with regex.
    /// When the connected account posts in its own channel (no target
    /// channel set — see `SettingsService::set_twitch_target_channel`),
    /// this and `mentions_bot` are always identical.
    pub mentions_broadcaster: bool,
    /// Whether the message `@`-mentions the *connected/bot* account
    /// specifically, distinct from `mentions_broadcaster` once that
    /// account posts in a channel other than its own. Mirrors
    /// `aivt_service_discord::DiscordChatMessage::mentions_bot`.
    pub mentions_bot: bool,
}

/// No `Debug` derive — this carries live OAuth secrets, and a stray
/// `{:?}`/`?field` tracing call is exactly how `TtsBackend`'s `api_key`
/// leaked a real ElevenLabs key into the log file on disk (found live,
/// fixed in `aivt_provider_tts::types`). Nothing today needs to print this;
/// leaving `Debug` off means a future accidental log call fails to compile
/// instead of silently leaking.
#[derive(Clone)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
}
