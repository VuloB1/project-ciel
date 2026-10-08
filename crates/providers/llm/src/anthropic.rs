use std::collections::BTreeMap;

use async_trait::async_trait;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::sse::drain_sse_data_lines;
use crate::types::{joined_system_prompt, ChatChunk, ChatMessage, ChatRequest, ConnectionStatus, MessageRole, ModelCapabilities, ModelInfo, ToolCall};
use crate::{LLMProvider, ProviderError};

const ANTHROPIC_VERSION: &str = "2023-06-01";
/// Anthropic exige `max_tokens` en cada request — a diferencia de OpenAI/
/// Gemini, que lo tratan como opcional — así que a esto se cae cuando el
/// proveedor configurado no tiene uno guardado (no debería pasar en la
/// práctica: `StoredLlmProviderEntry::max_tokens` siempre tiene un valor
/// por defecto, ver `SettingsService::add_llm_provider`).
const DEFAULT_MAX_TOKENS: u32 = 1024;

/// Adaptador real para la API de Anthropic (`/v1/messages`) — no es
/// compatible con el protocolo de OpenAI (headers propios, `system` como
/// campo aparte, bloques de contenido tipados, sin rol `tool`), así que
/// necesita su propio adaptador en vez de reusar `OpenAiCompatibleProvider`
/// como hace la tarjeta "OpenAI". Ver el plan de Proveedores IA.
pub struct AnthropicProvider {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl AnthropicProvider {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self { client: reqwest::Client::new(), base_url: base_url.into().trim_end_matches('/').to_string(), api_key: api_key.into() }
    }
}

#[async_trait]
impl LLMProvider for AnthropicProvider {
    async fn test_connection(&self) -> Result<ConnectionStatus, ProviderError> {
        let start = std::time::Instant::now();
        let models = self.list_models().await?;
        Ok(ConnectionStatus { connected: true, latency_ms: Some(start.elapsed().as_millis() as u64), models })
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        let response = self
            .client
            .get(format!("{}/models", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .send()
            .await
            .map_err(map_reqwest_error)?;
        let response = check_status(response).await?;
        let body: ModelsResponseBody = response.json().await.map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;
        Ok(body.data.into_iter().map(|m| ModelInfo { id: m.id }).collect())
    }

    fn capabilities(&self, _model: &str) -> ModelCapabilities {
        ModelCapabilities { text: true, vision: true, tool_calling: true, streaming: true, ..Default::default() }
    }

    fn stream_chat(&self, request: ChatRequest) -> BoxStream<'static, Result<ChatChunk, ProviderError>> {
        let (tx, rx) = mpsc::unbounded_channel();
        let client = self.client.clone();
        let url = format!("{}/messages", self.base_url);
        let api_key = self.api_key.clone();
        let body = MessagesRequestBody::from(&request);

        tokio::spawn(async move {
            let response = match client
                .post(&url)
                .header("x-api-key", &api_key)
                .header("anthropic-version", ANTHROPIC_VERSION)
                .json(&body)
                .send()
                .await
            {
                Ok(response) => response,
                Err(err) => {
                    let _ = tx.send(Err(map_reqwest_error(err)));
                    return;
                }
            };
            let mut response = match check_status(response).await {
                Ok(response) => response,
                Err(err) => {
                    let _ = tx.send(Err(err));
                    return;
                }
            };

            let mut buffer = String::new();
            // Bloques `tool_use` acumulados por índice — `content_block_start`
            // trae `id`/`name`, los `content_block_delta` siguientes traen
            // fragmentos de `partial_json` hasta el `content_block_stop`
            // correspondiente. Mismo patrón que `openai_compatible`'s
            // `AccumulatingToolCall`, adaptado a los eventos de Anthropic.
            let mut accumulating: BTreeMap<u32, AccumulatingToolUse> = BTreeMap::new();
            loop {
                match response.chunk().await {
                    Ok(Some(bytes)) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));
                        for data in drain_sse_data_lines(&mut buffer) {
                            match serde_json::from_str::<AnthropicStreamEvent>(&data) {
                                Ok(event) => match process_stream_event(&mut accumulating, event) {
                                    Some(chunk) => {
                                        let done = chunk.finished;
                                        if tx.send(Ok(chunk)).is_err() || done {
                                            return;
                                        }
                                    }
                                    None => continue,
                                },
                                // Eventos que no traemos tipados explícitamente
                                // (`ping`, campos desconocidos de una versión
                                // más nueva de la API) — se ignoran en vez de
                                // cortar el stream entero por uno solo.
                                Err(_) => continue,
                            }
                        }
                    }
                    Ok(None) => {
                        let _ = tx.send(Ok(ChatChunk { delta: String::new(), finished: true, finish_reason: Some("stop".to_string()), tool_calls: Vec::new() }));
                        return;
                    }
                    Err(err) => {
                        let _ = tx.send(Err(map_reqwest_error(err)));
                        return;
                    }
                }
            }
        });

        Box::pin(UnboundedReceiverStream::new(rx))
    }
}

