//! `STTProvider` trait + its embedded whisper.cpp implementation — the
//! transcription half of Fase 6's Llamada pipeline (`AudioIn -> VAD -> STT
//! -> AIOrchestrator -> TTS`).
//!
//! Runs entirely in-process via `whisper-rs` (bindings to whisper.cpp),
//! compiled **without any GPU feature** (CPU-only) — this is what lets the
//! sold product work identically on any customer's machine regardless of
//! their GPU vendor, with zero external installs (no Docker, no Python, no
//! server to run). `SttModelManager` owns downloading the GGML model file
//! on first use and caching the loaded `WhisperContext`; `WhisperCppProvider`
//! is the cheap per-call wrapper around an already-loaded context.

mod error;
mod model;
mod model_manager;
mod types;
mod whisper_provider;

use async_trait::async_trait;

pub use error::SttError;
pub use model::WhisperModelSize;
pub use model_manager::{ModelDownloadStatus, SttModelManager};
pub use types::TranscribeRequest;
pub use whisper_provider::WhisperCppProvider;

#[async_trait]
pub trait STTProvider: Send + Sync {
    /// Returns the transcribed text, trimmed. An empty result (silence, or
    /// a clip too short for the model to make sense of) is not itself an
    /// error — the caller (`CallService`) decides what an empty transcript
    /// means, same split as `TwitchError::EmptyResponse` living at the
    /// service level rather than the provider's.
    async fn transcribe(&self, request: TranscribeRequest) -> Result<String, SttError>;
}
