use std::collections::BTreeMap;

use async_trait::async_trait;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::sse::drain_sse_data_lines;
use crate::types::{ChatChunk, ChatMessage, ChatRequest, ConnectionStatus, MessageRole, ModelCapabilities, ModelInfo, ToolCall};
use crate::{LLMProvider, ProviderError};

/// Talks to any server exposing the OpenAI Chat Completions API —
/// LM Studio, a self-hosted OpenAI-compatible server, etc. LM Studio is
/// just this provider pointed at `http://localhost:1234/v1` with no API
/// key (sección 7 del brief: "Generic OpenAI-Compatible Provider... esto
/// permitirá conectar LM Studio, servidores locales...").
pub struct OpenAiCompatibleProvider {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

impl OpenAiCompatibleProvider {
    pub fn new(base_url: impl Into<String>, api_key: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key,
        }
    }
}

#[async_trait]
impl LLMProvider for OpenAiCompatibleProvider {
    async fn test_connection(&self) -> Result<ConnectionStatus, ProviderError> {
        let start = std::time::Instant::now();
        let models = self.list_models().await?;
        Ok(ConnectionStatus {
            connected: true,
            latency_ms: Some(start.elapsed().as_millis() as u64),
            models,
        })
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        let mut builder = self.client.get(format!("{}/models", self.base_url));
        if let Some(key) = &self.api_key {
            builder = builder.bearer_auth(key);
        }

        let response = builder.send().await.map_err(map_reqwest_error)?;
        let response = check_status(response).await?;
        let body: ModelsResponseBody = response
            .json()
            .await
            .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;
        Ok(body.data)
    }

    fn capabilities(&self, _model: &str) -> ModelCapabilities {
        ModelCapabilities { text: true, streaming: true, ..Default::default() }
    }

    fn stream_chat(&self, request: ChatRequest) -> BoxStream<'static, Result<ChatChunk, ProviderError>> {
        let (tx, rx) = mpsc::unbounded_channel();
        let client = self.client.clone();
        let url = format!("{}/chat/completions", self.base_url);
        let api_key = self.api_key.clone();
        let body = ChatCompletionRequestBody::from(&request);

        // Fase 10b diagnostic: confirms whether a request actually offered
        // any tools at all — cheap to leave in permanently, since it only
        // fires when `tools` is non-empty (every other request is silent,
        // same as before this existed).
        if !request.tools.is_empty() {
            let names: Vec<&str> = request.tools.iter().map(|t| t.name.as_str()).collect();
            let system_len = request.messages.first().map(|m| m.content.len()).unwrap_or(0);
            tracing::info!(
                target: "system",
                "llm: pidiendo respuesta con herramientas disponibles: {names:?} — {} mensajes, prompt de sistema de {system_len} caracteres",
                request.messages.len(),
            );
        }

