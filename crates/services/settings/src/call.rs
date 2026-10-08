//! Política de la Llamada de voz (detección de voz, silencio, a quién responde).

use serde::{Deserialize, Serialize};

use crate::service::SettingsService;
use crate::SettingsError;

const KEY_CALL_POLICY: &str = "call_policy";

/// How strictly `webrtc-vad` filters background noise out of "is this
/// speech" — named after the underlying library's own terms (see
/// `crates/services/call`) rather than inventing a lossy "sensitivity"
/// translation here; the frontend renders friendly labels for these.
/// `Quality` is the most permissive (picks up quieter speech, more prone to
/// false positives on background noise); `VeryAggressive` is the strictest
/// (fewest false positives, can clip quiet speech). `Aggressive` is a
/// reasonable middle default for a single-speaker, near-mic call setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VadMode {
    Quality,
    LowBitrate,
    Aggressive,
    VeryAggressive,
}

/// Fase 6 — mirrors `TwitchPolicy`'s shape: a couple of genuine per-room/
/// per-mic tuning knobs. Frame duration and onset-debounce length aren't
/// here because `webrtc-vad` dictates exact 10/20/30ms frames — no real
/// trade-off for the user to make there (see `crates/services/call`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallPolicy {
    pub vad_mode: VadMode,
    /// How long a silence has to last before an utterance is considered
    /// finished and gets sent to STT — too short cuts sentences off mid-
    /// pause, too long makes the character feel slow to respond.
    pub silence_timeout_ms: u32,
    /// The exact device name to capture from (see
    /// `aivt_service_call::list_input_devices`) — `None` uses the system's
    /// current default input device. A saved name that no longer matches
    /// any connected device (unplugged since) also falls back to the
    /// default, logged rather than failing the call outright.
    ///
    /// `#[serde(default)]` on this and the two fields below matters: they
    /// were added after `microphone` shipped, and without it a policy
    /// blob saved before they existed fails to deserialize entirely —
    /// `call_policy()` then errors, the frontend swallows that in
    /// `callStore.load()`'s catch, and the whole Llamada tab's policy
    /// card silently renders nothing (a live-testing checkpoint hit
    /// exactly this — see docs/DEVELOPMENT_STATUS.md's Fase 6 section).
    /// Same reasoning as `StoredLlmConnection::vision_enabled`.
    #[serde(default)]
    pub microphone: Option<String>,
    /// `false` requires `wake_phrase` (or, if that's empty, the active
    /// profile's name/nickname — same `contains_word` match Twitch's
    /// `mentioned_only` uses) to appear in an utterance's transcript
    /// before the character replies — lets someone keep a call open while
    /// gaming/streaming/talking to other people without the character
    /// responding to every single thing said. `true` (the default,
    /// preserving this fase's original always-reply behavior) answers
    /// every utterance.
    #[serde(default = "default_respond_to_all")]
    pub respond_to_all: bool,
    /// A custom trigger phrase for `respond_to_all: false` — `None`/empty
    /// falls back to the active profile's name/nickname.
    #[serde(default)]
    pub wake_phrase: Option<String>,
    /// Para quien usa altavoces: mientras el personaje habla (y un instante
    /// después) el micrófono se ignora, así no se escucha a sí mismo ni se
    /// interrumpe sola. Con auriculares conviene dejarlo apagado: sin esto el
    /// usuario puede interrumpirla hablando. No es una cancelación de eco
    /// real, es una guarda por turnos (ver `aivt_service_call::EchoGuard`).
    #[serde(default)]
    pub echo_guard: bool,
}

impl Default for CallPolicy {
    fn default() -> Self {
        Self { vad_mode: VadMode::Aggressive, silence_timeout_ms: 700, microphone: None, respond_to_all: true, wake_phrase: None, echo_guard: false }
    }
}

fn default_respond_to_all() -> bool {
    true
}

impl SettingsService {
    pub async fn call_policy(&self) -> Result<CallPolicy, SettingsError> {
        Ok(self.get_json(KEY_CALL_POLICY).await?.unwrap_or_default())
    }

    pub async fn set_call_policy(&self, policy: CallPolicy) -> Result<CallPolicy, SettingsError> {
        self.set_json(KEY_CALL_POLICY, &policy).await?;
        Ok(policy)
    }
}

#[cfg(test)]
mod tests {
    use aivt_database::AppSettingsRepository;
    

    use super::*;
    use crate::service::test_service;

    #[tokio::test]
    async fn call_policy_defaults_to_aggressive_with_a_silence_timeout() {
        let service = test_service().await;
        let policy = service.call_policy().await.unwrap();
        assert_eq!(policy.vad_mode, VadMode::Aggressive);
        assert_eq!(policy.silence_timeout_ms, 700);
        assert!(policy.microphone.is_none());
        assert!(policy.respond_to_all);
        assert!(policy.wake_phrase.is_none());
        assert!(!policy.echo_guard, "con auriculares se puede interrumpir: por defecto no se ignora el micrófono");
    }

    #[tokio::test]
    async fn call_policy_round_trips() {
        let service = test_service().await;
        let saved = service
            .set_call_policy(CallPolicy {
                vad_mode: VadMode::VeryAggressive,
                silence_timeout_ms: 1000,
                microphone: Some("USB Mic".to_string()),
                respond_to_all: false,
                wake_phrase: Some("oye Ciel".to_string()),
                echo_guard: true,
            })
            .await
            .unwrap();
        assert_eq!(saved.vad_mode, VadMode::VeryAggressive);
        assert_eq!(saved.microphone.as_deref(), Some("USB Mic"));
        assert!(!saved.respond_to_all);
        assert_eq!(saved.wake_phrase.as_deref(), Some("oye Ciel"));
        assert!(saved.echo_guard);
        assert!(service.call_policy().await.unwrap().echo_guard);
        assert_eq!(service.call_policy().await.unwrap().silence_timeout_ms, 1000);
    }

    /// A policy saved before `microphone`/`respondToAll`/`wakePhrase`
    /// existed (only `vadMode`/`silenceTimeoutMs` in the stored JSON) must
    /// still load — this is the exact bug a live-testing checkpoint hit:
    /// without `#[serde(default)]` on the newer fields, deserializing an
    /// old blob failed outright and the Llamada tab's policy card
    /// silently rendered nothing.
    #[tokio::test]
    async fn a_call_policy_saved_before_the_newer_fields_existed_still_loads() {
        let service = test_service().await;
        AppSettingsRepository::new(&service.pool).set(KEY_CALL_POLICY, r#"{"vadMode":"aggressive","silenceTimeoutMs":700}"#).await.unwrap();

        let loaded = service.call_policy().await.unwrap();
        assert_eq!(loaded.vad_mode, VadMode::Aggressive);
        assert_eq!(loaded.silence_timeout_ms, 700);
        assert!(loaded.microphone.is_none());
        assert!(loaded.respond_to_all);
        assert!(loaded.wake_phrase.is_none());
    }
}