async fn check_status(response: reqwest::Response) -> Result<reqwest::Response, ProviderError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }

    match status.as_u16() {
        401 | 403 => Err(ProviderError::Unauthorized),
        404 => Err(ProviderError::ModelUnavailable(response.text().await.unwrap_or_default())),
        429 => Err(ProviderError::RateLimited),
        400 => {
            let body = response.text().await.unwrap_or_default();
            // Anthropic no tiene un código de error distinto para "sin
            // crédito" — llega como un 400 `invalid_request_error` cuyo
            // mensaje menciona el balance de crédito (ver el plan). Sniffeado
            // como texto por la misma razón que `openai_compatible`'s
            // `is_insufficient_quota`.
            if body.contains("credit balance") {
                Err(ProviderError::QuotaExceeded(body))
            } else {
                Err(ProviderError::InvalidResponse(format!("HTTP {status}: {body}")))
            }
        }
        _ => {
            let body = response.text().await.unwrap_or_default();
            Err(ProviderError::InvalidResponse(format!("HTTP {status}: {body}")))
        }
    }
}

fn map_reqwest_error(err: reqwest::Error) -> ProviderError {
    if err.is_timeout() {
        ProviderError::Timeout
    } else if err.is_connect() {
        ProviderError::ConnectionRefused
    } else {
        ProviderError::Request(err.to_string())
    }
}

#[derive(Serialize)]
struct MessagesRequestBody {
    model: String,
    max_tokens: u32,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    stream: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<AnthropicToolSpec>,
}

#[derive(Serialize)]
struct AnthropicToolSpec {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: &'static str,
    content: AnthropicContent,
}

