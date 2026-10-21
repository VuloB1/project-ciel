//! `Tool`: one function the character's LLM can call mid-response — Fase
//! 10b. Two implementations so far: `BraveSearchTool` (general web search)
//! and `CurrentTimeTool` (direct timezone calculation, no network — added
//! after live testing showed web search has no reliable answer for "what
//! time is it in X", only pages *about* the topic). `AIOrchestrator`
//! (crate `aivt-orchestrator`) drives the actual tool-calling round trip;
//! this crate only describes and executes individual tools, matching how
//! `aivt-provider-llm`/`-stt`/`-tts` are each a trait + concrete
//! implementations (see docs/ADR/003-provider-architecture.md).

mod brave;
mod current_time;
mod error;
mod obscura;
mod read_url;
mod tool;

pub use brave::BraveSearchTool;
pub use current_time::CurrentTimeTool;
pub use error::ToolError;
pub use obscura::{ObscuraManager, APPROX_SIZE_MB as OBSCURA_APPROX_SIZE_MB};
pub use read_url::ReadUrlTool;
pub use tool::Tool;
