//! `PromptComposer` + `PersonalityPromptBuilder`: turn a profile's identity,
//! lore, relationship and personality sliders into the natural-language
//! instructions an LLM actually receives — never raw numbers (sección 11
//! del brief).
//!
//! `AIOrchestrator` is the one place "the character thinks" — every channel
//! (Chat now; Call/Twitch/Discord in their fases) funnels through it
//! instead of talking to an `LLMProvider` directly (sección 73 del brief:
//! Principio de Identidad Única). Memory retrieval and channel-specific
//! context (Fase 4+) fold into `stream_respond` later without changing
//! this contract.

mod companion;
#[cfg(test)]
mod lab;
mod mood;
mod orchestrator;
mod personality;
mod prompt;

pub use companion::Channel;
pub use mood::MoodPromptBuilder;
pub use orchestrator::AIOrchestrator;
pub use personality::PersonalityPromptBuilder;
pub use prompt::PromptComposer;
