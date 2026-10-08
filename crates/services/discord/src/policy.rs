pub(crate) use aivt_core::{contains_word, CooldownTracker};
use aivt_service_settings::DiscordPolicy;

use crate::types::DiscordChatMessage;

/// Decides whether an incoming Discord message should get a reply, given
/// the user's saved policy. Mirrors
/// `aivt_service_twitch::policy::should_respond`, with two differences
/// forced by Discord actually being a multi-account platform (Twitch only
/// ever has the streamer's own account posting):
///
/// - Every message from a bot account is skipped, not just this bot's own
///   messages — avoids bot-to-bot reply loops in servers that have other
///   bots too.
/// - `mentioned_only` accepts a *real* @-mention of this bot
///   (`message.mentions_bot`, computed from Discord's structured mentions)
///   as well as the plain-text name/nickname match Twitch relies on —
///   either counts.
pub fn should_respond(policy: &DiscordPolicy, message: &DiscordChatMessage, mentions_character: bool) -> bool {
    if !policy.enabled {
        return false;
    }
    if message.author_is_bot {
        return false;
    }
    let text = message.text.trim();
    if text.is_empty() || text.starts_with('!') {
        return false;
    }
    if policy.respond_to_all {
        return true;
    }
    policy.mentioned_only && (message.mentions_bot || mentions_character)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_message() -> DiscordChatMessage {
        DiscordChatMessage {
            channel_id: "1".to_string(),
            author_id: "2".to_string(),
            author_display_name: "Vulito".to_string(),
            author_is_bot: false,
            text: "hola personaje".to_string(),
            mentions_bot: false,
        }
    }

    fn enabled_policy() -> DiscordPolicy {
        DiscordPolicy { enabled: true, respond_to_all: false, mentioned_only: true, cooldown_seconds: 30 }
    }

    #[test]
    fn disabled_policy_never_responds() {
        let policy = DiscordPolicy { enabled: false, ..enabled_policy() };
        assert!(!should_respond(&policy, &sample_message(), false));
    }

    #[test]
    fn ignores_messages_from_any_bot_not_just_itself() {
        let mut message = sample_message();
        message.author_is_bot = true;
        message.mentions_bot = true;
        assert!(!should_respond(&enabled_policy(), &message, true));
    }

    #[test]
    fn ignores_bot_commands() {
        let mut message = sample_message();
        message.text = "!uptime".to_string();
        message.mentions_bot = true;
        assert!(!should_respond(&enabled_policy(), &message, true));
    }

    #[test]
    fn mentioned_only_requires_a_mention() {
        let message = sample_message();
        assert!(!should_respond(&enabled_policy(), &message, false));

        let mut mentioned = sample_message();
        mentioned.mentions_bot = true;
        assert!(should_respond(&enabled_policy(), &mentioned, false));
    }

    #[test]
    fn mentioning_the_character_by_name_counts_as_a_mention() {
        let message = sample_message();
        assert!(should_respond(&enabled_policy(), &message, true));
    }

    #[test]
    fn respond_to_all_ignores_the_mention_requirement() {
        let policy = DiscordPolicy { respond_to_all: true, mentioned_only: false, ..enabled_policy() };
        assert!(should_respond(&policy, &sample_message(), false));
    }
}
