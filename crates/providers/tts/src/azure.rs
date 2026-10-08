use reqwest::Client;
use serde::Deserialize;

use crate::error::TtsError;
use crate::types::VoiceInfo;

/// A widely-supported, reasonably small MP3 format — Azure has no default,
/// this header is required on every request.
const OUTPUT_FORMAT: &str = "audio-24khz-48kbitrate-mono-mp3";
/// Azure's REST TTS endpoint documents this header as required (it's meant
/// to identify the calling application) — not user-configurable, same
/// reasoning as `HttpEngine` fixing `model: "tts-1"`.
const USER_AGENT: &str = "aivt-companion";
/// Used only when `voice_id` doesn't look like an Azure voice name — see
/// `language_for_voice`.
const DEFAULT_LANGUAGE: &str = "en-US";

/// Azure voice names are `{languageCode}-{Region}-{Name}Neural` (e.g.
/// `"es-ES-ElviraNeural"`) — the language is its first two dash-separated
/// segments, same derivation as Google's `language_code_for_voice`.
fn language_for_voice(voice_id: &str) -> String {
    let mut parts = voice_id.splitn(3, '-');
    match (parts.next(), parts.next()) {
        (Some(a), Some(b)) if !a.is_empty() && !b.is_empty() => format!("{a}-{b}"),
        _ => DEFAULT_LANGUAGE.to_string(),
    }
}

fn escape_ssml_text(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

/// Only `rate` maps onto SSML's `<prosody rate="...">` here — `pitch`/
/// `volume` have no equivalent wired up (same as `HttpEngine`/`GoogleEngine`),
/// even though SSML itself could express them. `rate: 1.0` (the app's
/// neutral default) becomes `"+0%"`.
fn build_ssml(text: &str, voice_id: &str, rate: f32, language: &str) -> String {
    let percent = ((rate.clamp(0.5, 2.0) - 1.0) * 100.0).round();
    format!(
        "<speak version='1.0' xmlns='http://www.w3.org/2001/10/synthesis' xml:lang='{language}'>\
<voice name='{voice_id}'><prosody rate='{percent:+.0}%'>{text}</prosody></voice></speak>",
        language = language,
        voice_id = voice_id,
        percent = percent,
        text = escape_ssml_text(text),
    )
}

#[derive(Deserialize)]
struct AzureVoice {
    #[serde(rename = "ShortName")]
    short_name: String,
    #[serde(rename = "Locale")]
    locale: String,
}

/// Talks to Azure Speech (Cognitive Services) directly and returns the raw
/// audio it fetches. Unlike `GoogleEngine`, the response is the MP3 bytes
/// directly — no JSON envelope, no base64. Playback (and, for Chat,
/// optionally saving the bytes to disk) is `AivtTtsProvider`'s job, not
/// this engine's — see `TTSProvider::speak_and_capture`.
pub struct AzureEngine {
    client: Client,
}

impl AzureEngine {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub async fn synthesize(&self, text: &str, voice_id: &str, rate: f32, api_key: &str, region: &str) -> Result<Vec<u8>, TtsError> {
        if api_key.is_empty() {
            return Err(TtsError::Unavailable("falta la API key de Azure Speech — configúrala en Configuración > Motor de voz".to_string()));
        }
        if region.is_empty() {
            return Err(TtsError::Unavailable("falta configurar la región de Azure Speech (ej. \"eastus\")".to_string()));
        }
        if voice_id.is_empty() {
            return Err(TtsError::Unavailable("selecciona una voz de Azure Speech primero".to_string()));
        }

        let ssml = build_ssml(text, voice_id, rate, &language_for_voice(voice_id));
        let url = format!("https://{region}.tts.speech.microsoft.com/cognitiveservices/v1");
        let response = self
            .client
            .post(url)
            .header("Ocp-Apim-Subscription-Key", api_key)
            .header("Content-Type", "application/ssml+xml")
            .header("X-Microsoft-OutputFormat", OUTPUT_FORMAT)
            .header("User-Agent", USER_AGENT)
            .body(ssml)
            .send()
            .await?;
        if !response.status().is_success() {
            let status = response.status();
            let body_text = response.text().await.unwrap_or_default();
            return Err(TtsError::Server(format!("{status}: {body_text}")));
        }
        Ok(response.bytes().await?.to_vec())
    }

    pub async fn list_voices(&self, api_key: &str, region: &str) -> Result<Vec<VoiceInfo>, TtsError> {
        if api_key.is_empty() {
            return Err(TtsError::Unavailable("falta la API key de Azure Speech — configúrala en Configuración > Motor de voz".to_string()));
        }
        if region.is_empty() {
            return Err(TtsError::Unavailable("falta configurar la región de Azure Speech (ej. \"eastus\")".to_string()));
        }

        let url = format!("https://{region}.tts.speech.microsoft.com/cognitiveservices/voices/list");
        let response = self.client.get(url).header("Ocp-Apim-Subscription-Key", api_key).send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let body_text = response.text().await.unwrap_or_default();
            return Err(TtsError::Server(format!("{status}: {body_text}")));
        }
        let voices: Vec<AzureVoice> = response.json().await.map_err(|err| TtsError::Server(err.to_string()))?;
        Ok(voices.into_iter().map(|v| VoiceInfo { id: v.short_name.clone(), name: v.short_name, language: Some(v.locale) }).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_is_derived_from_the_voice_name_prefix() {
        assert_eq!(language_for_voice("es-ES-ElviraNeural"), "es-ES");
        assert_eq!(language_for_voice("en-US-JennyNeural"), "en-US");
    }

    #[test]
    fn language_falls_back_for_empty_or_unrecognized_voice_ids() {
        assert_eq!(language_for_voice(""), DEFAULT_LANGUAGE);
        assert_eq!(language_for_voice("solo"), DEFAULT_LANGUAGE);
    }

    #[test]
    fn ssml_escapes_special_characters_in_the_text() {
        let ssml = build_ssml("<hola> & \"amigos\"", "es-ES-ElviraNeural", 1.0, "es-ES");
        assert!(ssml.contains("&lt;hola&gt; &amp; &quot;amigos&quot;"));
        assert!(!ssml.contains("<hola>"));
    }

    #[test]
    fn ssml_carries_the_voice_name_and_language() {
        let ssml = build_ssml("hola", "es-ES-ElviraNeural", 1.0, "es-ES");
        assert!(ssml.contains("xml:lang='es-ES'"));
        assert!(ssml.contains("name='es-ES-ElviraNeural'"));
    }

    #[test]
    fn rate_maps_to_a_signed_prosody_percentage() {
        assert!(build_ssml("hola", "v", 1.0, "es-ES").contains("rate='+0%'"));
        assert!(build_ssml("hola", "v", 1.5, "es-ES").contains("rate='+50%'"));
        assert!(build_ssml("hola", "v", 0.5, "es-ES").contains("rate='-50%'"));
    }

    #[test]
    fn voices_response_body_parses_the_documented_shape() {
        let voices: Vec<AzureVoice> =
            serde_json::from_str(r#"[{"Name": "Microsoft Server Speech Text to Speech Voice (es-ES, ElviraNeural)", "ShortName": "es-ES-ElviraNeural", "Locale": "es-ES"}]"#)
                .unwrap();
        assert_eq!(voices.len(), 1);
        assert_eq!(voices[0].short_name, "es-ES-ElviraNeural");
        assert_eq!(voices[0].locale, "es-ES");
    }
}
