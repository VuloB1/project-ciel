use reqwest::Client;
use serde::Serialize;

use crate::chunking;
use crate::error::TtsError;

const DEFAULT_MODEL: &str = "tts-1";
const DEFAULT_VOICE: &str = "alloy";

/// The OpenAI `/audio/speech` request shape — also what most self-hosted
/// "OpenAI-compatible" TTS servers (Piper/XTTS wrappers, etc.) accept,
/// mirroring how `OpenAiCompatibleProvider` treats LM Studio as just this
/// shape pointed locally.
#[derive(Serialize)]
struct SpeechRequestBody<'a> {
    model: &'a str,
    input: &'a str,
    voice: &'a str,
    response_format: &'a str,
    /// Omitted for services that don't document it (sending an unknown field
    /// to a strict server can be rejected).
    #[serde(skip_serializing_if = "Option::is_none")]
    speed: Option<f32>,
}

/// What differs between services that speak the `/audio/speech` shape but
/// each with their own model names, voices and limits.
pub struct SpeechPreset {
    pub base_url: &'static str,
    pub default_model: &'static str,
    pub default_voice: &'static str,
    pub response_format: &'static str,
    /// Whether the service documents a `speed` field.
    pub send_speed: bool,
    /// Maximum characters per request; longer text is split and the clips
    /// joined (only supported for WAV, see `chunking::concat_wavs`).
    pub max_chars: Option<usize>,
}

/// Groq Orpheus: WAV only, 200 characters per request, no documented speed.
pub const GROQ: SpeechPreset = SpeechPreset {
    base_url: "https://api.groq.com/openai/v1",
    default_model: "canopylabs/orpheus-v1-english",
    default_voice: "troy",
    response_format: "wav",
    send_speed: false,
    max_chars: Some(200),
};

/// Together AI: mp3 is supported; the default is the model/voice from their
/// own quickstart example.
pub const TOGETHER: SpeechPreset = SpeechPreset {
    base_url: "https://api.together.ai/v1",
    default_model: "canopylabs/orpheus-3b-0.1-ft",
    default_voice: "tara",
    response_format: "mp3",
    send_speed: false,
    max_chars: None,
};

/// Talks to an HTTP TTS backend and returns the raw audio it fetches.
/// No standard "list voices" endpoint exists for this contract, so
/// `voice_id` is free text the user types in the Voz tab rather than a
/// dropdown (see `VoiceInfo`'s doc comment). Playback (and, for Chat,
/// optionally saving the bytes to disk) is `AivtTtsProvider`'s job, not
/// this engine's — see `TTSProvider::speak_and_capture`.
pub struct HttpEngine {
    client: Client,
}

impl HttpEngine {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Only `rate` maps onto this contract (as `speed`) — `pitch`/`volume`
    /// have no equivalent in the OpenAI TTS API and are silently ignored,
    /// same as an `LLMProvider` ignoring a capability the loaded model
    /// doesn't have.
    pub async fn synthesize(&self, text: &str, voice_id: &str, rate: f32, base_url: &str, api_key: Option<&str>) -> Result<Vec<u8>, TtsError> {
        if base_url.is_empty() {
            return Err(TtsError::Unavailable("no hay una URL configurada para el servidor de voz".to_string()));
        }
        let voice = if voice_id.is_empty() { DEFAULT_VOICE } else { voice_id };
        self.request_speech(base_url, api_key, DEFAULT_MODEL, text, voice, "mp3", Some(rate.clamp(0.25, 4.0))).await
    }

    /// Same contract for a named service (`GROQ`/`TOGETHER`): its own base
    /// URL, default model and voice, audio format and per-request text cap.
    pub async fn synthesize_preset(
        &self,
        preset: &SpeechPreset,
        text: &str,
        voice_id: &str,
        rate: f32,
        api_key: &str,
        model: Option<&str>,
    ) -> Result<Vec<u8>, TtsError> {
        if api_key.is_empty() {
            return Err(TtsError::Unavailable("falta la API key — configúrala en Configuración > Motor de voz".to_string()));
        }
        let model = model.map(str::trim).filter(|m| !m.is_empty()).unwrap_or(preset.default_model);
        let voice = if voice_id.trim().is_empty() { preset.default_voice } else { voice_id.trim() };
        let speed = preset.send_speed.then(|| rate.clamp(0.25, 4.0));

        let pieces = match preset.max_chars {
            Some(max) => chunking::split_text(text, max),
            None => vec![text.to_string()],
        };
        if pieces.is_empty() {
            return Err(TtsError::Unavailable("no hay texto que sintetizar".to_string()));
        }

        let mut clips = Vec::with_capacity(pieces.len());
        for piece in &pieces {
            clips.push(self.request_speech(preset.base_url, Some(api_key), model, piece, voice, preset.response_format, speed).await?);
        }
        if clips.len() == 1 {
            return Ok(clips.remove(0));
        }
        match preset.response_format {
            "wav" => chunking::concat_wavs(&clips).map_err(TtsError::Server),
            // Concatenated MP3 frames play back as one stream.
            _ => Ok(clips.concat()),
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn request_speech(
        &self,
        base_url: &str,
        api_key: Option<&str>,
        model: &str,
        text: &str,
        voice: &str,
        response_format: &str,
        speed: Option<f32>,
    ) -> Result<Vec<u8>, TtsError> {
        let body = SpeechRequestBody { model, input: text, voice, response_format, speed };
        let url = format!("{}/audio/speech", base_url.trim_end_matches('/'));
        let mut request = self.client.post(url).json(&body);
        if let Some(key) = api_key {
            if !key.is_empty() {
                request = request.bearer_auth(key);
            }
        }

        let response = request.send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let body_text = response.text().await.unwrap_or_default();
            return Err(TtsError::Server(format!("{status}: {body_text}")));
        }
        Ok(response.bytes().await?.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speech_request_body_serializes_with_the_openai_compatible_shape() {
        let body = SpeechRequestBody { model: "tts-1", input: "Hola", voice: "alloy", response_format: "mp3", speed: Some(1.0) };
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["model"], "tts-1");
        assert_eq!(json["input"], "Hola");
        assert_eq!(json["voice"], "alloy");
        assert_eq!(json["response_format"], "mp3");
        assert_eq!(json["speed"], 1.0);
    }

    #[test]
    fn speed_is_omitted_when_the_service_does_not_document_it() {
        let body = SpeechRequestBody { model: "m", input: "Hola", voice: "troy", response_format: "wav", speed: None };
        assert!(serde_json::to_value(&body).unwrap().get("speed").is_none());
    }

    #[test]
    fn groq_is_wav_with_a_200_character_cap_and_together_is_mp3() {
        assert_eq!(GROQ.response_format, "wav");
        assert_eq!(GROQ.max_chars, Some(200));
        assert_eq!(TOGETHER.response_format, "mp3");
        assert_eq!(TOGETHER.max_chars, None);
    }
}
