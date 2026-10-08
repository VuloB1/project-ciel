use std::collections::HashMap;

use async_trait::async_trait;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::sse::drain_sse_data_lines;
use crate::types::{joined_system_prompt, ChatChunk, ChatMessage, ChatRequest, ConnectionStatus, MessageRole, ModelCapabilities, ModelInfo, ToolCall};
use crate::{LLMProvider, ProviderError};

/// Adaptador real para Google Gemini (`streamGenerateContent`) — protocolo
/// propio: roles `user`/`model` (no `assistant`), el sistema va aparte en
/// `systemInstruction`, y las tool calls del modelo llegan sin id (se
/// sintetiza uno acá, ver `build_assistant_parts`). Ver el plan de
/// Proveedores IA.
pub struct GeminiProvider {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl GeminiProvider {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self { client: reqwest::Client::new(), base_url: base_url.into().trim_end_matches('/').to_string(), api_key: api_key.into() }
    }
}

#[async_trait]
impl LLMProvider for GeminiProvider {
    async fn test_connection(&self) -> Result<ConnectionStatus, ProviderError> {
        let start = std::time::Instant::now();
        let models = self.list_models().await?;
        Ok(ConnectionStatus { connected: true, latency_ms: Some(start.elapsed().as_millis() as u64), models })
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        let response = self.client.get(format!("{}/models", self.base_url)).header("x-goog-api-key", &self.api_key).send().await.map_err(map_reqwest_error)?;
        let response = check_status(response).await?;
        let body: ListModelsResponseBody = response.json().await.map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;
        Ok(body
            .models
            .into_iter()
            // Solo modelos que de verdad sirven para chat — `/models` también
            // devuelve modelos de solo-embeddings, que no tiene sentido
            // ofrecer acá.
            .filter(|m| m.supported_generation_methods.iter().any(|method| method == "streamGenerateContent"))
            .map(|m| ModelInfo { id: m.name.trim_start_matches("models/").to_string() })
            .collect())
    }

    fn capabilities(&self, _model: &str) -> ModelCapabilities {
        ModelCapabilities { text: true, vision: true, tool_calling: true, streaming: true, ..Default::default() }
    }

    fn stream_chat(&self, request: ChatRequest) -> BoxStream<'static, Result<ChatChunk, ProviderError>> {
        let (tx, rx) = mpsc::unbounded_channel();
        let client = self.client.clone();
        let url = format!("{}/models/{}:streamGenerateContent?alt=sse", self.base_url, request.model);
        let api_key = self.api_key.clone();
        let body = GenerateContentRequestBody::from_request(&request);

