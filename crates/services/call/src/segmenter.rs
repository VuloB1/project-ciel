use aivt_service_settings::VadMode as SettingsVadMode;
use webrtc_vad::{SampleRate as VadSampleRate, Vad, VadMode as WebRtcVadMode};

use crate::capture::CAPTURE_SAMPLE_RATE;

/// Exact frame size `webrtc-vad` requires per call — 30ms at 16kHz (its
/// largest allowed frame, so the fewest `is_voice_segment` calls/sec).
const FRAME_MS: u32 = 30;
const FRAME_SAMPLES: usize = (CAPTURE_SAMPLE_RATE * FRAME_MS / 1000) as usize;

/// How many consecutive voiced frames confirm "someone started talking" —
/// ~90ms, long enough to reject a single stray click/pop without adding
/// noticeable lag to onset detection.
const ONSET_DEBOUNCE_FRAMES: u32 = 3;

/// Minimum *total* voiced content an utterance needs (not counting the
/// trailing silence that ends it) to be worth sending to STT at all — 10
/// frames = 300ms. Onset debounce alone only guarantees ~90ms of voiced
/// audio, and every utterance's buffer already includes a full
/// `silence_timeout_frames` worth of trailing silence regardless of how
/// brief the actual speech was, so a plain minimum-*duration* check can't
/// tell a room-noise blip from real speech — this counts only the voiced
/// frames themselves. Below this, discard rather than transcribe: Whisper
/// is well known to hallucinate plausible-sounding text (often
/// "subscribe"-style YouTube phrases) when fed a clip that's mostly
/// silence, so filtering blips out here means they never reach STT at all
/// instead of coming back as confident-sounding nonsense (a live-testing
/// checkpoint hit exactly this on a noisier microphone — see
/// docs/DEVELOPMENT_STATUS.md's Fase 6 section).
const MIN_VOICED_FRAMES: u32 = 10;

pub enum SegmenterEvent {
    /// Onset just confirmed — fired once per utterance, before it's
    /// actually complete, purely so the orchestrator can publish
    /// `CallState::Listening` right away instead of waiting for the whole
    /// utterance to finish for any UI feedback at all. Not sustained
    /// enough on its own to prove this is real speech (see
    /// `SpeechConfirmed`) — a noise blip that trips onset debounce but
    /// never reaches `MIN_VOICED_FRAMES` fires this and nothing else.
    OnsetDetected,
    /// Fired once per utterance, the moment total voiced content crosses
    /// `MIN_VOICED_FRAMES` (~300ms) *while still buffering* — confirms
    /// "this is real sustained speech, not a noise blip" well before the
    /// utterance actually ends (which waits for the silence timeout on
    /// top of that). Used for barge-in: interrupting whatever the
    /// character is currently saying on unconfirmed onset alone would cut
    /// it off over coughs/clicks/room noise; waiting for the full
    /// utterance to complete would feel sluggish. This is the middle
    /// ground — see `CallService::Inner::run_orchestrator`.
    SpeechConfirmed,
    /// A complete utterance, ready for STT — mono 16kHz i16 PCM.
    UtteranceComplete(Vec<i16>),
}

enum State {
    Idle,
    /// Counting consecutive voiced frames since the first one, before
    /// committing to "someone is talking" — buffers them so they're not
    /// lost once onset confirms.
    OnsetDebounce { buffer: Vec<i16>, consecutive_voiced: u32 },
    /// Confirmed speech — buffering until enough consecutive silent frames
    /// end the utterance. `consecutive_silent` resets on any voiced frame;
    /// `voiced_frames` never resets — it's the running total checked
    /// against `MIN_VOICED_FRAMES` at completion.
    Buffering { buffer: Vec<i16>, consecutive_silent: u32, voiced_frames: u32 },
}

