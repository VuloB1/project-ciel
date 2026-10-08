//! `ChatService`: conversations/messages persistence + streaming, delegating
//! to `AIOrchestrator` for actual responses (sección 13-14 del brief).
//! Generation runs in a background task and is observed by the frontend via
//! `AppEvent`s (`AiResponseStarted/Token/Finished/Error/Cancelled`), never
//! by blocking a Tauri command on the full response.

mod error;
mod service;
mod title;

pub use error::ChatError;
pub use service::{ChatService, GenerationContext, MessageView, NewAttachmentInput, TtsInvocation};
