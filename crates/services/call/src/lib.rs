//! `CallService`: mic capture (`cpal`) → VAD utterance segmentation
//! (`webrtc-vad`) → STT (`aivt-provider-stt`) → `AIOrchestrator` → TTS
//! (`aivt-provider-tts`), with barge-in and per-stage latency diagnostics
//! landing in later milestones — see docs/call.md and
//! docs/DEVELOPMENT_STATUS.md's Fase 6 section for the full design and
//! build order.

mod audio_guard;
mod capture;
mod error;
mod history;
mod segmenter;
mod service;
mod speech_chunker;
mod speech_pipeline;

pub use capture::list_input_devices;
pub use error::CallError;
pub use service::CallService;
