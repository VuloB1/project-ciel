use async_trait::async_trait;

use crate::error::ToolError;

/// One function the character's LLM can call mid-response — `AIOrchestrator`
/// (crate `aivt-orchestrator`) drives the round trip (advertise these to the
/// provider, execute one when requested, feed the result back), this trait
/// only describes and executes a single call. See
/// docs/DEVELOPMENT_STATUS.md's Fase 10b section for the full design.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Sent to the model as the function name — must be stable, since it's
    /// also how `AIOrchestrator` looks up which `Tool` to run when the model
    /// requests a call by name.
    fn name(&self) -> &str;

    /// Sent to the model so it knows when to call this — the only real
    /// "prompt engineering" surface here, since the model decides on its
    /// own when a tool is worth using.
    fn description(&self) -> &str;

    /// JSON Schema for the arguments object the model must produce when
    /// calling this tool (the OpenAI tool-calling wire format expects this
    /// shape directly under `function.parameters`).
    fn parameters_schema(&self) -> serde_json::Value;

    /// `arguments_json` is exactly what the model produced (validated
    /// against `parameters_schema` by the model/server, not re-validated
    /// here beyond what parsing it requires) — returns plain text, sent
    /// back to the model as the tool's result message.
    async fn execute(&self, arguments_json: &str) -> Result<String, ToolError>;
}
