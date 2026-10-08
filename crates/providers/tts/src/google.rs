use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::TtsError;
use crate::types::VoiceInfo;

const API_BASE: &str = "https://texttospeech.googleapis.com/v1";
/// Used only when `voice_id` doesn't look like a Google voice name (empty,
/// or missing the `{languageCode}-...` prefix every real one has) — see
/// `language_code_for_voice`.
const DEFAULT_LANGUAGE_CODE: &str = "en-US";

#[derive(Serialize)]
struct SynthesizeRequestBody<'a> {
    input: SynthesisInput<'a>,
    voice: VoiceSelectionParams<'a>,
    #[serde(rename = "audioConfig")]
    audio_config: AudioConfig,
}

#[derive(Serialize)]
struct SynthesisInput<'a> {
    text: &'a str,
}

#[derive(Serialize)]
struct VoiceSelectionParams<'a> {
    #[serde(rename = "languageCode")]
    language_code: &'a str,
    name: &'a str,
}

#[derive(Serialize)]
struct AudioConfig {
    #[serde(rename = "audioEncoding")]
    audio_encoding: &'static str,
    #[serde(rename = "speakingRate")]
    speaking_rate: f32,
}

#[derive(Deserialize)]
struct SynthesizeResponseBody {
    #[serde(rename = "audioContent")]
    audio_content: String,
}

#[derive(Deserialize)]
struct VoicesResponseBody {
    voices: Vec<GoogleVoice>,
}

#[derive(Deserialize)]
struct GoogleVoice {
    name: String,
    #[serde(rename = "languageCodes")]
    language_codes: Vec<String>,
}

/// Google's voice names are always `{languageCode}-{Type}-{Letter}` (e.g.
/// `"es-ES-Standard-A"`, `"en-US-Wavenet-D"`) — the language code is just
/// its first two dash-separated segments. Falls back to
/// `DEFAULT_LANGUAGE_CODE` for an empty or unrecognized `voice_id` rather
/// than rejecting the request outright, same spirit as `HttpEngine`
/// defaulting an empty `voice_id` to `"alloy"`.
fn language_code_for_voice(voice_id: &str) -> String {
    let mut parts = voice_id.splitn(3, '-');
    match (parts.next(), parts.next()) {
        (Some(a), Some(b)) if !a.is_empty() && !b.is_empty() => format!("{a}-{b}"),
        _ => DEFAULT_LANGUAGE_CODE.to_string(),
    }
}

/// Talks to Google Cloud Text-to-Speech directly and returns the raw audio
/// it fetches. Playback (and, for Chat, optionally saving the bytes to
/// disk) is `AivtTtsProvider`'s job, not this engine's — see
/// `TTSProvider::speak_and_capture`.
pub struct GoogleEngine {
    client: Client,
}

impl GoogleEngine {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Only `rate` maps onto this contract (as `speakingRate`) —
    /// `pitch`/`volume` have no equivalent here and are silently ignored,
    /// same as `HttpEngine::synthesize`. Google documents `speakingRate` as
    /// valid in `[0.25, 4.0]`.
    pub async fn synthesize(&self, text: &str, voice_id: &str, rate: f32, api_key: &str) -> Result<Vec<u8>, TtsError> {
        if api_key.is_empty() {
            return Err(TtsError::Unavailable("falta la API key de Google Cloud — configúrala en Configuración > Motor de voz".to_string()));
        }
        if voice_id.is_empty() {
            return Err(TtsError::Unavailable("selecciona una voz de Google Cloud primero".to_string()));
        }

        let language_code = language_code_for_voice(voice_id);
        let body = SynthesizeRequestBody {
            input: SynthesisInput { text },
            voice: VoiceSelectionParams { language_code: &language_code, name: voice_id },
            audio_config: AudioConfig { audio_encoding: "MP3", speaking_rate: rate.clamp(0.25, 4.0) },
        };

        let url = format!("{API_BASE}/text:synthesize?key={api_key}");
        let response = self.client.post(url).json(&body).send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let body_text = response.text().await.unwrap_or_default();
            return Err(TtsError::Server(format!("{status}: {body_text}")));
        }
        let parsed: SynthesizeResponseBody = response.json().await.map_err(|err| TtsError::Server(err.to_string()))?;
        BASE64.decode(parsed.audio_content).map_err(|err| TtsError::Server(format!("audio en base64 inválido: {err}")))
    }

    /// The `languageCodes` field can list several — the first is a
    /// reasonable single value for `VoiceInfo::language`, which only ever
    /// shows one alongside the voice name in the Voz tab's picker.
    pub async fn list_voices(&self, api_key: &str) -> Result<Vec<VoiceInfo>, TtsError> {
        if api_key.is_empty() {
            return Err(TtsError::Unavailable("falta la API key de Google Cloud — configúrala en Configuración > Motor de voz".to_string()));
        }

        let url = format!("{API_BASE}/voices?key={api_key}");
        let response = self.client.get(url).send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let body_text = response.text().await.unwrap_or_default();
            return Err(TtsError::Server(format!("{status}: {body_text}")));
        }
        let body: VoicesResponseBody = response.json().await.map_err(|err| TtsError::Server(err.to_string()))?;
        Ok(body
            .voices
            .into_iter()
            .map(|v| VoiceInfo { id: v.name.clone(), name: v.name, language: v.language_codes.into_iter().next() })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthesize_request_body_serializes_with_the_google_shape() {
        let body = SynthesizeRequestBody {
            input: SynthesisInput { text: "Hola" },
            voice: VoiceSelectionParams { language_code: "es-ES", name: "es-ES-Standard-A" },
            audio_config: AudioConfig { audio_encoding: "MP3", speaking_rate: 1.0 },
        };
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["input"]["text"], "Hola");
        assert_eq!(json["voice"]["languageCode"], "es-ES");
        assert_eq!(json["voice"]["name"], "es-ES-Standard-A");
        assert_eq!(json["audioConfig"]["audioEncoding"], "MP3");
        assert_eq!(json["audioConfig"]["speakingRate"], 1.0);
    }

    #[test]
    fn language_code_is_derived_from_the_voice_name_prefix() {
        assert_eq!(language_code_for_voice("es-ES-Standard-A"), "es-ES");
        assert_eq!(language_code_for_voice("en-US-Wavenet-D"), "en-US");
    }

    #[test]
    fn language_code_falls_back_for_empty_or_unrecognized_voice_ids() {
        assert_eq!(language_code_for_voice(""), DEFAULT_LANGUAGE_CODE);
        assert_eq!(language_code_for_voice("not-a-real-voice-name"), "not-a");
        assert_eq!(language_code_for_voice("solo"), DEFAULT_LANGUAGE_CODE);
    }

    #[test]
    fn synthesize_response_body_decodes_base64_audio() {
        let parsed: SynthesizeResponseBody = serde_json::from_str(r#"{"audioContent": "aGVsbG8="}"#).unwrap();
        let decoded = BASE64.decode(parsed.audio_content).unwrap();
        assert_eq!(decoded, b"hello");
    }

    #[test]
    fn voices_response_body_parses_the_documented_shape() {
        let body: VoicesResponseBody = serde_json::from_str(
            r#"{"voices": [{"languageCodes": ["es-ES"], "name": "es-ES-Standard-A", "ssmlGender": "FEMALE", "naturalSampleRateHertz": 24000}]}"#,
        )
        .unwrap();
        assert_eq!(body.voices.len(), 1);
        assert_eq!(body.voices[0].name, "es-ES-Standard-A");
        assert_eq!(body.voices[0].language_codes, vec!["es-ES".to_string()]);
    }
}