        tokio::spawn(async move {
            let mut builder = client.post(&url).json(&body);
            if let Some(key) = &api_key {
                builder = builder.bearer_auth(key);
            }

            let response = match builder.send().await {
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
            // Accumulates fragmented `tool_calls` deltas by index across
            // SSE events (see `accumulate_tool_call_deltas`'s doc comment)
            // until the terminal `finish_reason: "tool_calls"` event, at
            // which point they're complete and ready to hand back.
            let mut accumulating: BTreeMap<u32, AccumulatingToolCall> = BTreeMap::new();
            loop {
                match response.chunk().await {
                    Ok(Some(bytes)) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));

                        for data in drain_sse_data_lines(&mut buffer) {
                            if data == "[DONE]" {
                                let _ = tx.send(Ok(finished_chunk()));
                                return;
                            }

                            match serde_json::from_str::<StreamChunkBody>(&data) {
                                Ok(parsed) => match process_stream_chunk(&mut accumulating, parsed) {
                                    Some(chunk) => {
                                        let is_final = chunk.finished;
                                        if is_final {
                                            // Fase 10b diagnostic: the single
                                            // most useful line for "did the
                                            // model actually try to call a
                                            // tool" — logs the real
                                            // `finish_reason` the server sent
                                            // (e.g. "stop" means it just
                                            // answered directly and never
                                            // attempted a call; "tool_calls"
                                            // means it did).
                                            tracing::info!(
                                                target: "system",
                                                "llm: generación terminada, finish_reason={:?}, tool_calls_pedidas={}",
                                                chunk.finish_reason,
                                                chunk.tool_calls.len(),
                                            );
                                        }
                                        // Receiver dropped means the caller
                                        // cancelled generation — stop quietly.
                                        if tx.send(Ok(chunk)).is_err() || is_final {
                                            return;
                                        }
                                    }
                                    None => continue,
                                },
                                Err(err) => {
                                    let _ = tx.send(Err(ProviderError::InvalidResponse(err.to_string())));
                                    return;
                                }
                            }
                        }
                    }
                    Ok(None) => {
                        // Body ended without an explicit [DONE] sentinel —
                        // some servers omit it. Treat as a normal finish.
                        let _ = tx.send(Ok(finished_chunk()));
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

fn finished_chunk() -> ChatChunk {
    ChatChunk { delta: String::new(), finished: true, finish_reason: Some("stop".to_string()), tool_calls: Vec::new() }
}

async fn check_status(response: reqwest::Response) -> Result<reqwest::Response, ProviderError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }

    match status.as_u16() {
        401 | 403 => Err(ProviderError::Unauthorized),
        404 => Err(ProviderError::ModelUnavailable(response.text().await.unwrap_or_default())),
        429 => {
            let body = response.text().await.unwrap_or_default();
            if is_insufficient_quota(&body) {
                Err(ProviderError::QuotaExceeded(body))
            } else {
                Err(ProviderError::RateLimited)
            }
        }
        _ => {
            let body = response.text().await.unwrap_or_default();
            Err(ProviderError::InvalidResponse(format!("HTTP {status}: {body}")))
        }
    }
}

/// OpenAI (y todo servidor que copie su formato de error) marca la falta de
/// crédito con `error.code`/`error.type == "insufficient_quota"` en el
/// cuerpo de un 429 — un rate limit genérico no trae ese campo. Sniffeado
/// como texto en vez de deserializado a un struct porque solo nos importa
/// esa única señal, no el resto del cuerpo.
fn is_insufficient_quota(body: &str) -> bool {
    body.contains("insufficient_quota")
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
struct ChatCompletionRequestBody {
    model: String,
    messages: Vec<ChatCompletionMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    stream: bool,
    /// Omitted entirely (not even `"tools": []`) when there are none, for
    /// the best compatibility with servers that error on an unrecognized
    /// or empty `tools` field (Fase 10b).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<ToolSpec>,
}

#[derive(Serialize)]
struct ToolSpec {
    r#type: &'static str,
    function: FunctionSpec,
}

#[derive(Serialize)]
struct FunctionSpec {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Serialize)]
struct ChatCompletionMessage {
    role: &'static str,
    /// `None` (serialized as omitted, not `null` — see below) for a
    /// text-only message with nothing to say; an assistant message that
    /// *only* requested tool calls has empty `content` and is sent as
    /// `None` too, matching how OpenAI-compatible servers themselves emit
    /// such messages (Fase 10b).
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<MessageContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ToolCallWire>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Serialize)]
struct ToolCallWire {
    id: String,
    r#type: &'static str,
    function: ToolCallFunctionWire,
}

#[derive(Serialize)]
struct ToolCallFunctionWire {
    name: String,
    arguments: String,
}

/// The OpenAI Chat Completions API accepts `content` as either a plain
/// string or an array of typed parts — the array form is only needed once
/// there's at least one image, so text-only messages (still the vast
/// majority) keep sending the simpler string shape, for the best
/// compatibility with servers that only understand that form.
#[derive(Serialize)]
#[serde(untagged)]
enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
}

#[derive(Serialize)]
struct ImageUrl {
    url: String,
}

