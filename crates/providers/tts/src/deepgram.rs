use reqwest::Client;
use serde::Serialize;

use crate::error::TtsError;

const API_URL: &str = "https://api.deepgram.com/v1/speak";
/// En Deepgram la "voz" es el propio modelo (`aura-2-thalia-en`, ...): se usa
/// este cuando el usuario no escribió ninguno.
const DEFAULT_MODEL: &str = "aura-2-thalia-en";

#[derive(Serialize)]
struct SpeakRequestBody<'a> {
    text: &'a str,
}

/// Deepgram Aura (`POST /v1/speak?model=...`, `Authorization: Token`) —
/// devuelve el audio directo. No hay endpoint de voces ni parámetro de
/// velocidad, así que `rate`/`pitch`/`volume` se ignoran y `voice_id` es el
/// nombre del modelo, en texto libre. Aura limita el texto a 2000
/// caracteres por petición (el servidor responde 413 si se pasa).
pub struct DeepgramEngine {
    client: Client,
}

impl DeepgramEngine {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub async fn synthesize(&self, text: &str, voice_id: &str, api_key: &str) -> Result<Vec<u8>, TtsError> {
        if api_key.is_empty() {
            return Err(TtsError::Unavailable("falta la API key de Deepgram — configúrala en Configuración > Motor de voz".to_string()));
        }
        let model = if voice_id.trim().is_empty() { DEFAULT_MODEL } else { voice_id.trim() };
        let response = self
            .client
            .post(API_URL)
            .query(&[("model", model), ("encoding", "mp3")])
            .header("Authorization", format!("Token {api_key}"))
            .json(&SpeakRequestBody { text })
            .send()
            .await?;
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
    fn request_body_is_just_the_text() {
        let json = serde_json::to_value(SpeakRequestBody { text: "Hola" }).unwrap();
        assert_eq!(json, serde_json::json!({ "text": "Hola" }));
    }
}
