use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::TtsError;
use crate::types::VoiceInfo;

const API_URL: &str = "https://api.fish.audio/v1/tts";
const MODELS_URL: &str = "https://api.fish.audio/model";
/// Cuántas voces populares se traen además de las propias del usuario — el
/// catálogo público es enorme; esto es un punto de partida, no el catálogo
/// completo (cualquier otro id se puede pegar a mano en la pestaña Voz).
const POPULAR_VOICES: u32 = 50;
/// Modelo gratuito para desarrollo y pruebas (sin crédito de API, con política
/// de uso justo, disponible hasta el 30-nov-2026 según el blog de Fish Audio).
/// Se usa cuando el usuario no eligió otro modelo.
pub const DEFAULT_MODEL: &str = "s2.1-pro-free";

#[derive(Serialize)]
struct TtsRequestBody<'a> {
    text: &'a str,
    /// Id del modelo de voz de Fish Audio (el que aparece en la URL de la
    /// voz en fish.audio). Se omite si el usuario no eligió ninguno — la API
    /// usa entonces su voz por defecto.
    #[serde(skip_serializing_if = "Option::is_none")]
    reference_id: Option<&'a str>,
    format: &'static str,
    prosody: Prosody,
}

#[derive(Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    items: Vec<ModelItem>,
}

#[derive(Deserialize)]
struct ModelItem {
    #[serde(rename = "_id")]
    id: String,
    title: String,
    #[serde(default)]
    languages: Vec<String>,
}

#[derive(Serialize)]
struct Prosody {
    speed: f32,
}

/// Fish Audio (`POST /v1/tts`, `Authorization: Bearer`) — devuelve el audio
/// MP3 directo, sin JSON ni base64. No documenta un endpoint para listar
/// voces propias, así que, igual que `HttpEngine`, `voice_id` es texto libre
/// en la pestaña Voz (por eso `AivtTtsProvider::list_voices` devuelve vacío).
pub struct FishAudioEngine {
    client: Client,
}

impl FishAudioEngine {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Solo `rate` tiene equivalente (`prosody.speed`, documentado en
    /// `[0.5, 2.0]`); `pitch`/`volume` se ignoran, como en los demás motores.
    pub async fn synthesize(&self, text: &str, voice_id: &str, rate: f32, api_key: &str, model: Option<&str>) -> Result<Vec<u8>, TtsError> {
        if api_key.is_empty() {
            return Err(TtsError::Unavailable("falta la API key de Fish Audio — configúrala en Configuración > Motor de voz".to_string()));
        }
        let body = TtsRequestBody {
            text,
            reference_id: (!voice_id.trim().is_empty()).then_some(voice_id.trim()),
            format: "mp3",
            prosody: Prosody { speed: rate.clamp(0.5, 2.0) },
        };
        let model = model.map(str::trim).filter(|m| !m.is_empty()).unwrap_or(DEFAULT_MODEL);
        let response = self.client.post(API_URL).bearer_auth(api_key).header("model", model).json(&body).send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let body_text = response.text().await.unwrap_or_default();
            return Err(TtsError::Server(format!("{status}: {body_text}")));
        }
        Ok(response.bytes().await?.to_vec())
    }
}

impl FishAudioEngine {
    /// Las voces propias del usuario (`self=true`) primero, y después las más
    /// populares del catálogo público, sin repetir. Si falla la consulta de
    /// las propias (p. ej. cuenta sin voces) se sigue con las populares; solo
    /// falla todo si fallan las dos.
    pub async fn list_voices(&self, api_key: &str) -> Result<Vec<VoiceInfo>, TtsError> {
        if api_key.is_empty() {
            return Err(TtsError::Unavailable("falta la API key de Fish Audio — configúrala en Configuración > Motor de voz".to_string()));
        }
        // Both queries at once: Fish is slow, so waiting for one after the other doubled the delay.
        let own_query = [("self", "true".to_string()), ("page_size", "100".to_string())];
        let popular_query = [("sort_by", "score".to_string()), ("page_size", POPULAR_VOICES.to_string())];
        let (own, popular) = tokio::join!(self.fetch_models(api_key, &own_query), self.fetch_models(api_key, &popular_query));
        if let (Err(err), Err(_)) = (&own, &popular) {
            return Err(TtsError::Server(err.to_string()));
        }
        let mut seen = std::collections::HashSet::new();
        Ok(own
            .unwrap_or_default()
            .into_iter()
            .chain(popular.unwrap_or_default())
            .filter(|item| seen.insert(item.id.clone()))
            .map(|item| VoiceInfo { id: item.id, name: item.title, language: item.languages.into_iter().next() })
            .collect())
    }

    /// Búsqueda por nombre en el catálogo público (`title`), ordenada por
    /// puntaje. Con la búsqueda vacía devuelve las más populares.
    pub async fn search_voices(&self, api_key: &str, query: &str) -> Result<Vec<VoiceInfo>, TtsError> {
        if api_key.is_empty() {
            return Err(TtsError::Unavailable("falta la API key de Fish Audio — configúrala en Configuración > Motor de voz".to_string()));
        }
        let mut params = vec![("sort_by", "score".to_string()), ("page_size", "30".to_string())];
        if !query.trim().is_empty() {
            params.push(("title", query.trim().to_string()));
        }
        let items = self.fetch_models(api_key, &params).await?;
        Ok(items.into_iter().map(|item| VoiceInfo { id: item.id, name: item.title, language: item.languages.into_iter().next() }).collect())
    }

    async fn fetch_models(&self, api_key: &str, query: &[(&str, String)]) -> Result<Vec<ModelItem>, TtsError> {
        let response = self.client.get(MODELS_URL).bearer_auth(api_key).query(query).timeout(std::time::Duration::from_secs(15)).send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let body_text = response.text().await.unwrap_or_default();
            return Err(TtsError::Server(format!("{status}: {body_text}")));
        }
        let parsed: ModelsResponse = response.json().await.map_err(|err| TtsError::Server(err.to_string()))?;
        Ok(parsed.items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_response_parses_id_title_and_languages() {
        let json = r#"{"total":1,"items":[{"_id":"abc","title":"Voz de prueba","languages":["es","en"],"state":"trained"}]}"#;
        let parsed: ModelsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.items[0].id, "abc");
        assert_eq!(parsed.items[0].title, "Voz de prueba");
        assert_eq!(parsed.items[0].languages, vec!["es", "en"]);
    }

    #[test]
    fn models_response_tolerates_missing_languages() {
        let parsed: ModelsResponse = serde_json::from_str(r#"{"items":[{"_id":"a","title":"t"}]}"#).unwrap();
        assert!(parsed.items[0].languages.is_empty());
    }

    #[test]
    fn request_body_omits_the_voice_when_none_was_chosen() {
        let body = TtsRequestBody { text: "Hola", reference_id: None, format: "mp3", prosody: Prosody { speed: 1.0 } };
        let json = serde_json::to_value(&body).unwrap();
        assert!(json.get("reference_id").is_none());
        assert_eq!(json["format"], "mp3");
        assert_eq!(json["prosody"]["speed"], 1.0);
    }

    #[test]
    fn request_body_carries_the_voice_id_as_reference_id() {
        let body = TtsRequestBody { text: "Hola", reference_id: Some("abc123"), format: "mp3", prosody: Prosody { speed: 1.5 } };
        assert_eq!(serde_json::to_value(&body).unwrap()["reference_id"], "abc123");
    }
}
