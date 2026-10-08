use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::TtsError;
use crate::types::VoiceInfo;

const API_BASE: &str = "https://api.elevenlabs.io/v1";
/// A good balance of quality and language coverage, per ElevenLabs' own
/// docs — not user-configurable, same reasoning as `HttpEngine` fixing
/// `model: "tts-1"` rather than exposing a picker the Voz tab never had.
const MODEL_ID: &str = "eleven_multilingual_v2";

#[derive(Serialize)]
struct SpeechRequestBody<'a> {
    text: &'a str,
    model_id: &'a str,
    voice_settings: VoiceSettings,
}

/// ElevenLabs' own knobs — `stability`/`similarity_boost` are left at their
/// documented defaults (Fase 5 never exposed sliders for these, only
/// rate/pitch/volume, which don't map onto this API 1:1 — see `speak`'s
/// doc comment on `speed`).
#[derive(Serialize)]
struct VoiceSettings {
    stability: f32,
    similarity_boost: f32,
    speed: f32,
}

#[derive(Deserialize)]
struct VoicesResponseBody {
    voices: Vec<ElevenLabsVoice>,
}

#[derive(Deserialize)]
struct ElevenLabsVoice {
    voice_id: String,
    name: String,
}

/// ElevenLabs rejects "Voice Library" (shared/community) voices on free
/// accounts with 402 + this documented error code — surfaced as raw JSON
/// by default (`{"detail":{"status":"payment_required","code":"paid_plan_required",...}}`),
/// which reads as an app bug to the user. Translate it into a clear,
/// actionable message instead of passing the API's own wording through.
fn friendly_error_for_status(status: reqwest::StatusCode, body_text: &str) -> TtsError {
    if status == reqwest::StatusCode::PAYMENT_REQUIRED && body_text.contains("paid_plan_required") {
        TtsError::Unavailable(
            "esta voz es de la Voice Library de ElevenLabs y tu plan gratuito no permite usarla por API — \
             elige otra voz (una de las voces por defecto o una clonada por ti) o mejora tu plan en \
             elevenlabs.io/pricing"
                .to_string(),
        )
    } else {
        TtsError::Server(format!("{status}: {body_text}"))
    }
}

/// Talks to ElevenLabs' own API directly (not the generic `Http` backend's
/// OpenAI-compatible contract) and returns the raw audio it fetches.
/// Playback (and, for Chat, optionally saving the bytes to disk) is
/// `AivtTtsProvider`'s job, not this engine's — see
/// `TTSProvider::speak_and_capture`.
pub struct ElevenLabsEngine {
    client: Client,
}

impl ElevenLabsEngine {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Only `rate` maps onto this contract (as `speed`) — `pitch`/`volume`
    /// have no equivalent in ElevenLabs' `voice_settings` and are silently
    /// ignored, same as `HttpEngine::synthesize`. Unlike OpenAI's much wider
    /// `speed` range, ElevenLabs only accepts roughly 0.7–1.2 — the app's
    /// own `rate` slider (0.5–2.0×) is clamped into that narrower window
    /// rather than sent as-is, which the API would otherwise reject.
    pub async fn synthesize(&self, text: &str, voice_id: &str, rate: f32, api_key: &str) -> Result<Vec<u8>, TtsError> {
        if api_key.is_empty() {
            return Err(TtsError::Unavailable("falta la API key de ElevenLabs — configúrala en Configuración > Proveedores IA".to_string()));
        }
        if voice_id.is_empty() {
            return Err(TtsError::Unavailable("selecciona una voz de ElevenLabs primero".to_string()));
        }

        let body = SpeechRequestBody {
            text,
            model_id: MODEL_ID,
            voice_settings: VoiceSettings { stability: 0.5, similarity_boost: 0.75, speed: rate.clamp(0.7, 1.2) },
        };

        let url = format!("{API_BASE}/text-to-speech/{voice_id}");
        let response = self.client.post(url).header("xi-api-key", api_key).json(&body).send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let body_text = response.text().await.unwrap_or_default();
            return Err(friendly_error_for_status(status, &body_text));
        }
        Ok(response.bytes().await?.to_vec())
    }

    /// The one backend besides `Native` that can actually enumerate its
    /// own voices — see `VoiceInfo`'s doc comment.
    pub async fn list_voices(&self, api_key: &str) -> Result<Vec<VoiceInfo>, TtsError> {
        if api_key.is_empty() {
            return Err(TtsError::Unavailable("falta la API key de ElevenLabs — configúrala en Configuración > Proveedores IA".to_string()));
        }

        let response = self.client.get(format!("{API_BASE}/voices")).header("xi-api-key", api_key).send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let body_text = response.text().await.unwrap_or_default();
            return Err(TtsError::Server(format!("{status}: {body_text}")));
        }
        let body: VoicesResponseBody = response.json().await.map_err(|err| TtsError::Server(err.to_string()))?;
        Ok(body.voices.into_iter().map(|v| VoiceInfo { id: v.voice_id, name: v.name, language: None }).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speech_request_body_serializes_with_elevenlabs_shape() {
        let body = SpeechRequestBody {
            text: "Hola",
            model_id: MODEL_ID,
            voice_settings: VoiceSettings { stability: 0.5, similarity_boost: 0.75, speed: 1.0 },
        };
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["text"], "Hola");
        assert_eq!(json["model_id"], "eleven_multilingual_v2");
        assert_eq!(json["voice_settings"]["stability"], 0.5);
        assert_eq!(json["voice_settings"]["similarity_boost"], 0.75);
        assert_eq!(json["voice_settings"]["speed"], 1.0);
    }

    #[test]
    fn voices_response_body_parses_the_documented_shape() {
        let body: VoicesResponseBody = serde_json::from_str(
            r#"{"voices": [{"voice_id": "21m00Tcm4TlvDq8ikWAM", "name": "Rachel", "category": "premade"}]}"#,
        )
        .unwrap();
        assert_eq!(body.voices.len(), 1);
        assert_eq!(body.voices[0].voice_id, "21m00Tcm4TlvDq8ikWAM");
        assert_eq!(body.voices[0].name, "Rachel");
    }

    #[test]
    fn payment_required_for_a_voice_library_voice_gets_a_friendly_spanish_message() {
        let body_text = r#"{"detail":{"status":"payment_required","code":"paid_plan_required","message":"..."}}"#;
        let err = friendly_error_for_status(reqwest::StatusCode::PAYMENT_REQUIRED, body_text);
        match err {
            TtsError::Unavailable(msg) => assert!(msg.contains("Voice Library"), "unexpected message: {msg}"),
            other => panic!("expected Unavailable, got {other:?}"),
        }
    }

    #[test]
    fn other_error_statuses_pass_through_as_server_errors() {
        let err = friendly_error_for_status(reqwest::StatusCode::UNAUTHORIZED, "invalid_api_key");
        match err {
            TtsError::Server(msg) => assert!(msg.contains("invalid_api_key")),
            other => panic!("expected Server, got {other:?}"),
        }
    }
}
