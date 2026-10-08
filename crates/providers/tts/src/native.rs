use std::sync::{Arc, Mutex};

use aivt_eventbus::{AppEvent, EventBus};
use tts::{Tts, UtteranceId};

use crate::error::TtsError;
use crate::types::VoiceInfo;

/// Which chat message (if any) the currently-speaking utterance belongs to,
/// keyed by its `UtteranceId` — consumed by whichever callback
/// (`on_utterance_end`/`on_utterance_stop`) fires next, but only if that
/// callback's id still matches what's stored here. Every `speak` call below
/// passes `interrupt: true`, which stops whatever's currently playing as
/// part of that same call — found live (rapidly switching voices in the
/// "Probar voz" preview got the button permanently stuck on "Detener"):
/// without the id check, the *old* utterance's stop callback could fire
/// after this slot had already been overwritten with the *new* utterance's
/// info, "stealing" it — clearing the slot and publishing `Finished` for an
/// utterance that hadn't even started yet. Then, when that new utterance
/// actually finished, its own callback found the slot already empty and
/// silently did nothing, so the frontend never got the `Finished` it was
/// still waiting for. Comparing ids makes each callback only ever affect
/// the utterance it's actually about, regardless of firing order.
struct CurrentUtterance {
    id: UtteranceId,
    profile_id: String,
    message_id: Option<String>,
}

/// Wraps the OS's native voices (`tts` crate — WinRT on Windows, AVFoundation
/// on macOS, Speech Dispatcher on Linux). `Tts`'s own mutating methods
/// (`speak`/`stop`/`set_rate`/...) take `&mut self`, and the crate's
/// `Send`/`Sync` impls are asserted by its author (`unsafe impl`) rather
/// than derived — one owned instance behind a plain `Mutex`, never cloned,
/// keeps that assertion's actual precondition (no concurrent access to the
/// same handle) trivially true regardless.
pub struct NativeEngine {
    tts: Mutex<Tts>,
    current: Arc<Mutex<Option<CurrentUtterance>>>,
}

impl NativeEngine {
    /// `Err` when there's no speech engine to talk to at all (e.g. Linux
    /// without Speech Dispatcher installed) — callers treat that the same
    /// way `SecretStore` treats an unavailable Linux Secret Service: report
    /// it, don't silently pretend native voices work.
    pub fn new(event_bus: EventBus) -> Result<Self, TtsError> {
        let tts = Tts::default().map_err(|err| TtsError::Unavailable(err.to_string()))?;
        let current: Arc<Mutex<Option<CurrentUtterance>>> = Arc::new(Mutex::new(None));

        // Both callbacks just mean "nothing is speaking anymore" from the
        // frontend's point of view — see `AppEvent::VoicePlaybackFinished`'s
        // doc comment for why naturally-finished and interrupted collapse
        // into the same event.
        let end_bus = event_bus.clone();
        let end_current = current.clone();
        let _ = tts.on_utterance_end(Some(Box::new(move |id| {
            tracing::info!(target: "system", "native TTS: on_utterance_end fired");
            let mut guard = end_current.lock().expect("tts current-utterance mutex poisoned");
            if guard.as_ref().is_some_and(|playing| playing.id == id) {
                let playing = guard.take().expect("checked Some above");
                drop(guard);
                end_bus.publish(AppEvent::VoicePlaybackFinished { profile_id: playing.profile_id, message_id: playing.message_id });
            }
        })));

        let stop_bus = event_bus.clone();
        let stop_current = current.clone();
        let _ = tts.on_utterance_stop(Some(Box::new(move |id| {
            let mut guard = stop_current.lock().expect("tts current-utterance mutex poisoned");
            if guard.as_ref().is_some_and(|playing| playing.id == id) {
                let playing = guard.take().expect("checked Some above");
                drop(guard);
                stop_bus.publish(AppEvent::VoicePlaybackFinished { profile_id: playing.profile_id, message_id: playing.message_id });
            }
        })));

        Ok(Self { tts: Mutex::new(tts), current })
    }

    pub fn list_voices(&self) -> Result<Vec<VoiceInfo>, TtsError> {
        let tts = self.tts.lock().expect("tts mutex poisoned");
        let voices = tts.voices().map_err(|err| TtsError::Native(err.to_string()))?;
        Ok(voices.into_iter().map(|v| VoiceInfo { id: v.id(), name: v.name(), language: Some(v.language().to_string()) }).collect())
    }

    pub fn speak(
        &self,
        text: &str,
        voice_id: &str,
        rate: f32,
        pitch: f32,
        volume: f32,
        profile_id: String,
        message_id: Option<String>,
        event_bus: &EventBus,
    ) -> Result<(), TtsError> {
        let mut tts = self.tts.lock().expect("tts mutex poisoned");

        if !voice_id.is_empty() {
            if let Some(voice) = tts.voices().map_err(|err| TtsError::Native(err.to_string()))?.into_iter().find(|v| v.id() == voice_id) {
                let _ = tts.set_voice(&voice);
            }
        }
        let clamped_rate = rate.clamp(tts.min_rate(), tts.max_rate());
        let clamped_pitch = pitch.clamp(tts.min_pitch(), tts.max_pitch());
        let clamped_volume = volume.clamp(tts.min_volume(), tts.max_volume());
        let _ = tts.set_rate(clamped_rate);
        let _ = tts.set_pitch(clamped_pitch);
        let _ = tts.set_volume(clamped_volume);

        // `interrupt: true` stops whatever's currently speaking as part of
        // this same call, which can fire *that* utterance's stop callback
        // before `speak` returns here. Recording this utterance's id only
        // *after* we have it (never before) means that stop callback still
        // finds the *old* id in `self.current`, not this one's — so it
        // can't mistake this brand-new utterance for the one it just
        // interrupted. See `CurrentUtterance`'s doc comment for the bug
        // this fixes.
        let id = tts.speak(text, true).map_err(|err| TtsError::Native(err.to_string()))?;
        if let Some(id) = id {
            *self.current.lock().expect("tts current-utterance mutex poisoned") =
                Some(CurrentUtterance { id, profile_id: profile_id.clone(), message_id: message_id.clone() });
        }
        tracing::info!(target: "system", profile_id = %profile_id, message_id = ?message_id, "native TTS: publishing VoicePlaybackStarted");
        event_bus.publish(AppEvent::VoicePlaybackStarted { profile_id, message_id });
        Ok(())
    }

    pub fn stop(&self) -> Result<(), TtsError> {
        let mut tts = self.tts.lock().expect("tts mutex poisoned");
        // `Tts::stop` already no-ops when nothing is speaking (see the
        // WinRT backend's `is_speaking` guard) — safe to call unconditionally.
        tts.stop().map_err(|err| TtsError::Native(err.to_string()))?;
        Ok(())
    }
}