impl From<&ChatMessage> for MessageContent {
    fn from(message: &ChatMessage) -> Self {
        if message.images.is_empty() {
            return MessageContent::Text(message.content.clone());
        }

        let mut parts = Vec::with_capacity(1 + message.images.len());
        if !message.content.is_empty() {
            parts.push(ContentPart::Text { text: message.content.clone() });
        }
        parts.extend(message.images.iter().map(|url| ContentPart::ImageUrl { image_url: ImageUrl { url: url.clone() } }));
        MessageContent::Parts(parts)
    }
}

impl From<&ChatMessage> for ChatCompletionMessage {
    fn from(message: &ChatMessage) -> Self {
        let role = match message.role {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool => "tool",
        };

        let tool_calls = (!message.tool_calls.is_empty()).then(|| {
            message
                .tool_calls
                .iter()
                .map(|call| ToolCallWire {
                    id: call.id.clone(),
                    r#type: "function",
                    function: ToolCallFunctionWire { name: call.name.clone(), arguments: call.arguments.clone() },
                })
                .collect()
        });

        // An assistant message that only carries tool_calls has nothing to
        // say yet — send no `content` at all rather than an empty string,
        // matching the shape real OpenAI-compatible servers themselves
        // produce for such messages.
        let content = if message.content.is_empty() && !message.tool_calls.is_empty() { None } else { Some(message.into()) };

        Self { role, content, tool_calls, tool_call_id: message.tool_call_id.clone() }
    }
}

impl From<&ChatRequest> for ChatCompletionRequestBody {
    fn from(request: &ChatRequest) -> Self {
        Self {
            model: request.model.clone(),
            messages: request.messages.iter().map(ChatCompletionMessage::from).collect(),
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            stream: true,
            tools: request
                .tools
                .iter()
                .map(|tool| ToolSpec {
                    r#type: "function",
                    function: FunctionSpec {
                        name: tool.name.clone(),
                        description: tool.description.clone(),
                        parameters: tool.parameters_schema.clone(),
                    },
                })
                .collect(),
        }
    }
}

#[derive(Deserialize)]
struct StreamChunkBody {
    choices: Vec<StreamChoice>,
}

#[derive(Deserialize)]
struct StreamChoice {
    delta: StreamDelta,
    finish_reason: Option<String>,
}

#[derive(Deserialize, Default)]
struct StreamDelta {
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<StreamToolCallDelta>,
}

#[derive(Deserialize)]
struct StreamToolCallDelta {
    index: u32,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<StreamFunctionDelta>,
}

#[derive(Deserialize, Default)]
struct StreamFunctionDelta {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Default)]
struct AccumulatingToolCall {
    id: String,
    name: String,
    arguments: String,
}

/// Turns one parsed SSE event into an outgoing `ChatChunk`, or `None` if
/// it's just another fragment of a tool call still being accumulated
/// (`accumulating` carries that state across calls — one per `stream_chat`
/// invocation). Pulled out as a pure function so the accumulation logic
/// (the genuinely tricky part of Fase 10b's streaming support) is
/// unit-testable without any networking involved, same reasoning as
/// `sse::drain_sse_data_lines`.
///
/// A tool name/id can itself arrive split across the *first* one or two
/// deltas for that call (some servers send `id` and `function.name`
/// together in delta 0, then only `function.arguments` fragments after —
/// others split even those) — accumulating every field by `push_str`
/// handles both without needing to special-case either shape.
fn process_stream_chunk(accumulating: &mut BTreeMap<u32, AccumulatingToolCall>, parsed: StreamChunkBody) -> Option<ChatChunk> {
    let choice = parsed.choices.into_iter().next()?;
    let had_tool_call_fragment = !choice.delta.tool_calls.is_empty();

    for delta in choice.delta.tool_calls {
        let entry = accumulating.entry(delta.index).or_default();
        if let Some(id) = delta.id {
            entry.id.push_str(&id);
        }
        if let Some(function) = delta.function {
            if let Some(name) = function.name {
                entry.name.push_str(&name);
            }
            if let Some(arguments) = function.arguments {
                entry.arguments.push_str(&arguments);
            }
        }
    }

    if choice.finish_reason.as_deref() == Some("tool_calls") {
        let tool_calls = std::mem::take(accumulating)
            .into_values()
            .map(|call| ToolCall { id: call.id, name: call.name, arguments: call.arguments })
            .collect();
        return Some(ChatChunk { delta: String::new(), finished: true, finish_reason: Some("tool_calls".to_string()), tool_calls });
    }

    let finished = choice.finish_reason.is_some();
    let delta_text = choice.delta.content.unwrap_or_default();
    if had_tool_call_fragment && delta_text.is_empty() && !finished {
        // A tool-call-only fragment (still accumulating) with nothing else
        // to report yet — every other shape (including a genuinely empty
        // non-tool-call delta, e.g. the role-only first event some servers
        // send) is forwarded exactly as before Fase 10b.
        return None;
    }

    Some(ChatChunk { delta: delta_text, finished, finish_reason: choice.finish_reason, tool_calls: Vec::new() })
}

