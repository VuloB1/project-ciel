use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use aivt_database::Message;

/// How many past exchanges (chatter message + character reply, so 2
/// `Message`s each) to keep per chatter. Kept short on purpose — Twitch
/// chat moves fast and a long history mostly just bloats every prompt for
/// marginal benefit; this is enough for "responde a lo que me dijiste hace
/// un par de mensajes" without dragging in an entire stream's worth of
/// unrelated chatter.
const MAX_TURNS_PER_CHATTER: usize = 3;

/// A chatter's context is dropped once they've been quiet this long.
/// Without this, a multi-hour stream with hundreds of distinct chatters
/// would accumulate one entry per chatter forever — nobody ever comes back
/// to a Twitch conversation from an hour ago, so there's no point keeping
/// it around. 30 minutes comfortably covers a short "brb" without carrying
/// context across two unrelated segments of a stream.
const CHATTER_IDLE_TIMEOUT: Duration = Duration::from_secs(30 * 60);

struct ChatterEntry {
    history: VecDeque<Message>,
    last_active: Instant,
}

/// Short-lived, in-memory conversation memory per Twitch chatter — so a
/// reply to someone's second message actually knows what their first one
/// said, instead of every reply being generated as if it were the only
/// thing anyone ever said (pedido del usuario: sin esto, una respuesta al
/// segundo mensaje de alguien no entendía de qué se le estaba hablando).
///
/// Deliberately NOT persisted to the database: Twitch chat is ephemeral by
/// nature and a channel can have thousands of distinct chatters across a
/// stream's lifetime — keeping this in memory, capped per chatter and
/// pruned on idle, avoids an ever-growing table for context nobody will
/// ever look back at. Losing it on app restart is an accepted tradeoff,
/// not a bug.
pub struct ChatterContext {
    entries: Mutex<HashMap<String, ChatterEntry>>,
    idle_timeout: Duration,
}

impl ChatterContext {
    pub fn new() -> Self {
        Self::with_idle_timeout(CHATTER_IDLE_TIMEOUT)
    }

    fn with_idle_timeout(idle_timeout: Duration) -> Self {
        Self { entries: Mutex::new(HashMap::new()), idle_timeout }
    }

    /// The chatter's history so far, oldest first — empty for a chatter
    /// seen for the first time, or whose last message aged out.
    pub fn history_for(&self, chatter_id: &str) -> Vec<Message> {
        let mut entries = self.entries.lock().expect("chatter context mutex poisoned");
        self.prune_idle(&mut entries);
        entries.get(chatter_id).map(|entry| entry.history.iter().cloned().collect()).unwrap_or_default()
    }

    /// Appends the chatter's message and the character's reply to that
    /// chatter's history, trimming down to `MAX_TURNS_PER_CHATTER`
    /// exchanges (oldest dropped first).
    pub fn record_exchange(&self, chatter_id: &str, chatter_message: Message, character_message: Message) {
        let mut entries = self.entries.lock().expect("chatter context mutex poisoned");
        let entry = entries.entry(chatter_id.to_string()).or_insert_with(|| ChatterEntry { history: VecDeque::new(), last_active: Instant::now() });
        entry.history.push_back(chatter_message);
        entry.history.push_back(character_message);
        while entry.history.len() > MAX_TURNS_PER_CHATTER * 2 {
            entry.history.pop_front();
        }
        entry.last_active = Instant::now();
    }

    fn prune_idle(&self, entries: &mut HashMap<String, ChatterEntry>) {
        entries.retain(|_, entry| entry.last_active.elapsed() < self.idle_timeout);
    }
}

impl Default for ChatterContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aivt_database::{MessageRole, MessageStatus};

    fn message(role: MessageRole, content: &str) -> Message {
        Message {
            id: "test".to_string(),
            conversation_id: "twitch".to_string(),
            role,
            content: content.to_string(),
            status: MessageStatus::Complete,
            provider_id: None,
            model: None,
            input_tokens: None,
            output_tokens: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    #[test]
    fn a_never_seen_chatter_has_no_history() {
        let context = ChatterContext::new();
        assert!(context.history_for("chatter-1").is_empty());
    }

    #[test]
    fn records_exchanges_in_order() {
        let context = ChatterContext::new();
        context.record_exchange("chatter-1", message(MessageRole::User, "hola"), message(MessageRole::Assistant, "hola!"));
        context.record_exchange("chatter-1", message(MessageRole::User, "como estas"), message(MessageRole::Assistant, "bien!"));

        let history = context.history_for("chatter-1");
        let contents: Vec<&str> = history.iter().map(|m| m.content.as_str()).collect();
        assert_eq!(contents, vec!["hola", "hola!", "como estas", "bien!"]);
    }

    #[test]
    fn caps_history_at_max_turns_per_chatter() {
        let context = ChatterContext::new();
        for i in 0..(MAX_TURNS_PER_CHATTER + 2) {
            context.record_exchange("chatter-1", message(MessageRole::User, &format!("msg {i}")), message(MessageRole::Assistant, &format!("reply {i}")));
        }

        let history = context.history_for("chatter-1");
        assert_eq!(history.len(), MAX_TURNS_PER_CHATTER * 2);
        // The oldest exchanges were dropped first — the earliest surviving
        // message is from the exchange that pushed the count over the cap.
        assert_eq!(history[0].content, "msg 2");
    }

    #[test]
    fn different_chatters_have_independent_history() {
        let context = ChatterContext::new();
        context.record_exchange("chatter-1", message(MessageRole::User, "de chatter 1"), message(MessageRole::Assistant, "respuesta 1"));
        context.record_exchange("chatter-2", message(MessageRole::User, "de chatter 2"), message(MessageRole::Assistant, "respuesta 2"));

        assert_eq!(context.history_for("chatter-1")[0].content, "de chatter 1");
        assert_eq!(context.history_for("chatter-2")[0].content, "de chatter 2");
    }

    #[test]
    fn idle_chatters_are_pruned() {
        let context = ChatterContext::with_idle_timeout(Duration::from_millis(10));
        context.record_exchange("chatter-1", message(MessageRole::User, "hola"), message(MessageRole::Assistant, "hola!"));
        std::thread::sleep(Duration::from_millis(30));

        assert!(context.history_for("chatter-1").is_empty());
    }
}