/// Turns a continuous stream of mono 16kHz i16 PCM into discrete
/// utterances via `webrtc-vad`. Lives entirely on one dedicated thread
/// (`CallService`'s orchestrator thread — VAD's C library state isn't
/// meant to be shared or moved across threads either), fed by
/// `std::sync::mpsc` from `MicCapture`'s callback.
pub struct Segmenter {
    vad: Vad,
    silence_timeout_frames: u32,
    /// Samples not yet long enough to fill one `FRAME_SAMPLES` frame —
    /// carried across `push` calls since `MicCapture`'s chunks rarely line
    /// up exactly with the VAD's fixed frame size.
    frame_carry: Vec<i16>,
    state: State,
}

impl Segmenter {
    pub fn new(mode: SettingsVadMode, silence_timeout_ms: u32) -> Self {
        let vad = Vad::new_with_rate_and_mode(VadSampleRate::Rate16kHz, to_webrtc_mode(mode));
        let silence_timeout_frames = (silence_timeout_ms / FRAME_MS).max(1);
        Self { vad, silence_timeout_frames, frame_carry: Vec::new(), state: State::Idle }
    }

    /// Feeds newly captured samples in, returning any events produced —
    /// usually empty (most calls just update internal state), occasionally
    /// one `UtteranceComplete`.
    pub fn push(&mut self, samples: &[i16]) -> Vec<SegmenterEvent> {
        self.frame_carry.extend_from_slice(samples);
        let mut events = Vec::new();

        while self.frame_carry.len() >= FRAME_SAMPLES {
            let frame: Vec<i16> = self.frame_carry.drain(..FRAME_SAMPLES).collect();
            let voiced = self.vad.is_voice_segment(&frame).unwrap_or(false);
            if let Some(event) = transition(&mut self.state, self.silence_timeout_frames, frame, voiced) {
                events.push(event);
            }
        }

        events
    }
}

/// The actual state machine, factored out from `Vad` classification so it
/// can be unit-tested with hand-picked `voiced` values instead of needing
/// real audio and a real `libfvad` instance.
///
/// Takes `*state` by value (via `mem::replace` with a placeholder) and
/// writes the computed next state back at the end, rather than mutating
/// through the match's destructured bindings in place — keeps every branch
/// a plain, unambiguous value computation instead of juggling nested
/// mutable borrows of `state` alongside its own fields.
fn transition(state: &mut State, silence_timeout_frames: u32, frame: Vec<i16>, voiced: bool) -> Option<SegmenterEvent> {
    let current = std::mem::replace(state, State::Idle);
    let (next, event) = match current {
        State::Idle => {
            if voiced {
                (State::OnsetDebounce { buffer: frame, consecutive_voiced: 1 }, None)
            } else {
                (State::Idle, None)
            }
        }
        State::OnsetDebounce { mut buffer, consecutive_voiced } => {
            if voiced {
                buffer.extend_from_slice(&frame);
                let consecutive_voiced = consecutive_voiced + 1;
                if consecutive_voiced >= ONSET_DEBOUNCE_FRAMES {
                    // All `consecutive_voiced` onset frames were voiced —
                    // they count toward the total from the start.
                    (State::Buffering { buffer, consecutive_silent: 0, voiced_frames: consecutive_voiced }, Some(SegmenterEvent::OnsetDetected))
                } else {
                    (State::OnsetDebounce { buffer, consecutive_voiced }, None)
                }
            } else {
                // Wasn't sustained enough to count as real speech.
                (State::Idle, None)
            }
        }
        State::Buffering { mut buffer, consecutive_silent, voiced_frames } => {
            buffer.extend_from_slice(&frame);
            if voiced {
                let next_voiced_frames = voiced_frames + 1;
                // Fires exactly once per utterance, the frame this
                // crosses the threshold — not on every voiced frame
                // after.
                let event =
                    if voiced_frames < MIN_VOICED_FRAMES && next_voiced_frames >= MIN_VOICED_FRAMES {
                        Some(SegmenterEvent::SpeechConfirmed)
                    } else {
                        None
                    };
                (State::Buffering { buffer, consecutive_silent: 0, voiced_frames: next_voiced_frames }, event)
            } else {
                let consecutive_silent = consecutive_silent + 1;
                if consecutive_silent >= silence_timeout_frames {
                    if voiced_frames >= MIN_VOICED_FRAMES {
                        (State::Idle, Some(SegmenterEvent::UtteranceComplete(buffer)))
                    } else {
                        tracing::info!(
                            target: "system",
                            "call: descartado un posible ruido ({}ms de voz detectada, mínimo {}ms) — no se envió a transcribir",
                            voiced_frames * FRAME_MS,
                            MIN_VOICED_FRAMES * FRAME_MS,
                        );
                        (State::Idle, None)
                    }
                } else {
                    (State::Buffering { buffer, consecutive_silent, voiced_frames }, None)
                }
            }
        }
    };
    *state = next;
    event
}

