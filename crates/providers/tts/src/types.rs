use serde::{Deserialize, Serialize};

/// One installed voice, as reported by the active backend. The native
/// backend enumerates these via `SpeechSynthesizer::AllVoices` on Windows;
/// ElevenLabs, Google and Azure each have their own real "list voices"
/// endpoint — only the generic `Http`/`OpenAi` backend has no standard one
/// at all, so `TTSProvider::list_voices` returns an empty list for it and
/// the Voz tab falls back to a free-text voice id field only in that case.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceInfo {
    pub id: String,
    pub name: String,
    pub language: Option<String>,
}

/// Which engine `speak`/`stop`/`list_voices` should use for *this call* —
/// carried per-request rather than fixed at construction time, so a single
/// long-lived provider instance can serve whichever backend is active in
/// Configuración > AI Providers right now without being rebuilt.
///
/// `Debug` is hand-written, not derived (see below) — the derived version
/// would print `api_key` in plain text, and this type does get logged
/// (`crates/services/chat/src/service.rs`'s `auto-play: dispatching speak
/// request` uses `backend = ?tts.backend`) — a live-testing session
/// actually caught this leaking a real ElevenLabs key into the log file on
/// disk. Same secrets-handling principle as `aivt_service_settings`
/// exposing only `has_api_key: bool` to callers, never the raw value.
#[derive(Clone)]
pub enum TtsBackend {
    Native,
    /// An OpenAI-compatible `/audio/speech` endpoint — the same contract
    /// OpenAI's own TTS API and several self-hosted servers (Piper/XTTS
    /// wrappers, etc.) implement, mirroring how `OpenAiCompatibleProvider`
    /// already treats LM Studio as "just this shape, pointed locally."
    Http { base_url: String, api_key: Option<String> },
    /// ElevenLabs' own API directly — a fixed endpoint, no `base_url` to
    /// configure, just an API key. Kept as its own variant (not folded
    /// into `Http`) since ElevenLabs' request/response shape and its
    /// real `list_voices` support are both genuinely different contracts.
    ElevenLabs { api_key: String },
    /// Google Cloud Text-to-Speech — fixed endpoint (`GoogleEngine::API_BASE`),
    /// API key only (no OAuth/service-account flow).
    Google { api_key: String },
    /// Azure Speech (Cognitive Services) — needs `region` in addition to
    /// the key since its endpoint is `https://{region}.tts.speech.microsoft.com`,
    /// unlike every other backend here.
    Azure { api_key: String, region: String },
    /// Fish Audio — fixed endpoint, API key only; `voice_id` is the id of a
    /// voice (free text, there is no list endpoint). `model` is sent as the
    /// `model` header; `None` means the free `s2.1-pro-free`.
    FishAudio { api_key: String, model: Option<String> },
    /// Deepgram Aura — fixed endpoint, API key only; `voice_id` is the Aura
    /// model name (free text).
    Deepgram { api_key: String },
    /// Groq (Orpheus) — `/audio/speech` shape against Groq's fixed URL; `model`
    /// `None` means its English Orpheus model.
    Groq { api_key: String, model: Option<String> },
    /// Together AI — same shape against Together's fixed URL; `model` `None`
    /// means the Orpheus 3B model from their quickstart.
    Together { api_key: String, model: Option<String> },
}

impl std::fmt::Debug for TtsBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Native => write!(f, "Native"),
            Self::Http { base_url, api_key } => f
                .debug_struct("Http")
                .field("base_url", base_url)
                .field("api_key", &api_key.as_ref().map(|_| "<redacted>"))
                .finish(),
            Self::ElevenLabs { .. } => f.debug_struct("ElevenLabs").field("api_key", &"<redacted>").finish(),
            Self::Google { .. } => f.debug_struct("Google").field("api_key", &"<redacted>").finish(),
            Self::FishAudio { .. } => f.debug_struct("FishAudio").field("api_key", &"<redacted>").finish(),
            Self::Groq { model, .. } => f.debug_struct("Groq").field("api_key", &"<redacted>").field("model", model).finish(),
            Self::Together { model, .. } => f.debug_struct("Together").field("api_key", &"<redacted>").field("model", model).finish(),
            Self::Deepgram { .. } => f.debug_struct("Deepgram").field("api_key", &"<redacted>").finish(),
            Self::Azure { region, .. } => f.debug_struct("Azure").field("api_key", &"<redacted>").field("region", region).finish(),
        }
    }
}

/// Everything one `speak` call needs: what to say, how to say it, and where
/// to route it. `profile_id`/`message_id` never reach the synthesis engine
/// itself — they ride along purely so the emitted `AppEvent::VoicePlayback*`
/// events can tell the frontend *which* message is currently speaking.
#[derive(Debug, Clone)]
pub struct SpeakRequest {
    pub text: String,
    pub voice_id: String,
    pub rate: f32,
    pub pitch: f32,
    pub volume: f32,
    pub backend: TtsBackend,
    pub profile_id: String,
    pub message_id: Option<String>,
}
