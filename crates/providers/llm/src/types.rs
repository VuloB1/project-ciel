use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    /// The result of a tool call (Fase 10b) — always paired with
    /// `ChatMessage::tool_call_id` naming which call it answers.
    Tool,
}

/// One function call the model asked for mid-response (Fase 10b) —
/// `arguments` is a raw JSON-encoded string exactly as the model produced
/// it, not yet parsed (whichever `Tool` runs it does the parsing, since
/// only it knows its own argument shape).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// Describes one callable tool to the model (Fase 10b) — `AIOrchestrator`
/// builds these from whatever `aivt_provider_tools::Tool`s it was given;
/// this type is what actually crosses into `ChatRequest` so
/// `aivt-provider-llm` never needs to depend on `aivt-provider-tools`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    /// Data URLs (`data:image/png;base64,...`) — empty for a text-only
    /// message. Only `OpenAiCompatibleProvider`-style vision-capable
    /// requests read this (sección 3 del brief, Fase 3: Multimodal).
    pub images: Vec<String>,
    /// Non-empty only on an assistant message that requested tool calls
    /// (Fase 10b) — echoed back verbatim when replaying history so the
    /// model sees its own prior call alongside the `Tool`-role result(s)
    /// that answered it.
    pub tool_calls: Vec<ToolCall>,
    /// Set only on a `MessageRole::Tool` message — which call this result
    /// answers.
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn text(role: MessageRole, content: impl Into<String>) -> Self {
        Self { role, content: content.into(), images: Vec::new(), tool_calls: Vec::new(), tool_call_id: None }
    }

    /// The assistant message that requested `tool_calls` — sent back as-is
    /// in the follow-up request, per the OpenAI tool-calling contract
    /// (Fase 10b).
    pub fn assistant_tool_calls(tool_calls: Vec<ToolCall>) -> Self {
        Self { role: MessageRole::Assistant, content: String::new(), images: Vec::new(), tool_calls, tool_call_id: None }
    }

    /// One tool's result, addressed back to the call it answers (Fase 10b).
    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Tool,
            content: content.into(),
            images: Vec::new(),
            tool_calls: Vec::new(),
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

/// Contexto opcional que el orquestador quiere darle al modelo pero que el
/// usuario puede no querer mandar a TODOS sus proveedores (ver
/// `PrivacyMode`). No viaja dentro de los mensajes: el `PrivacyGuardProvider`
/// que envuelve a cada proveedor decide cuáles se anexan al mensaje de sistema.
/// Un proveedor sin ese envoltorio simplemente los ignora — falla cerrado: lo
/// privado nunca sale por accidente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextBlock {
    /// Recuerdos relevantes ya redactados como sección del prompt: `all` con
    /// todos y `public_only` solo con los marcados como públicos (vacío si no
    /// hay ninguno). Se redactan los dos porque el encabezado y el idioma los
    /// pone el orquestador.
    Memories { all: String, public_only: String },
    /// Estado de ánimo actual del personaje, ya redactado.
    Mood(String),
}

/// Todos los mensajes `System` de la petición, en orden y separados por una línea
/// en blanco, o `None` si no hay texto. Anthropic y Gemini tienen un único campo de
/// sistema (no admiten mensajes `System` intercalados): quedarse solo con el primero
/// perdía el recordatorio de fecha/herramientas que el orquestador añade después del
/// historial.
pub(crate) fn joined_system_prompt(messages: &[ChatMessage]) -> Option<String> {
    let parts: Vec<&str> = messages.iter().filter(|m| m.role == MessageRole::System).map(|m| m.content.as_str()).filter(|c| !c.is_empty()).collect();
    (!parts.is_empty()).then(|| parts.join("

"))
}

#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    /// Empty means "no tools" — behaves exactly as before Fase 10b.
    pub tools: Vec<ToolDefinition>,
    /// Contexto que depende del modo de privacidad del proveedor que responda.
    pub context: Vec<ContextBlock>,
    /// `true` si los mensajes llevan recuerdos guardados del usuario (la
    /// fusión de memorias los manda todos). Un proveedor que no puede ver
    /// recuerdos rechaza la petición en vez de recibirlos.
    pub contains_stored_memories: bool,
}

/// One piece of a streamed response. `finished` is set on the final chunk
/// (which may or may not carry a trailing `delta`). `tool_calls` is only
/// ever non-empty on a finished chunk whose `finish_reason` is
/// `"tool_calls"` (Fase 10b) — a provider accumulates the streamed
/// fragments itself and hands back complete calls, so callers never see
/// partial function-call JSON.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatChunk {
    pub delta: String,
    pub finished: bool,
    pub finish_reason: Option<String>,
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
}

/// What a model can actually do — the UI reacts to this instead of
/// assuming (sección 84-85 del brief). Generic OpenAI-compatible servers
/// don't expose a capabilities endpoint, so this starts as a conservative
/// heuristic (text + streaming only) with manual override arriving in a
/// later fase — see ADR-003.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCapabilities {
    pub text: bool,
    pub vision: bool,
    pub audio_input: bool,
    pub audio_output: bool,
    pub tool_calling: bool,
    pub streaming: bool,
    pub embeddings: bool,
    pub image_generation: bool,
    pub json_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionStatus {
    pub connected: bool,
    pub latency_ms: Option<u64>,
    pub models: Vec<ModelInfo>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_system_message_is_joined_in_order_and_empty_ones_are_skipped() {
        let messages = vec![
            ChatMessage::text(MessageRole::System, "Eres Ciel"),
            ChatMessage::text(MessageRole::User, "hola"),
            ChatMessage::text(MessageRole::System, ""),
            ChatMessage::text(MessageRole::Assistant, "buenas"),
            ChatMessage::text(MessageRole::System, "Hoy es lunes"),
        ];
        assert_eq!(joined_system_prompt(&messages).as_deref(), Some("Eres Ciel

Hoy es lunes"));
    }

    #[test]
    fn a_single_system_message_is_unchanged_and_none_gives_none() {
        assert_eq!(joined_system_prompt(&[ChatMessage::text(MessageRole::System, "Eres Ciel")]).as_deref(), Some("Eres Ciel"));
        assert_eq!(joined_system_prompt(&[ChatMessage::text(MessageRole::User, "hola")]), None);
        assert_eq!(joined_system_prompt(&[]), None);
    }
}
