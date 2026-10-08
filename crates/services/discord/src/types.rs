/// One message read off the Gateway's `MESSAGE_CREATE` event — trimmed
/// down to what the interaction policy and the character's response
/// actually need, not the full Discord payload shape. Mirrors
/// `aivt_service_twitch::TwitchChatMessage`.
#[derive(Debug, Clone)]
pub struct DiscordChatMessage {
    pub channel_id: String,
    pub author_id: String,
    pub author_display_name: String,
    /// `true` for any bot account, including this one — messages from bots
    /// are always ignored (see `policy::should_respond`) to avoid
    /// bot-to-bot reply loops, a concern Twitch never had since only one
    /// account (the streamer's own) could ever post there.
    pub author_is_bot: bool,
    pub text: String,
    /// Whether the message's own `mentions` field names this bot's user id
    /// — a real, structured signal (unlike Twitch's plain-text name-match
    /// workaround), computed once in `gateway.rs` from the raw payload so
    /// `policy::should_respond` doesn't need the bot's own id passed in.
    pub mentions_bot: bool,
}
