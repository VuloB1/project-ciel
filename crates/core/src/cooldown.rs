use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Tracks the last time a character actually replied in a chat integration
/// (Twitch, Discord), independent of each integration's own `should_respond`
/// checks — a hard floor so a burst of qualifying messages (e.g. several
/// people mentioning the channel/bot at once) can't make the character
/// reply faster than `cooldown_seconds` apart.
pub struct CooldownTracker {
    last_response_at: Mutex<Option<Instant>>,
}

impl CooldownTracker {
    pub fn new() -> Self {
        Self { last_response_at: Mutex::new(None) }
    }

    pub fn ready(&self, cooldown_seconds: i64) -> bool {
        let guard = self.last_response_at.lock().expect("cooldown mutex poisoned");
        match *guard {
            None => true,
            Some(last) => last.elapsed() >= Duration::from_secs(cooldown_seconds.max(0) as u64),
        }
    }

    pub fn mark_responded(&self) {
        *self.last_response_at.lock().expect("cooldown mutex poisoned") = Some(Instant::now());
    }
}

impl Default for CooldownTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cooldown_blocks_a_second_response_immediately_after_the_first() {
        let tracker = CooldownTracker::new();
        assert!(tracker.ready(30));
        tracker.mark_responded();
        assert!(!tracker.ready(30));
    }

    #[test]
    fn zero_cooldown_never_blocks() {
        let tracker = CooldownTracker::new();
        tracker.mark_responded();
        assert!(tracker.ready(0));
    }
}
