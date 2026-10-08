//! `LLMProvider`: the trait `AIOrchestrator` (crate `orchestrator`) talks to
//! for chat completions, plus its first real implementation,
//! `OpenAiCompatibleProvider` — LM Studio is just this provider pointed at
//! `http://localhost:1234/v1`. See docs/ADR/003-provider-architecture.md.

mod anthropic;
mod error;
mod failover;
mod gemini;
mod openai_compatible;
mod privacy;
mod sse;
mod types;

use async_trait::async_trait;
use futures::stream::BoxStream;

pub use anthropic::AnthropicProvider;
pub use error::{FailoverReason, ProviderError};
pub use failover::{FailoverLLMProvider, FailoverReport, LlmPoolEntry};
pub use gemini::GeminiProvider;
pub use openai_compatible::OpenAiCompatibleProvider;
pub use privacy::{PrivacyGuardProvider, PrivacyMode};
pub use types::{
    ChatChunk, ChatMessage, ChatRequest, ConnectionStatus, ContextBlock, MessageRole, ModelCapabilities, ModelInfo, ToolCall,
    ToolDefinition,
};

#[async_trait]
pub trait LLMProvider: Send + Sync {
    async fn test_connection(&self) -> Result<ConnectionStatus, ProviderError>;
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError>;

    /// Never assume — the UI reacts to this instead of hardcoding what a
    /// model can do (sección 84-85 del brief).
    fn capabilities(&self, model: &str) -> ModelCapabilities;

    fn stream_chat(&self, request: ChatRequest) -> BoxStream<'static, Result<ChatChunk, ProviderError>>;
}
