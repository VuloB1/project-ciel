pub(crate) use aivt_core::{contains_word, CooldownTracker};
use aivt_service_settings::TwitchPolicy;

use crate::types::TwitchChatMessage;

/// Decides whether an incoming chat message should get a reply, given the
/// user's saved policy. `respond_to_all` takes precedence over
/// `mentioned_only` when both are set — a redundant but harmless
/// combination; if neither is set the character never replies on its own
/// even with `enabled: true` (nothing to trigger it).
///
/// `mentions_character` is caller-computed (see
/// `Inner::mentions_active_character` in `service.rs`) because it needs a
/// DB read for the active profile's name/nickname — a plain-text name
/// match is how `mentioned_only` mode gets triggered when neither
/// official-@mention signal fires. `message.mentions_broadcaster` (an
/// @mention of whichever channel this message came from) and
/// `message.mentions_bot` (an @mention of the connected/bot account
/// itself) both count — when the bot posts in its own channel (no target
/// channel set) these two are always the same message, but once it posts
/// in someone else's channel a viewer might address either the streamer
/// or the bot by name, and both should reach the character.
///
/// Always skips: the connected account's own messages (it would otherwise
/// reply to itself), blank text, and anything starting with `!` (the
/// near-universal convention for *other* bots' commands — replying to
/// those would make the character look like it's talking to bots instead
/// of the audience).
pub fn should_respond(policy: &TwitchPolicy, message: &TwitchChatMessage, own_login: &str, mentions_character: bool) -> bool {
    if !policy.enabled {
        return false;
    }
    if message.chatter_login.eq_ignore_ascii_case(own_login) {
        return false;
    }
    let text = message.text.trim();
    if text.is_empty() || text.starts_with('!') {
        return false;
    }
    if policy.respond_to_all {
        return true;
    }
    policy.mentioned_only && (message.mentions_broadcaster || message.mentions_bot || mentions_character)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_message() -> TwitchChatMessage {
        TwitchChatMessage {
            message_id: "m1".to_string(),
            chatter_user_id: "2".to_string(),
            chatter_login: "vulito".to_string(),
            chatter_display_name: "Vulito".to_string(),
            text: "hola personaje".to_string(),
            mentions_broadcaster: false,
            mentions_bot: false,
        }
    }

    fn enabled_policy() -> TwitchPolicy {
        TwitchPolicy { enabled: true, respond_to_all: false, mentioned_only: true, cooldown_seconds: 30 }
    }

    #[test]
    fn disabled_policy_never_responds() {
        let policy = TwitchPolicy { enabled: false, ..enabled_policy() };
        assert!(!should_respond(&policy, &sample_message(), "personaje_bot", false));
    }

    #[test]
    fn ignores_its_own_messages() {
        let mut message = sample_message();
        message.chatter_login = "Personaje_Bot".to_string();
        assert!(!should_respond(&enabled_policy(), &message, "personaje_bot", false));
    }

    #[test]
    fn ignores_bot_commands() {
        let mut message = sample_message();
        message.text = "!uptime".to_string();
        message.mentions_broadcaster = true;
        assert!(!should_respond(&enabled_policy(), &message, "personaje_bot", true));
    }

    #[test]
    fn mentioned_only_requires_a_mention() {
        let message = sample_message();
        assert!(!should_respond(&enabled_policy(), &message, "personaje_bot", false));

        let mut mentioned = sample_message();
        mentioned.mentions_broadcaster = true;
        assert!(should_respond(&enabled_policy(), &mentioned, "personaje_bot", false));
    }

    #[test]
    fn mentioning_the_character_by_name_counts_as_a_mention() {
        let message = sample_message();
        assert!(should_respond(&enabled_policy(), &message, "personaje_bot", true));
    }

    #[test]
    fn mentioning_the_bot_account_directly_counts_as_a_mention() {
        let mut mentioned = sample_message();
        mentioned.mentions_bot = true;
        assert!(should_respond(&enabled_policy(), &mentioned, "personaje_bot", false));
    }

    #[test]
    fn respond_to_all_ignores_the_mention_requirement() {
        let policy = TwitchPolicy { respond_to_all: true, mentioned_only: false, ..enabled_policy() };
        assert!(should_respond(&policy, &sample_message(), "personaje_bot", false));
    }
}
