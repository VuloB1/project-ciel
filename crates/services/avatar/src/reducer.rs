use aivt_eventbus::{AppEvent, CallState};

/// Mirrors the frontend's `AvatarVisualState` (`src/lib/ipc/avatar.ts`) — a
/// separate type from `aivt_database::AvatarState` on purpose: that one
/// names *file-storage columns* (which PNG a profile has uploaded for which
/// state), while this one is the *live, wire/reducer* state a viewer should
/// be showing right now. Keeping them separate means this crate has no
/// reason to depend on `aivt-database` at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VisualState {
    Idle,
    Thinking,
    Talking,
}

/// Per-connection state machine — ported 1:1 from the frontend's
/// `avatarStore.ts` (`handleAppEvent`), scoped to one `profile_id`. Each
/// WebSocket connection in `routes.rs` owns its own `Reducer` rather than
/// sharing one derived-state layer across connections — simpler, and at the
/// scale of "OBS has one browser source open" there's no cost to it.
pub struct Reducer {
    profile_id: String,
    visual_state: VisualState,
    /// Correlates a completion/error/cancel event back to the
    /// `AiResponseStarted` that set `Thinking` — those events only carry
    /// `request_id`, not `profile_id`, so this is how a different profile's
    /// concurrent generation finishing doesn't flip this one back to idle.
    active_request_id: Option<String>,
}

impl VisualState {
    /// Parses the `{state}` path segment of `GET /avatar/{profile_id}/{state}`
    /// (`routes.rs`) — the only other place a `VisualState` needs to round-trip
    /// through a plain string.
    pub fn from_url_param(value: &str) -> Option<Self> {
        match value {
            "idle" => Some(Self::Idle),
            "thinking" => Some(Self::Thinking),
            "talking" => Some(Self::Talking),
            _ => None,
        }
    }
}

impl Reducer {
    pub fn new(profile_id: String) -> Self {
        Self { profile_id, visual_state: VisualState::Idle, active_request_id: None }
    }

    pub fn state(&self) -> VisualState {
        self.visual_state
    }