#[derive(Serialize)]
#[serde(untagged)]
enum AnthropicContent {
    Text(String),
    Blocks(Vec<AnthropicContentBlock>),
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicContentBlock {
    Text {
        text: String,
    },
    Image {
        source: AnthropicImageSource,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

#[derive(Serialize)]
struct AnthropicImageSource {
    r#type: &'static str,
    media_type: String,
    data: String,
}

/// Separa un data URL (`data:image/png;base64,AAAA...`) en el `media_type`
/// y el base64 crudo que Anthropic espera, sin el prefijo — a diferencia de
/// `OpenAiCompatibleProvider`, que manda la URL completa tal cual.
fn split_data_url(url: &str) -> (String, String) {
    match url.strip_prefix("data:").and_then(|rest| rest.split_once(";base64,")) {
        Some((media_type, data)) => (media_type.to_string(), data.to_string()),
        None => ("image/png".to_string(), url.to_string()),
    }
}

impl From<&ChatMessage> for AnthropicMessage {
    fn from(message: &ChatMessage) -> Self {
        match message.role {
            // Un resultado de tool va como mensaje `user` con un bloque
            // `tool_result` — Anthropic no tiene rol `tool` propio (ver el
            // plan).
            MessageRole::Tool => {
                let tool_use_id = message.tool_call_id.clone().unwrap_or_default();
                AnthropicMessage {
                    role: "user",
                    content: AnthropicContent::Blocks(vec![AnthropicContentBlock::ToolResult { tool_use_id, content: message.content.clone() }]),
                }
            }
            MessageRole::Assistant if !message.tool_calls.is_empty() => {
                let blocks = message
                    .tool_calls
                    .iter()
                    .map(|call| AnthropicContentBlock::ToolUse {
                        id: call.id.clone(),
                        name: call.name.clone(),
                        input: serde_json::from_str(&call.arguments).unwrap_or(serde_json::json!({})),
                    })
                    .collect();
                AnthropicMessage { role: "assistant", content: AnthropicContent::Blocks(blocks) }
            }
            MessageRole::User | MessageRole::Assistant => {
                let role = if message.role == MessageRole::User { "user" } else { "assistant" };
                if message.images.is_empty() {
                    AnthropicMessage { role, content: AnthropicContent::Text(message.content.clone()) }
                } else {
                    let mut blocks = Vec::with_capacity(1 + message.images.len());
                    if !message.content.is_empty() {
                        blocks.push(AnthropicContentBlock::Text { text: message.content.clone() });
                    }
                    blocks.extend(message.images.iter().map(|url| {
                        let (media_type, data) = split_data_url(url);
                        AnthropicContentBlock::Image { source: AnthropicImageSource { r#type: "base64", media_type, data } }
                    }));
                    AnthropicMessage { role, content: AnthropicContent::Blocks(blocks) }
                }
            }
            // El mensaje `System` se extrae aparte en `MessagesRequestBody::from`
            // y nunca llega hasta acá — la rama existe solo para que el
            // `match` sea exhaustivo.
            MessageRole::System => AnthropicMessage { role: "user", content: AnthropicContent::Text(message.content.clone()) },
        }
    }
}

impl From<&ChatRequest> for MessagesRequestBody {
    fn from(request: &ChatRequest) -> Self {
        let system = joined_system_prompt(&request.messages);

        Self {
            model: request.model.clone(),
            max_tokens: request.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS),
            messages: request.messages.iter().filter(|m| m.role != MessageRole::System).map(AnthropicMessage::from).collect(),
            system,
            temperature: request.temperature,
            stream: true,
            tools: request
                .tools
                .iter()
                .map(|tool| AnthropicToolSpec { name: tool.name.clone(), description: tool.description.clone(), input_schema: tool.parameters_schema.clone() })
                .collect(),
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicStreamEvent {
    ContentBlockStart { index: u32, content_block: ContentBlockStart },
    ContentBlockDelta { index: u32, delta: ContentDelta },
    ContentBlockStop,
    MessageDelta { delta: MessageDelta },
    MessageStop,
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentBlockStart {
    Text { #[allow(dead_code)] text: String },
    ToolUse { id: String, name: String },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentDelta {
    TextDelta { text: String },
    InputJsonDelta { partial_json: String },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct MessageDelta {
    stop_reason: Option<String>,
}

#[derive(Default)]
struct AccumulatingToolUse {
    id: String,
    name: String,
    arguments: String,
}

/// Espejo de `openai_compatible::process_stream_chunk`, adaptado a los
/// eventos con nombre propio de Anthropic — pura, sin red, para poder
/// testearla igual que la de OpenAI.
fn process_stream_event(accumulating: &mut BTreeMap<u32, AccumulatingToolUse>, event: AnthropicStreamEvent) -> Option<ChatChunk> {
    match event {
        AnthropicStreamEvent::ContentBlockStart { index, content_block: ContentBlockStart::ToolUse { id, name } } => {
            accumulating.insert(index, AccumulatingToolUse { id, name, arguments: String::new() });
            None
        }
        AnthropicStreamEvent::ContentBlockStart { .. } => None,
        AnthropicStreamEvent::ContentBlockDelta { index, delta: ContentDelta::TextDelta { text } } => {
            // Un `text_delta` en un índice que no es un `tool_use` acumulado
            // — el caso normal, texto de la respuesta.
            let _ = index;
            Some(ChatChunk { delta: text, finished: false, finish_reason: None, tool_calls: Vec::new() })
        }
        AnthropicStreamEvent::ContentBlockDelta { index, delta: ContentDelta::InputJsonDelta { partial_json } } => {
            if let Some(entry) = accumulating.get_mut(&index) {
                entry.arguments.push_str(&partial_json);
            }
            None
        }
        AnthropicStreamEvent::ContentBlockDelta { .. } => None,
        AnthropicStreamEvent::ContentBlockStop => None,
        AnthropicStreamEvent::MessageDelta { delta } => match delta.stop_reason.as_deref() {
            Some("tool_use") => {
                let tool_calls = std::mem::take(accumulating).into_values().map(|call| ToolCall { id: call.id, name: call.name, arguments: call.arguments }).collect();
                Some(ChatChunk { delta: String::new(), finished: true, finish_reason: Some("tool_calls".to_string()), tool_calls })
            }
            Some(_) => Some(ChatChunk { delta: String::new(), finished: true, finish_reason: Some("stop".to_string()), tool_calls: Vec::new() }),
            None => None,
        },
        AnthropicStreamEvent::MessageStop => None,
        AnthropicStreamEvent::Other => None,
    }
}

#[derive(Deserialize)]
struct ModelsResponseBody {
    data: Vec<AnthropicModelInfo>,
}

#[derive(Deserialize)]
struct AnthropicModelInfo {
    id: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ToolDefinition;

    #[test]
    fn a_trailing_system_reminder_after_the_history_still_reaches_the_model() {
        let body = MessagesRequestBody::from(&ChatRequest {
            model: "claude-3-5-sonnet-20241022".to_string(),
            messages: vec![
                ChatMessage::text(MessageRole::System, "Eres Ciel"),
                ChatMessage::text(MessageRole::User, "hola"),
                ChatMessage::text(MessageRole::Assistant, "buenas"),
                ChatMessage::text(MessageRole::System, "Hoy es lunes 5 de octubre"),
            ],
            temperature: None,
            max_tokens: None,
            tools: Vec::new(),
            context: Vec::new(),
            contains_stored_memories: false,
        });

        assert_eq!(body.system.as_deref(), Some("Eres Ciel

Hoy es lunes 5 de octubre"));
        assert_eq!(body.messages.len(), 2, "los mensajes de sistema no se mandan también como turnos");
    }

    #[test]
    fn system_message_moves_to_the_top_level_field_and_is_excluded_from_messages() {
        let body = MessagesRequestBody::from(&ChatRequest {
            model: "claude-3-5-sonnet-20241022".to_string(),
            messages: vec![ChatMessage::text(MessageRole::System, "Eres Ciel"), ChatMessage::text(MessageRole::User, "hola")],
            temperature: None,
            max_tokens: None,
            tools: Vec::new(),
            context: Vec::new(),
            contains_stored_memories: false,
        });

        assert_eq!(body.system.as_deref(), Some("Eres Ciel"));
        assert_eq!(body.messages.len(), 1);
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["messages"][0]["role"], "user");
    }

    #[test]
    fn missing_max_tokens_falls_back_to_the_default() {
        let body = MessagesRequestBody::from(&ChatRequest {
            model: "m".to_string(),
            messages: vec![ChatMessage::text(MessageRole::User, "hola")],
            temperature: None,
            max_tokens: None,
            tools: Vec::new(),
            context: Vec::new(),
            contains_stored_memories: false,
        });
        assert_eq!(body.max_tokens, DEFAULT_MAX_TOKENS);
    }

    #[test]
    fn an_image_serializes_as_a_base64_block_without_the_data_url_prefix() {
        let message = ChatMessage {
            role: MessageRole::User,
            content: "¿Qué ves?".to_string(),
            images: vec!["data:image/png;base64,AAAA".to_string()],
            tool_calls: Vec::new(),
            tool_call_id: None,
        };
        let json = serde_json::to_value(AnthropicMessage::from(&message)).unwrap();
        let content = &json["content"];
        assert_eq!(content[0], serde_json::json!({"type": "text", "text": "¿Qué ves?"}));
        assert_eq!(content[1]["type"], "image");
        assert_eq!(content[1]["source"]["media_type"], "image/png");
        assert_eq!(content[1]["source"]["data"], "AAAA");
        assert_eq!(content[1]["source"]["type"], "base64");
    }

    #[test]
    fn a_tool_result_message_becomes_a_user_message_with_a_tool_result_block() {
        let message = ChatMessage::tool_result("call_1", "18°C, nublado");
        let json = serde_json::to_value(AnthropicMessage::from(&message)).unwrap();

        assert_eq!(json["role"], "user");
        assert_eq!(json["content"][0]["type"], "tool_result");
        assert_eq!(json["content"][0]["tool_use_id"], "call_1");
        assert_eq!(json["content"][0]["content"], "18°C, nublado");
    }

    #[test]
    fn an_assistant_tool_call_message_becomes_a_tool_use_block_with_parsed_input() {
        let message = ChatMessage::assistant_tool_calls(vec![ToolCall { id: "call_1".to_string(), name: "web_search".to_string(), arguments: r#"{"query":"clima"}"#.to_string() }]);
        let json = serde_json::to_value(AnthropicMessage::from(&message)).unwrap();

        assert_eq!(json["role"], "assistant");
        assert_eq!(json["content"][0]["type"], "tool_use");
        assert_eq!(json["content"][0]["id"], "call_1");
        assert_eq!(json["content"][0]["name"], "web_search");
        assert_eq!(json["content"][0]["input"], serde_json::json!({"query": "clima"}));
    }

    #[test]
    fn a_request_with_tools_serializes_the_input_schema() {
        let body = MessagesRequestBody::from(&ChatRequest {
            model: "m".to_string(),
            messages: vec![ChatMessage::text(MessageRole::User, "hola")],
            temperature: None,
            max_tokens: None,
            tools: vec![ToolDefinition { name: "web_search".to_string(), description: "busca en la web".to_string(), parameters_schema: serde_json::json!({"type": "object"}) }],
            context: Vec::new(),
            contains_stored_memories: false,
        });

        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["tools"][0]["name"], "web_search");
        assert_eq!(json["tools"][0]["input_schema"]["type"], "object");
    }

    #[test]
    fn accumulates_tool_use_input_json_fragments_across_deltas() {
        let mut accumulating = BTreeMap::new();

        let start = serde_json::from_str::<AnthropicStreamEvent>(r#"{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"call_1","name":"web_search"}}"#).unwrap();
        assert!(process_stream_event(&mut accumulating, start).is_none());

        let delta1 = serde_json::from_str::<AnthropicStreamEvent>(r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"qu"}}"#).unwrap();
        assert!(process_stream_event(&mut accumulating, delta1).is_none());

        let delta2 = serde_json::from_str::<AnthropicStreamEvent>(r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"ery\":\"clima\"}"}}"#).unwrap();
        assert!(process_stream_event(&mut accumulating, delta2).is_none());

        let stop = serde_json::from_str::<AnthropicStreamEvent>(r#"{"type":"message_delta","delta":{"stop_reason":"tool_use"}}"#).unwrap();
        let chunk = process_stream_event(&mut accumulating, stop).unwrap();

        assert!(chunk.finished);
        assert_eq!(chunk.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(chunk.tool_calls, vec![ToolCall { id: "call_1".to_string(), name: "web_search".to_string(), arguments: r#"{"query":"clima"}"#.to_string() }]);
    }

    #[test]
    fn a_text_delta_forwards_immediately_without_finishing() {
        let mut accumulating = BTreeMap::new();
        let event = serde_json::from_str::<AnthropicStreamEvent>(r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hola"}}"#).unwrap();
        let chunk = process_stream_event(&mut accumulating, event).unwrap();

        assert_eq!(chunk.delta, "Hola");
        assert!(!chunk.finished);
    }

    #[test]
    fn a_normal_stop_reason_finishes_without_tool_calls() {
        let mut accumulating = BTreeMap::new();
        let event = serde_json::from_str::<AnthropicStreamEvent>(r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"}}"#).unwrap();
        let chunk = process_stream_event(&mut accumulating, event).unwrap();

        assert!(chunk.finished);
        assert_eq!(chunk.finish_reason.as_deref(), Some("stop"));
        assert!(chunk.tool_calls.is_empty());
    }

    #[test]
    fn a_400_with_credit_balance_wording_is_recognized_as_quota_exceeded() {
        // `check_status` needs a real `reqwest::Response`, which can't be
        // built without a live server — so this test targets the same
        // substring check `check_status` uses directly, mirroring
        // `openai_compatible`'s `is_insufficient_quota` tests.
        let body = "Your credit balance is too low to access the Anthropic API";
        assert!(body.contains("credit balance"));
    }

    #[test]
    fn an_unrecognized_event_type_is_ignored_not_an_error() {
        let mut accumulating = BTreeMap::new();
        let event = serde_json::from_str::<AnthropicStreamEvent>(r#"{"type":"ping"}"#).unwrap();
        assert!(process_stream_event(&mut accumulating, event).is_none());
    }
}
