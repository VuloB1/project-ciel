use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::stream::{self, BoxStream};
use futures::StreamExt;

use aivt_provider_llm::{
    ChatChunk, ChatRequest, ConnectionStatus, LLMProvider, ModelCapabilities, ModelInfo, ProviderError,
};

/// Configurable fake `LLMProvider`: streams back a fixed sequence of chunks
/// (or a canned error) and records every request it receives, so tests can
/// assert on exactly what was sent — including the composed system prompt.
///
/// `responses` holds one response-set *per call* to `stream_chat` — needed
/// to test `AIOrchestrator`'s tool-calling round trip (Fase 10b), where a
/// single `stream_respond` invocation can call the provider more than once
/// (first call asks for a tool, second call answers using its result). A
/// mock built via `with_response`/`with_error` just has one response-set
/// and returns it for every call, matching the old single-shot behavior.
/// Calling `stream_chat` more times than there are response-sets repeats
/// the last one rather than panicking, so a test doesn't need to predict
/// an exact call count it isn't actually asserting on.
pub struct MockLLMProvider {
    responses: Vec<Vec<Result<ChatChunk, ProviderError>>>,
    received: Arc<Mutex<Vec<ChatRequest>>>,
    call_count: Arc<Mutex<usize>>,
}

impl MockLLMProvider {
    /// Streams back a single assistant message split into `pieces`,
    /// followed by a `finished` chunk — mirrors how a real provider
    /// delivers a response token-by-token.
    pub fn with_response(pieces: impl IntoIterator<Item = &'static str>) -> Self {
        Self::with_call_sequence([text_response(pieces)])
    }

    pub fn with_error(error: ProviderError) -> Self {
        Self::with_call_sequence([vec![Err(error)]])
    }

    /// One response-set per expected call to `stream_chat`, in order —
    /// e.g. `[tool_call_response(...), text_response(["18°C"])]` for "the
    /// model asks for a tool, then answers using its result."
    pub fn with_call_sequence(responses: impl IntoIterator<Item = Vec<Result<ChatChunk, ProviderError>>>) -> Self {
        Self { responses: responses.into_iter().collect(), received: Arc::new(Mutex::new(Vec::new())), call_count: Arc::new(Mutex::new(0)) }
    }

    pub fn received_requests(&self) -> Vec<ChatRequest> {
        self.received.lock().expect("mock mutex poisoned").clone()
    }
}

/// Builds a normal (non-tool-calling) response-set — the same shape
/// `with_response` has always produced, exposed standalone so a
/// `with_call_sequence` mock can mix this with `tool_call_response` across
/// calls.
pub fn text_response(pieces: impl IntoIterator<Item = &'static str>) -> Vec<Result<ChatChunk, ProviderError>> {
    let mut chunks: Vec<Result<ChatChunk, ProviderError>> = pieces
        .into_iter()
        .map(|piece| Ok(ChatChunk { delta: piece.to_string(), finished: false, finish_reason: None, tool_calls: Vec::new() }))
        .collect();
    chunks.push(Ok(ChatChunk {
        delta: String::new(),
        finished: true,
        finish_reason: Some("stop".to_string()),
        tool_calls: Vec::new(),
    }));
    chunks
}

/// Builds a response-set where the model asks to call `tool_calls` instead
/// of answering directly — mirrors what a real provider's finished chunk
/// looks like once `openai_compatible.rs` finishes accumulating a
/// streamed tool call (Fase 10b).
pub fn tool_call_response(tool_calls: Vec<aivt_provider_llm::ToolCall>) -> Vec<Result<ChatChunk, ProviderError>> {
    vec![Ok(ChatChunk { delta: String::new(), finished: true, finish_reason: Some("tool_calls".to_string()), tool_calls })]
}

#[async_trait]
impl LLMProvider for MockLLMProvider {
    async fn test_connection(&self) -> Result<ConnectionStatus, ProviderError> {
        Ok(ConnectionStatus {
            connected: true,
            latency_ms: Some(0),
            models: vec![ModelInfo { id: "mock-model".to_string() }],
        })
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(vec![ModelInfo { id: "mock-model".to_string() }])
    }

    fn capabilities(&self, _model: &str) -> ModelCapabilities {
        ModelCapabilities { text: true, streaming: true, ..Default::default() }
    }

    fn stream_chat(&self, request: ChatRequest) -> BoxStream<'static, Result<ChatChunk, ProviderError>> {
        self.received.lock().expect("mock mutex poisoned").push(request);

        let index = {
            let mut count = self.call_count.lock().expect("mock mutex poisoned");
            let index = (*count).min(self.responses.len().saturating_sub(1));
            *count += 1;
            index
        };
        let chunks = self.responses.get(index).cloned().unwrap_or_default();

        // `.then()` forces a real yield point between items (via
        // `tokio::task::yield_now`), the same way a real provider's network
        // I/O would — without this, `stream::iter` resolves every item
        // synchronously and a caller racing it against cancellation
        // (`tokio::select!`) could never actually observe the cancellation
        // winning.
        Box::pin(stream::iter(chunks).then(|item| async move {
            tokio::task::yield_now().await;
            item
        }))
    }
}