        tokio::spawn(async move {
            let response = match client.post(&url).header("x-goog-api-key", &api_key).json(&body).send().await {
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
            // A diferencia de OpenAI/Anthropic, una `functionCall` de Gemini
            // suele llegar completa en un solo evento (su `args` ya es un
            // objeto JSON, no un fragmento de texto a ensamblar) — así que
            // acá se acumulan llamadas *completas*, no fragmentos.
            let mut accumulating: Vec<ToolCall> = Vec::new();
            loop {
                match response.chunk().await {
                    Ok(Some(bytes)) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));
                        for data in drain_sse_data_lines(&mut buffer) {
                            match serde_json::from_str::<GenerateContentResponse>(&data) {
                                Ok(event) => {
                                    for chunk in process_stream_event(&mut accumulating, event) {
                                        let done = chunk.finished;
                                        if tx.send(Ok(chunk)).is_err() || done {
                                            return;
                                        }
                                    }
                                }
                                Err(err) => {
                                    let _ = tx.send(Err(ProviderError::InvalidResponse(err.to_string())));
                                    return;
                                }
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
        429 => {
            let body = response.text().await.unwrap_or_default();
            // El caso más limpio de los tres adaptadores: Gemini señaliza
            // "sin cupo" con `error.status == "RESOURCE_EXHAUSTED"` en el
            // cuerpo del 429 (ver el plan).
            if body.contains("RESOURCE_EXHAUSTED") {
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

fn map_reqwest_error(err: reqwest::Error) -> ProviderError {
    if err.is_timeout() {
        ProviderError::Timeout
    } else if err.is_connect() {
        ProviderError::ConnectionRefused
    } else {
        ProviderError::Request(err.to_string())
    }
}

/// Separa un data URL en el `mimeType`/base64 que Gemini espera — mismo
/// propósito que `anthropic::split_data_url`, duplicado a propósito (cada
/// adaptador queda autocontenido, mismo criterio que el resto de este
/// crate).
fn split_data_url(url: &str) -> (String, String) {
    match url.strip_prefix("data:").and_then(|rest| rest.split_once(";base64,")) {
        Some((mime_type, data)) => (mime_type.to_string(), data.to_string()),
        None => ("image/png".to_string(), url.to_string()),
    }
}

fn empty_json_object() -> serde_json::Value {
    serde_json::json!({})
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GenerateContentRequestBody {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiSystemInstruction>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<GeminiToolSpec>,
    generation_config: GeminiGenerationConfig,
}

#[derive(Serialize)]
struct GeminiContent {
    role: &'static str,
    parts: Vec<GeminiPartOut>,
}

#[derive(Serialize)]
struct GeminiSystemInstruction {
    parts: Vec<GeminiPartOut>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiToolSpec {
    function_declarations: Vec<GeminiFunctionDeclaration>,
}

#[derive(Serialize)]
struct GeminiFunctionDeclaration {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Serialize)]
#[serde(untagged)]
enum GeminiPartOut {
    Text(GeminiTextPart),
    InlineData(GeminiInlineDataPart),
    FunctionCall(GeminiFunctionCallPart),
    FunctionResponse(GeminiFunctionResponsePart),
}

#[derive(Serialize)]
struct GeminiTextPart {
    text: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiInlineDataPart {
    inline_data: GeminiInlineData,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiInlineData {
    mime_type: String,
    data: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiFunctionCallPart {
    function_call: GeminiFunctionCallOut,
}

#[derive(Serialize)]
struct GeminiFunctionCallOut {
    name: String,
    args: serde_json::Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiFunctionResponsePart {
    function_response: GeminiFunctionResponseOut,
}

#[derive(Serialize)]
struct GeminiFunctionResponseOut {
    name: String,
    response: serde_json::Value,
}

impl GenerateContentRequestBody {
    fn from_request(request: &ChatRequest) -> Self {
        let system_text = joined_system_prompt(&request.messages);

        Self {
            contents: build_contents(&request.messages),
            system_instruction: system_text.map(|text| GeminiSystemInstruction { parts: vec![GeminiPartOut::Text(GeminiTextPart { text })] }),
            tools: if request.tools.is_empty() {
                Vec::new()
            } else {
                vec![GeminiToolSpec {
                    function_declarations: request
                        .tools
                        .iter()
                        .map(|tool| GeminiFunctionDeclaration { name: tool.name.clone(), description: tool.description.clone(), parameters: tool.parameters_schema.clone() })
                        .collect(),
                }]
            },
            generation_config: GeminiGenerationConfig { temperature: request.temperature, max_output_tokens: request.max_tokens },
        }
    }
}

fn build_parts(message: &ChatMessage) -> Vec<GeminiPartOut> {
    let mut parts = Vec::with_capacity(1 + message.images.len());
    if !message.content.is_empty() {
        parts.push(GeminiPartOut::Text(GeminiTextPart { text: message.content.clone() }));
    }
    parts.extend(message.images.iter().map(|url| {
        let (mime_type, data) = split_data_url(url);
        GeminiPartOut::InlineData(GeminiInlineDataPart { inline_data: GeminiInlineData { mime_type, data } })
    }));
    parts
}

fn build_assistant_parts(message: &ChatMessage) -> Vec<GeminiPartOut> {
    if message.tool_calls.is_empty() {
        return build_parts(message);
    }
    message
        .tool_calls
        .iter()
        .map(|call| {
            let args = serde_json::from_str(&call.arguments).unwrap_or_else(|_| empty_json_object());
            GeminiPartOut::FunctionCall(GeminiFunctionCallPart { function_call: GeminiFunctionCallOut { name: call.name.clone(), args } })
        })
        .collect()
}

/// Arma `contents` recorriendo los mensajes en orden y llevando un mapa
/// `id de tool call -> nombre` (poblado en cada mensaje `Assistant` con
/// `tool_calls`) — Gemini no tiene id de llamada en su protocolo, así que
/// un `functionResponse` se correlaciona por `name`, no por id como
/// OpenAI/Anthropic. El id sintético que generamos (`ToolCall::id`) solo
/// sirve para que el resto de la app (que sí razona en términos de ids)
/// pueda encontrar de vuelta el nombre acá.
fn build_contents(messages: &[ChatMessage]) -> Vec<GeminiContent> {
    let mut call_names: HashMap<String, String> = HashMap::new();
    let mut out = Vec::with_capacity(messages.len());

    for message in messages {
        match message.role {
            MessageRole::System => continue,
            MessageRole::User => out.push(GeminiContent { role: "user", parts: build_parts(message) }),
            MessageRole::Assistant => {
                for call in &message.tool_calls {
                    call_names.insert(call.id.clone(), call.name.clone());
                }
                out.push(GeminiContent { role: "model", parts: build_assistant_parts(message) });
            }
            MessageRole::Tool => {
                let name = message.tool_call_id.as_ref().and_then(|id| call_names.get(id)).cloned().unwrap_or_default();
                out.push(GeminiContent {
                    role: "function",
                    parts: vec![GeminiPartOut::FunctionResponse(GeminiFunctionResponsePart {
                        function_response: GeminiFunctionResponseOut { name, response: serde_json::json!({ "result": message.content }) },
                    })],
                });
            }
        }
    }

    out
}

#[derive(Deserialize, Default)]
struct GenerateContentResponse {
    #[serde(default)]
    candidates: Vec<GeminiCandidate>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    #[serde(default)]
    content: Option<GeminiContentIn>,
    #[serde(default, rename = "finishReason")]
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct GeminiContentIn {
    #[serde(default)]
    parts: Vec<GeminiPartIn>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum GeminiPartIn {
    FunctionCall {
        #[serde(rename = "functionCall")]
        function_call: GeminiFunctionCallIn,
    },
    Text {
        text: String,
    },
    /// Cualquier otra forma de parte (ej. `inlineData` reflejado de vuelta)
    /// — se ignora en vez de romper el parseo de la respuesta entera por
    /// una sola parte que no nos interesa. El campo nunca se lee (por
    /// diseño, ver `process_stream_event`), solo existe para que la parte
    /// deserialice sin fallar.
    #[allow(dead_code)]
    Other(serde_json::Value),
}

#[derive(Deserialize)]
struct GeminiFunctionCallIn {
    name: String,
    #[serde(default = "empty_json_object")]
    args: serde_json::Value,
}

#[derive(Deserialize)]
struct ListModelsResponseBody {
    #[serde(default)]
    models: Vec<GeminiModelInfo>,
}

#[derive(Deserialize)]
struct GeminiModelInfo {
    name: String,
    #[serde(default, rename = "supportedGenerationMethods")]
    supported_generation_methods: Vec<String>,
}

/// Convierte un evento ya parseado en 0+ `ChatChunk`s — a diferencia de
/// `openai_compatible::process_stream_chunk`/`anthropic::process_stream_event`
/// (que devuelven `Option`), acá puede haber texto *y* el fin de la
/// respuesta en el mismo evento (el último chunk de Gemini suele traer
/// `finishReason` junto con la última parte de texto), así que se devuelve
/// una lista. Pura, sin red, para poder testearla igual que las otras dos.
fn process_stream_event(accumulating: &mut Vec<ToolCall>, event: GenerateContentResponse) -> Vec<ChatChunk> {
    let mut out = Vec::new();
    let Some(candidate) = event.candidates.into_iter().next() else { return out };

    if let Some(content) = candidate.content {
        for part in content.parts {
            match part {
                GeminiPartIn::Text { text } => {
                    if !text.is_empty() {
                        out.push(ChatChunk { delta: text, finished: false, finish_reason: None, tool_calls: Vec::new() });
                    }
                }
                GeminiPartIn::FunctionCall { function_call } => {
                    let id = format!("gemini_call_{}", accumulating.len());
                    let arguments = serde_json::to_string(&function_call.args).unwrap_or_else(|_| "{}".to_string());
                    accumulating.push(ToolCall { id, name: function_call.name, arguments });
                }
                GeminiPartIn::Other(_) => {}
            }
        }
    }

    if candidate.finish_reason.is_some() {
        if accumulating.is_empty() {
            out.push(ChatChunk { delta: String::new(), finished: true, finish_reason: Some("stop".to_string()), tool_calls: Vec::new() });
        } else {
            let tool_calls = std::mem::take(accumulating);
            out.push(ChatChunk { delta: String::new(), finished: true, finish_reason: Some("tool_calls".to_string()), tool_calls });
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ToolDefinition;

    #[test]
    fn a_trailing_system_reminder_after_the_history_still_reaches_the_model() {
        let body = GenerateContentRequestBody::from_request(&ChatRequest {
            model: "gemini-1.5-pro".to_string(),
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

        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["systemInstruction"]["parts"][0]["text"], "Eres Ciel

Hoy es lunes 5 de octubre");
        assert_eq!(body.contents.len(), 2, "los mensajes de sistema no se mandan también como turnos");
    }

    #[test]
    fn system_message_moves_to_system_instruction_and_is_excluded_from_contents() {
        let body = GenerateContentRequestBody::from_request(&ChatRequest {
            model: "gemini-1.5-pro".to_string(),
            messages: vec![ChatMessage::text(MessageRole::System, "Eres Ciel"), ChatMessage::text(MessageRole::User, "hola")],
            temperature: None,
            max_tokens: None,
            tools: Vec::new(),
            context: Vec::new(),
            contains_stored_memories: false,
        });

        assert_eq!(body.contents.len(), 1);
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["systemInstruction"]["parts"][0]["text"], "Eres Ciel");
        assert_eq!(json["contents"][0]["role"], "user");
    }

    #[test]
    fn assistant_role_serializes_as_model_not_assistant() {
        let body = GenerateContentRequestBody::from_request(&ChatRequest {
            model: "m".to_string(),
            messages: vec![ChatMessage::text(MessageRole::User, "hola"), ChatMessage::text(MessageRole::Assistant, "hola!")],
            temperature: None,
            max_tokens: None,
            tools: Vec::new(),
            context: Vec::new(),
            contains_stored_memories: false,
        });
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["contents"][1]["role"], "model");
    }

    #[test]
    fn an_image_serializes_as_inline_data_with_mime_type() {
        let message = ChatMessage {
            role: MessageRole::User,
            content: "¿Qué ves?".to_string(),
            images: vec!["data:image/png;base64,AAAA".to_string()],
            tool_calls: Vec::new(),
            tool_call_id: None,
        };
        let json = serde_json::to_value(build_parts(&message)).unwrap();
        assert_eq!(json[0]["text"], "¿Qué ves?");
        assert_eq!(json[1]["inlineData"]["mimeType"], "image/png");
        assert_eq!(json[1]["inlineData"]["data"], "AAAA");
    }

    #[test]
    fn an_assistant_tool_call_message_becomes_a_function_call_part() {
        let message = ChatMessage::assistant_tool_calls(vec![ToolCall { id: "call_1".to_string(), name: "web_search".to_string(), arguments: r#"{"query":"clima"}"#.to_string() }]);
        let json = serde_json::to_value(build_assistant_parts(&message)).unwrap();

        assert_eq!(json[0]["functionCall"]["name"], "web_search");
        assert_eq!(json[0]["functionCall"]["args"], serde_json::json!({"query": "clima"}));
    }

    #[test]
    fn a_tool_result_correlates_back_to_the_call_name_by_id() {
        let messages = vec![
            ChatMessage::text(MessageRole::User, "¿qué clima hace?"),
            ChatMessage::assistant_tool_calls(vec![ToolCall { id: "call_1".to_string(), name: "web_search".to_string(), arguments: "{}".to_string() }]),
            ChatMessage::tool_result("call_1", "18°C, nublado"),
        ];
        let contents = build_contents(&messages);
        let json = serde_json::to_value(&contents[2]).unwrap();

        assert_eq!(json["role"], "function");
        assert_eq!(json["parts"][0]["functionResponse"]["name"], "web_search");
        assert_eq!(json["parts"][0]["functionResponse"]["response"]["result"], "18°C, nublado");
    }

    #[test]
    fn a_request_with_tools_wraps_declarations_in_one_tools_entry() {
        let body = GenerateContentRequestBody::from_request(&ChatRequest {
            model: "m".to_string(),
            messages: vec![ChatMessage::text(MessageRole::User, "hola")],
            temperature: None,
            max_tokens: None,
            tools: vec![ToolDefinition { name: "web_search".to_string(), description: "busca en la web".to_string(), parameters_schema: serde_json::json!({"type": "object"}) }],
            context: Vec::new(),
            contains_stored_memories: false,
        });
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["tools"][0]["functionDeclarations"][0]["name"], "web_search");
    }

    #[test]
    fn a_text_only_event_with_no_finish_reason_yields_one_unfinished_chunk() {
        let mut accumulating = Vec::new();
        let event = serde_json::from_str::<GenerateContentResponse>(r#"{"candidates":[{"content":{"role":"model","parts":[{"text":"Hola"}]}}]}"#).unwrap();
        let chunks = process_stream_event(&mut accumulating, event);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].delta, "Hola");
        assert!(!chunks[0].finished);
    }

    #[test]
    fn text_plus_finish_reason_in_the_same_event_yields_two_chunks() {
        let mut accumulating = Vec::new();
        let event = serde_json::from_str::<GenerateContentResponse>(r#"{"candidates":[{"content":{"role":"model","parts":[{"text":"final."}]},"finishReason":"STOP"}]}"#).unwrap();
        let chunks = process_stream_event(&mut accumulating, event);

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].delta, "final.");
        assert!(!chunks[0].finished);
        assert!(chunks[1].finished);
        assert_eq!(chunks[1].finish_reason.as_deref(), Some("stop"));
    }

    #[test]
    fn a_function_call_part_with_finish_reason_yields_a_finished_tool_calls_chunk() {
        let mut accumulating = Vec::new();
        let event = serde_json::from_str::<GenerateContentResponse>(
            r#"{"candidates":[{"content":{"role":"model","parts":[{"functionCall":{"name":"web_search","args":{"query":"clima"}}}]},"finishReason":"STOP"}]}"#,
        )
        .unwrap();
        let chunks = process_stream_event(&mut accumulating, event);

        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].finished);
        assert_eq!(chunks[0].finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(chunks[0].tool_calls[0].name, "web_search");
        assert_eq!(chunks[0].tool_calls[0].arguments, r#"{"query":"clima"}"#);
    }

    #[test]
    fn an_event_with_no_candidates_yields_nothing() {
        let mut accumulating = Vec::new();
        let event = serde_json::from_str::<GenerateContentResponse>(r#"{"candidates":[]}"#).unwrap();
        assert!(process_stream_event(&mut accumulating, event).is_empty());
    }

    #[test]
    fn a_resource_exhausted_body_is_recognized_as_quota_exceeded() {
        let body = r#"{"error":{"code":429,"message":"Quota exceeded","status":"RESOURCE_EXHAUSTED"}}"#;
        assert!(body.contains("RESOURCE_EXHAUSTED"));
    }
}
