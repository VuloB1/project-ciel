//! `MemoryManager`: retrieval (FTS5 search, used both by the Memoria tab and
//! by the orchestrator to pull context relevant to the current message) and
//! extraction (an LLM call that turns a slice of conversation into
//! candidate memories — see `extraction.rs`) for Fase 4 (Memoria).
//!
//! Deliberately reuses the same `LLMProvider`/`stream_chat` the chat
//! feature already talks to, rather than a separate summarization backend
//! — no new external dependency, and it's the connection the user already
//! has configured and tested (see docs/DEVELOPMENT_STATUS.md, Fase 4,
//! "Decisiones de alcance").

mod error;
mod extraction;
mod fusion;
mod people;
mod service;

pub use error::MemoryError;
pub use fusion::{FusionGroupInput, FusionProposal};
pub use people::{PeopleManager, PeopleService};
pub use service::{MemoryManager, MemoryService};