#[derive(Deserialize)]
struct ModelsResponseBody {
    data: Vec<ModelInfo>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ToolDefinition;

    #[test]
    fn text_only_message_serializes_content_as_a_plain_string() {
        let message = ChatMessage::text(MessageRole::User, "Hola");
        let body = ChatCompletionRequestBody::from(&ChatRequest {
            model: "m".to_string(),
            messages: vec![message],
            temperature: None,
            max_tokens: None,
            tools: Vec::new(),
            context: Vec::new(),
            contains_stored_memories: false,
        });

        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["messages"][0]["content"], serde_json::json!("Hola"));
        assert!(json.get("tools").is_none());
    }

    #[test]
    fn message_with_an_image_serializes_content_as_typed_parts() {
        let message = ChatMessage {
            role: MessageRole::User,
            content: "¿Qué ves?".to_string(),
            images: vec!["data:image/png;base64,AAAA".to_string()],
            tool_calls: Vec::new(),
            tool_call_id: None,
        };
        let body = ChatCompletionRequestBody::from(&ChatRequest {
            model: "m".to_string(),
            messages: vec![message],
            temperature: None,
            max_tokens: None,
            tools: Vec::new(),
            context: Vec::new(),
            contains_stored_memories: false,
        });

        let json = serde_json::to_value(&body).unwrap();
        let content = &json["messages"][0]["content"];
        assert_eq!(content[0], serde_json::json!({"type": "text", "text": "¿Qué ves?"}));
        assert_eq!(
            content[1],
            serde_json::json!({"type": "image_url", "image_url": {"url": "data:image/png;base64,AAAA"}})
        );
    }

    #[test]
    fn an_image_with_no_text_omits_the_text_part() {
        let message = ChatMessage {
            role: MessageRole::User,
            content: String::new(),
            images: vec!["data:image/png;base64,AAAA".to_string()],
            tool_calls: Vec::new(),
            tool_call_id: None,
        };
        let content = MessageContent::from(&message);
        match content {
            MessageContent::Parts(parts) => {
                assert_eq!(parts.len(), 1);
                assert!(matches!(parts[0], ContentPart::ImageUrl { .. }));
            }
            MessageContent::Text(_) => panic!("expected the array form once there's an image"),
        }
    }

    #[test]
    fn a_request_with_tools_serializes_the_function_schema() {
        let body = ChatCompletionRequestBody::from(&ChatRequest {
            model: "m".to_string(),
            messages: vec![ChatMessage::text(MessageRole::User, "hola")],
            temperature: None,
            max_tokens: None,
            tools: vec![ToolDefinition {
                name: "web_search".to_string(),
                description: "busca en la web".to_string(),
                parameters_schema: serde_json::json!({"type": "object", "properties": {"query": {"type": "string"}}}),
            }],
            context: Vec::new(),
            contains_stored_memories: false,
        });

        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["tools"][0]["type"], "function");
        assert_eq!(json["tools"][0]["function"]["name"], "web_search");
        assert_eq!(json["tools"][0]["function"]["description"], "busca en la web");
        assert_eq!(json["tools"][0]["function"]["parameters"]["type"], "object");
    }

    #[test]
    fn an_assistant_tool_call_message_omits_content_and_includes_tool_calls() {
        let message = ChatMessage::assistant_tool_calls(vec![ToolCall {
            id: "call_1".to_string(),
            name: "web_search".to_string(),
            arguments: r#"{"query":"clima en Bogotá"}"#.to_string(),
        }]);
        let json = serde_json::to_value(ChatCompletionMessage::from(&message)).unwrap();

        assert!(json.get("content").is_none());
        assert_eq!(json["tool_calls"][0]["id"], "call_1");
        assert_eq!(json["tool_calls"][0]["type"], "function");
        assert_eq!(json["tool_calls"][0]["function"]["name"], "web_search");
    }

    #[test]
    fn a_tool_result_message_serializes_with_the_tool_role_and_call_id() {
        let message = ChatMessage::tool_result("call_1", "18°C, nublado");
        let json = serde_json::to_value(ChatCompletionMessage::from(&message)).unwrap();

        assert_eq!(json["role"], "tool");
        assert_eq!(json["tool_call_id"], "call_1");
        assert_eq!(json["content"], "18°C, nublado");
    }

    #[test]
    fn accumulates_tool_call_argument_fragments_across_deltas() {
        let mut accumulating = BTreeMap::new();

        let first = serde_json::from_str::<StreamChunkBody>(
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"web_search","arguments":"{\"qu"}}]},"finish_reason":null}]}"#,
        )
        .unwrap();
        assert!(process_stream_chunk(&mut accumulating, first).is_none());

        let second = serde_json::from_str::<StreamChunkBody>(
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"ery\":\"clima\"}"}}]},"finish_reason":null}]}"#,
        )
        .unwrap();
        assert!(process_stream_chunk(&mut accumulating, second).is_none());

        let last = serde_json::from_str::<StreamChunkBody>(r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#).unwrap();
        let chunk = process_stream_chunk(&mut accumulating, last).unwrap();

        assert!(chunk.finished);
        assert_eq!(chunk.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(chunk.tool_calls, vec![ToolCall { id: "call_1".to_string(), name: "web_search".to_string(), arguments: r#"{"query":"clima"}"#.to_string() }]);
    }

    #[test]
    fn accumulates_two_parallel_tool_calls_by_index() {
        let mut accumulating = BTreeMap::new();

        let deltas = serde_json::from_str::<StreamChunkBody>(
            r#"{"choices":[{"delta":{"tool_calls":[
                {"index":0,"id":"call_1","function":{"name":"web_search","arguments":"{\"query\":\"a\"}"}},
                {"index":1,"id":"call_2","function":{"name":"web_search","arguments":"{\"query\":\"b\"}"}}
            ]},"finish_reason":null}]}"#,
        )
        .unwrap();
        assert!(process_stream_chunk(&mut accumulating, deltas).is_none());

        let last = serde_json::from_str::<StreamChunkBody>(r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#).unwrap();
        let chunk = process_stream_chunk(&mut accumulating, last).unwrap();

        assert_eq!(chunk.tool_calls.len(), 2);
        assert_eq!(chunk.tool_calls[0].id, "call_1");
        assert_eq!(chunk.tool_calls[1].id, "call_2");
    }

    #[test]
    fn recognizes_an_insufficient_quota_body_as_quota_exceeded() {
        let body = r#"{"error":{"message":"You exceeded your current quota","type":"insufficient_quota","code":"insufficient_quota"}}"#;
        assert!(is_insufficient_quota(body));
    }

    #[test]
    fn a_generic_rate_limit_body_is_not_quota_exceeded() {
        let body = r#"{"error":{"message":"Rate limit reached","type":"requests","code":"rate_limit_exceeded"}}"#;
        assert!(!is_insufficient_quota(body));
    }

    #[test]
    fn a_normal_text_chunk_with_no_tool_calls_behaves_as_before() {
        let mut accumulating = BTreeMap::new();
        let parsed = serde_json::from_str::<StreamChunkBody>(r#"{"choices":[{"delta":{"content":"Hola"},"finish_reason":null}]}"#).unwrap();

        let chunk = process_stream_chunk(&mut accumulating, parsed).unwrap();
        assert_eq!(chunk.delta, "Hola");
        assert!(!chunk.finished);
        assert!(chunk.tool_calls.is_empty());
    }
}