fn to_webrtc_mode(mode: SettingsVadMode) -> WebRtcVadMode {
    match mode {
        SettingsVadMode::Quality => WebRtcVadMode::Quality,
        SettingsVadMode::LowBitrate => WebRtcVadMode::LowBitrate,
        SettingsVadMode::Aggressive => WebRtcVadMode::Aggressive,
        SettingsVadMode::VeryAggressive => WebRtcVadMode::VeryAggressive,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(fill: i16) -> Vec<i16> {
        vec![fill; FRAME_SAMPLES]
    }

    /// Drives `transition` directly with scripted voiced/silent frames —
    /// same tiny frames `Segmenter::push` would produce, without needing a
    /// real `Vad`.
    fn run(script: &[bool], silence_timeout_frames: u32) -> Vec<SegmenterEvent> {
        let mut state = State::Idle;
        let mut events = Vec::new();
        for &voiced in script {
            if let Some(event) = transition(&mut state, silence_timeout_frames, frame(1), voiced) {
                events.push(event);
            }
        }
        events
    }

    #[test]
    fn silence_alone_never_produces_an_utterance() {
        let events = run(&[false; 20], 5);
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn a_brief_blip_shorter_than_onset_debounce_is_ignored() {
        // Two voiced frames, then back to silence — never reaches
        // ONSET_DEBOUNCE_FRAMES (3), so this never becomes an utterance.
        let events = run(&[true, true, false, false, false, false, false], 3);
        assert_eq!(events.len(), 0);
    }

    /// `n` consecutive voiced frames — tests that need to clear
    /// `MIN_VOICED_FRAMES` use this instead of spelling out `true` by hand.
    fn speech(n: usize) -> Vec<bool> {
        vec![true; n]
    }

    fn silence(n: usize) -> Vec<bool> {
        vec![false; n]
    }

    #[test]
    fn sustained_speech_then_silence_produces_onset_confirmed_then_one_utterance() {
        // Enough voiced frames to clear MIN_VOICED_FRAMES (also crossing
        // the SpeechConfirmed threshold right at the last one), then 3
        // silent frames (== timeout) end it (→ UtteranceComplete).
        let script: Vec<bool> = speech(MIN_VOICED_FRAMES as usize).into_iter().chain(silence(3)).collect();
        let events = run(&script, 3);
        assert_eq!(events.len(), 3);
        assert!(matches!(events[0], SegmenterEvent::OnsetDetected));
        assert!(matches!(events[1], SegmenterEvent::SpeechConfirmed));
        let SegmenterEvent::UtteranceComplete(samples) = &events[2] else { panic!("expected UtteranceComplete") };
        let expected_frames = MIN_VOICED_FRAMES as usize + 3;
        assert_eq!(samples.len(), expected_frames * FRAME_SAMPLES);
    }

    #[test]
    fn a_short_pause_mid_sentence_does_not_end_the_utterance_early() {
        // Onset, one silent frame (below the timeout of 3), then more
        // speech (clearing MIN_VOICED_FRAMES total, so SpeechConfirmed
        // fires too), then a real silence-timeout ending — one onset, one
        // confirmation, one completion.
        let script: Vec<bool> =
            speech(3).into_iter().chain(silence(1)).chain(speech(MIN_VOICED_FRAMES as usize)).chain(silence(3)).collect();
        let events = run(&script, 3);
        assert_eq!(events.len(), 3);
        assert!(matches!(events[0], SegmenterEvent::OnsetDetected));
        assert!(matches!(events[1], SegmenterEvent::SpeechConfirmed));
        assert!(matches!(events[2], SegmenterEvent::UtteranceComplete(_)));
    }

    #[test]
    fn back_to_back_utterances_are_each_reported_separately() {
        let one_utterance: Vec<bool> = speech(MIN_VOICED_FRAMES as usize).into_iter().chain(silence(3)).collect();
        let script: Vec<bool> = one_utterance.iter().chain(one_utterance.iter()).copied().collect();
        let events = run(&script, 3);
        let onsets = events.iter().filter(|e| matches!(e, SegmenterEvent::OnsetDetected)).count();
        let completions = events.iter().filter(|e| matches!(e, SegmenterEvent::UtteranceComplete(_))).count();
        assert_eq!(onsets, 2);
        assert_eq!(completions, 2);
    }

    #[test]
    fn a_brief_noise_blip_reaches_onset_but_never_gets_transcribed() {
        // Exactly ONSET_DEBOUNCE_FRAMES of voiced audio (the minimum that
        // still confirms onset) followed by silence — real onset fires
        // (the UI shows "Listening"), but with too little actual voiced
        // content to be real speech, no UtteranceComplete ever follows.
        // This is the "¡Suscríbete!" hallucination bug: a noisy mic's room
        // tone crossing the VAD threshold for just long enough to trigger
        // onset, then Whisper hallucinating text for an almost-silent clip.
        let script: Vec<bool> = speech(ONSET_DEBOUNCE_FRAMES as usize).into_iter().chain(silence(3)).collect();
        let events = run(&script, 3);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], SegmenterEvent::OnsetDetected));
    }

    #[test]
    fn speech_confirmed_fires_exactly_once_per_utterance_right_at_the_threshold() {
        // A few extra voiced frames past MIN_VOICED_FRAMES before silence
        // — confirms the event fires once, at the crossing frame, not
        // again for every voiced frame after.
        let script: Vec<bool> = speech(MIN_VOICED_FRAMES as usize + 4).into_iter().chain(silence(3)).collect();
        let events = run(&script, 3);
        let confirmations = events.iter().filter(|e| matches!(e, SegmenterEvent::SpeechConfirmed)).count();
        assert_eq!(confirmations, 1);
    }

    #[test]
    fn speech_confirmed_never_fires_for_a_noise_blip_that_never_reaches_the_threshold() {
        // Same script as the hallucination-guard test above — onset
        // fires, but SpeechConfirmed must not: this is exactly what keeps
        // barge-in from cutting the character off over a cough or click
        // (see `CallService::Inner::run_orchestrator`).
        let script: Vec<bool> = speech(ONSET_DEBOUNCE_FRAMES as usize).into_iter().chain(silence(3)).collect();
        let events = run(&script, 3);
        assert!(!events.iter().any(|e| matches!(e, SegmenterEvent::SpeechConfirmed)));
    }

    #[test]
    fn voiced_frames_accumulate_across_interruptions_by_brief_silence() {
        // Speech, a short pause (doesn't reset the voiced total, only
        // consecutive_silent does), more speech totaling
        // MIN_VOICED_FRAMES, then real silence — still produces an
        // utterance, proving `voiced_frames` truly never resets.
        let script: Vec<bool> = speech(5)
            .into_iter()
            .chain(silence(1))
            .chain(speech(MIN_VOICED_FRAMES as usize - 5))
            .chain(silence(3))
            .collect();
        let events = run(&script, 3);
        assert_eq!(events.iter().filter(|e| matches!(e, SegmenterEvent::UtteranceComplete(_))).count(), 1);
    }

    #[test]
    fn vad_mode_maps_onto_the_underlying_webrtc_vad_mode() {
        // Just proves the mapping compiles/covers every variant — actual
        // VAD behavior differences are a live-tuning concern, not a unit
        // test's job.
        for mode in [SettingsVadMode::Quality, SettingsVadMode::LowBitrate, SettingsVadMode::Aggressive, SettingsVadMode::VeryAggressive]
        {
            let _ = to_webrtc_mode(mode);
        }
    }
}
