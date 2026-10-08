//! Typed view over `app_settings`, backing Configuración > General (and,
//! eventually, every other Configuración subtab — see docs/architecture.md).
//!
//! `SettingsService` is one struct, but its methods live next to the types
//! they work on, one module per area (`llm`, `tts`, `twitch`, ...): each file
//! adds its own `impl SettingsService` block. The public API is unchanged.

mod call;
mod discord;
mod error;
mod llm;
mod preferences;
mod service;
mod stt;
mod tts;
mod twitch;
mod usage;
mod web_search;

pub use call::{CallPolicy, VadMode};
pub use discord::{DiscordConnectionSettings, DiscordPolicy};
pub use error::SettingsError;
pub use llm::{
    AddLlmProviderInput, LlmConnectionSettings, LlmProviderEntry, LlmProviderKind, LlmProvidersSettings, UpdateLlmProviderInput,
};
pub use preferences::{
    AppearanceSettings, AvatarServerSettings, BackgroundSettings, ChatAudioSettings, ColorMode, DictationSettings, MessageDensity,
    PeoplePolicy,
};
pub use service::{SettingsService, DEFAULT_LANGUAGE};
pub use stt::{SttModelInput, SttModelSettings, SttModelSize};
pub use tts::{AddTtsProviderInput, TtsBackendKind, TtsProviderEntry, TtsProvidersSettings, UpdateTtsProviderInput};
pub use twitch::{TwitchConnectionSettings, TwitchEmoteSettings, TwitchPolicy};
pub use usage::{LlmUsageEntry, UsagePeriod};
pub use web_search::{WebSearchInput, WebSearchSettings};