    /// Returns `Some(new_state)` only on an actual transition, so callers
    /// only need to push a message when something visibly changed.
    pub fn apply(&mut self, event: &AppEvent) -> Option<VisualState> {
        let previous = self.visual_state;
        match event {
            AppEvent::AiResponseStarted { request_id, profile_id, .. } if *profile_id == self.profile_id => {
                self.visual_state = VisualState::Thinking;
                self.active_request_id = Some(request_id.clone());
            }
            AppEvent::AiResponseFinished { request_id } | AppEvent::AiResponseCancelled { request_id }
                if Some(request_id) == self.active_request_id.as_ref() =>
            {
                self.visual_state = VisualState::Idle;
                self.active_request_id = None;
            }
            AppEvent::AiResponseError { request_id, .. } if Some(request_id) == self.active_request_id.as_ref() => {
                self.visual_state = VisualState::Idle;
                self.active_request_id = None;
            }
            AppEvent::VoicePlaybackStarted { profile_id, .. } if *profile_id == self.profile_id => {
                self.visual_state = VisualState::Talking;
            }
            AppEvent::VoicePlaybackFinished { profile_id, .. } | AppEvent::VoicePlaybackError { profile_id, .. }
                if *profile_id == self.profile_id =>
            {
                self.visual_state = VisualState::Idle;
            }
            // Call events (Fase 6) carry no `profile_id` — same reasoning as
            // the frontend reducer: there's only ever one active call and
            // one avatar viewer at a time, so there's no cross-profile
            // mismatch to guard against here.
            AppEvent::CallStateChanged { state } => {
                self.visual_state = match state {
                    CallState::Transcribing | CallState::Thinking => VisualState::Thinking,
                    CallState::Speaking => VisualState::Talking,
                    CallState::Idle | CallState::Listening | CallState::Error => VisualState::Idle,
                };
            }
            _ => return None,
        }
        (self.visual_state != previous).then_some(self.visual_state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn started(profile_id: &str, request_id: &str) -> AppEvent {
        AppEvent::AiResponseStarted {
            request_id: request_id.to_string(),
            profile_id: profile_id.to_string(),
            conversation_id: "conv-1".to_string(),
            message_id: "msg-1".to_string(),
        }
    }

    #[test]
    fn ai_response_started_for_tracked_profile_moves_to_thinking() {
        let mut reducer = Reducer::new("p1".to_string());
        let transition = reducer.apply(&started("p1", "req-1"));
        assert_eq!(transition, Some(VisualState::Thinking));
        assert_eq!(reducer.state(), VisualState::Thinking);
    }

    #[test]
    fn ai_response_started_for_a_different_profile_is_ignored() {
        let mut reducer = Reducer::new("p1".to_string());
        let transition = reducer.apply(&started("p2", "req-1"));
        assert_eq!(transition, None);
        assert_eq!(reducer.state(), VisualState::Idle);
    }

    #[test]
    fn ai_response_finished_matching_active_request_returns_to_idle() {
        let mut reducer = Reducer::new("p1".to_string());
        reducer.apply(&started("p1", "req-1"));
        let transition = reducer.apply(&AppEvent::AiResponseFinished { request_id: "req-1".to_string() });
        assert_eq!(transition, Some(VisualState::Idle));
    }

    #[test]
    fn ai_response_finished_with_unrelated_request_id_is_ignored() {
        let mut reducer = Reducer::new("p1".to_string());
        reducer.apply(&started("p1", "req-1"));
        // Simulates a different, concurrent generation finishing.
        let transition = reducer.apply(&AppEvent::AiResponseFinished { request_id: "some-other-req".to_string() });
        assert_eq!(transition, None);
        assert_eq!(reducer.state(), VisualState::Thinking);
    }

    #[test]
    fn ai_response_cancelled_and_error_also_return_to_idle() {
        let mut cancelled = Reducer::new("p1".to_string());
        cancelled.apply(&started("p1", "req-1"));
        assert_eq!(cancelled.apply(&AppEvent::AiResponseCancelled { request_id: "req-1".to_string() }), Some(VisualState::Idle));

        let mut errored = Reducer::new("p1".to_string());
        errored.apply(&started("p1", "req-1"));
        let err_transition = errored.apply(&AppEvent::AiResponseError { request_id: "req-1".to_string(), message: "boom".to_string() });
        assert_eq!(err_transition, Some(VisualState::Idle));
    }

    #[test]
    fn voice_playback_round_trips_through_talking() {
        let mut reducer = Reducer::new("p1".to_string());
        let start = reducer.apply(&AppEvent::VoicePlaybackStarted { profile_id: "p1".to_string(), message_id: None });
        assert_eq!(start, Some(VisualState::Talking));

        let finish = reducer.apply(&AppEvent::VoicePlaybackFinished { profile_id: "p1".to_string(), message_id: None });
        assert_eq!(finish, Some(VisualState::Idle));
    }

    #[test]
    fn voice_playback_error_also_returns_to_idle() {
        let mut reducer = Reducer::new("p1".to_string());
        reducer.apply(&AppEvent::VoicePlaybackStarted { profile_id: "p1".to_string(), message_id: None });
        let transition = reducer.apply(&AppEvent::VoicePlaybackError {
            profile_id: "p1".to_string(),
            message_id: None,
            message: "boom".to_string(),
        });
        assert_eq!(transition, Some(VisualState::Idle));
    }

    #[test]
    fn voice_playback_for_a_different_profile_is_ignored() {
        let mut reducer = Reducer::new("p1".to_string());
        let transition = reducer.apply(&AppEvent::VoicePlaybackStarted { profile_id: "p2".to_string(), message_id: None });
        assert_eq!(transition, None);
    }

    #[test]
    fn call_state_changed_maps_phases_to_visual_states() {
        let mut reducer = Reducer::new("p1".to_string());

        assert_eq!(reducer.apply(&AppEvent::CallStateChanged { state: CallState::Transcribing }), Some(VisualState::Thinking));
        assert_eq!(reducer.apply(&AppEvent::CallStateChanged { state: CallState::Thinking }), None); // already Thinking
        assert_eq!(reducer.apply(&AppEvent::CallStateChanged { state: CallState::Speaking }), Some(VisualState::Talking));
        assert_eq!(reducer.apply(&AppEvent::CallStateChanged { state: CallState::Listening }), Some(VisualState::Idle));
        assert_eq!(reducer.apply(&AppEvent::CallStateChanged { state: CallState::Error }), None); // already Idle
    }

    #[test]
    fn apply_returns_none_when_state_does_not_actually_change() {
        let mut reducer = Reducer::new("p1".to_string());
        reducer.apply(&started("p1", "req-1"));
        // A second, unrelated event for the same profile that doesn't move
        // the needle — e.g. a duplicate `AiResponseStarted` isn't realistic,
        // but `CallStateChanged` re-affirming the same phase is a clean way
        // to exercise the no-op path without another event type.
        assert_eq!(reducer.apply(&AppEvent::CallStateChanged { state: CallState::Thinking }), None);
    }
}
