use serde::Serialize;

/// Lifecycle states a background integration (a `ServiceManager` entry) can
/// be in. Mirrors section 80 of the product brief.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceStatus {
    Stopped,
    Starting,
    Running,
    Error,
    Stopping,
}

/// What's happening *inside* an active call (Fase 6) — kept separate from
/// `ServiceStatus` (mic open/closed lifecycle, shared by every
/// `ManagedService`) rather than widening that generic type with phases
/// only Call has. One call session moves through these on its own, over
/// and over, for as long as `ServiceStatus::Running` holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CallState {
    /// Mic open, nothing detected yet.
    Idle,
    /// Voice detected, buffering the utterance.
    Listening,
    Transcribing,
    Thinking,
    Speaking,
    Error,
}

/// Everything that happens inside AIVT Companion that some other part of the
/// app might care about, without producer and consumer knowing about each
/// other. New variants are added one at a time, only once a real producer
/// exists for them (Chat/TTS/STT/Memory/Twitch/Discord/Call/Avatar events
/// arrive with their own phases) — see docs/architecture.md.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum AppEvent {
    ServiceStatusChanged {
        service: String,
        status: ServiceStatus,
    },
    /// Emitted on a fixed interval while the app runs. Exists purely to
    /// prove the EventBus -> Tauri `emit` -> frontend bridge end-to-end
    /// before anything real produces events.
    Heartbeat {
        timestamp: String,
    },
    /// A generation started — `message_id` is the (initially empty)
    /// assistant message row it's filling in.
    AiResponseStarted {
        request_id: String,
        profile_id: String,
        conversation_id: String,
        message_id: String,
    },
    AiResponseToken {
        request_id: String,
        token: String,
    },
    AiResponseFinished {
        request_id: String,
    },
    AiResponseCancelled {
        request_id: String,
    },
    AiResponseError {
        request_id: String,
        message: String,
    },
    /// One or more memories were created for `profile_id` outside of
    /// whatever's currently rendering them — automatic consolidation runs
    /// in a detached background task the Memoria tab has no other way to
    /// find out about (Fase 4). Also fired after the manual "extract now"
    /// trigger, so a Memoria tab open in another window/state stays in
    /// sync too. Carries no memory data itself — just a "go refetch" nudge.
    MemoriesUpdated {
        profile_id: String,
    },
    /// Texto transcrito hasta ahora de un dictado en curso (botón de
    /// micrófono del chat) — se reemplaza entero en cada evento, no se
    /// concatena. Es provisional: la pasada final al detener la grabación
    /// es la que vale.
    DictationPartial {
        text: String,
    },
    /// A TTS engine (native or HTTP — see `aivt_provider_tts`) started
    /// speaking. `message_id` is `None` for anything spoken outside a chat
    /// message (e.g. a "probar voz" preview from the Voz tab).
    VoicePlaybackStarted {
        profile_id: String,
        message_id: Option<String>,
    },
    /// Speech ended — either it finished naturally or was interrupted by a
    /// `stop`/a newer `speak` call. The frontend only needs "not speaking
    /// anymore", not which — see `aivt_provider_tts::TTSProvider::stop`.
    VoicePlaybackFinished {
        profile_id: String,
        message_id: Option<String>,
    },
    VoicePlaybackError {
        profile_id: String,
        message_id: Option<String>,
        message: String,
    },
    /// A message arrived in the connected Twitch channel's chat — shown in
    /// the Twitch tab's live feed regardless of whether it qualified for a
    /// reply (the whole point of a chat view).
    TwitchMessageReceived {
        chatter_display_name: String,
        text: String,
    },
    /// The character actually posted a reply back to Twitch chat.
    /// `character_name` is the active profile's own name at the moment it
    /// replied — the Twitch tab's feed shows this instead of a generic
    /// "Personaje" label, and instead of the account's Twitch login,
    /// since the two can differ once a dedicated bot account is in use.
    TwitchMessageSent {
        character_name: String,
        text: String,
    },
    /// Describing a channel's 7TV/BTTV/FFZ emotes by vision (Configuración
    /// del canal de Twitch, "Generar descripciones") is in progress — one
    /// event per emote (time-throttled, same shape as
    /// `EmbeddingBackfillProgress`), always including a final 100%-complete
    /// sample right before `TwitchEmoteCaptionFinished`.
    TwitchEmoteCaptionProgress {
        broadcaster_id: String,
        captioned: usize,
        total: usize,
    },
    TwitchEmoteCaptionFinished {
        broadcaster_id: String,
        captioned: usize,
        total: usize,
    },
    /// Fired instead of `Finished` when the vision connection itself fails
    /// (assumed broken for every remaining emote, not just this one — see
    /// `aivt_service_twitch::emotes::caption`'s doc comment) or the cache
    /// couldn't be read at all. A single emote's image failing to download
    /// is tolerated individually and only logged, not surfaced here.
    TwitchEmoteCaptionError {
        broadcaster_id: String,
        message: String,
    },
    /// A message arrived in a Discord server/channel the bot can see —
    /// shown in the Discord tab's live feed regardless of whether it
    /// qualified for a reply, mirroring `TwitchMessageReceived`.
    DiscordMessageReceived {
        author_display_name: String,
        text: String,
    },
    /// The character actually posted a reply back to Discord. See
    /// `TwitchMessageSent::character_name`.
    DiscordMessageSent {
        character_name: String,
        text: String,
    },
    /// The active call session moved to a new phase — see `CallState`.
    CallStateChanged {
        state: CallState,
    },
    /// The user's utterance finished transcribing — shown in the Llamada
    /// tab's transcript, mirrors `TwitchMessageReceived`'s role for a chat
    /// feed. Published even if the transcript ended up empty (e.g. noise
    /// that tripped the VAD), so the tab's feed reflects reality.
    CallUserUtterance {
        text: String,
    },
    /// The character's reply — published per sentence-chunk once streaming
    /// TTS is wired in (Milestone 4), one full-text event per turn until
    /// then.
    CallCharacterReply {
        text: String,
    },
    /// Volumen actual del micrófono (0 a 1, escala logarítmica), unas 15
    /// veces por segundo mientras hay una llamada o una prueba de micrófono
    /// abierta. Solo alimenta la barra de nivel de la interfaz.
    CallMicLevel {
        level: f32,
    },
    /// A turn failed (transcription or generation) — `CallStateChanged
    /// { state: Error }` fires alongside this, but that's just a badge
    /// blip the orchestrator resets to `Idle` right after; this is what
    /// actually explains *why* to the user, since the underlying failure
    /// otherwise only ever reached `tracing::warn!` (see docs/DEVELOPMENT_STATUS.md's
    /// Fase 6 section — a live-testing checkpoint hit exactly this: a
    /// transcription 404 that was completely invisible outside the log).
    CallError {
        message: String,
    },
    /// A Whisper GGML model download is in progress — Configuración >
    /// Transcripción de voz's "Descargar" button switches to a progress
    /// bar on this. Emitted on a time-based throttle (not per network
    /// chunk — a 1.5GB "medium" model at typical chunk sizes would
    /// otherwise spam thousands of events), always including a final
    /// 100%-complete sample right before `SttModelDownloadFinished`.
    /// `total_bytes` is `None` if the server didn't send a
    /// `Content-Length` header.
    SttModelDownloadProgress {
        model_size: String,
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    },
    SttModelDownloadFinished {
        model_size: String,
    },
    SttModelDownloadError {
        model_size: String,
        message: String,
    },
    /// The local semantic-search embedding model download is in progress —
    /// Configuración > Memoria's "Descargar" button switches to a progress
    /// bar on this. Same time-throttled shape as `SttModelDownloadProgress`,
    /// but for the one `model.safetensors` file that actually takes a
    /// while (`config.json`/`tokenizer.json` download first, synchronously,
    /// with no events of their own — see `aivt_provider_embedding::EmbeddingModelManager`).
    EmbeddingModelDownloadProgress {
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    },
    EmbeddingModelDownloadFinished,
    EmbeddingModelDownloadError {
        message: String,
    },
    /// The optional Obscura download (the headless browser behind `read_url`) is in progress. Same time-throttled
    /// shape as `SttModelDownloadProgress`.
    ObscuraDownloadProgress {
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    },
    ObscuraDownloadFinished,
    ObscuraDownloadError {
        message: String,
    },
    /// A profile's memory backfill (Configuración > Memoria's "Rellenar
    /// memorias antiguas") is in progress — computes embeddings for
    /// memories that predate the model being downloaded. Same
    /// time-throttled shape as the download progress events above.
    EmbeddingBackfillProgress {
        profile_id: String,
        embedded: usize,
        total: usize,
    },
    EmbeddingBackfillFinished {
        profile_id: String,
        embedded: usize,
        total: usize,
    },
    /// Fired instead of `Finished` only when the model isn't downloaded —
    /// every other per-item embed failure is logged, not surfaced here,
    /// same as the rest of this app's "best-effort background work"
    /// stance.
    EmbeddingBackfillError {
        profile_id: String,
        message: String,
    },
    /// The character's evolving emotional state (`docs/ADR/006-mood.md`)
    /// changed — fired after `MemoryManager::extract_and_store` applies a
    /// delta (or pure time-based decay) via `MoodRepository::apply_delta`.
    /// Carries the new absolute values, not just the delta, so the
    /// frontend can render directly without an extra round trip.
    MoodUpdated {
        profile_id: String,
        valence: i64,
        energy: i64,
    },
    /// El personaje inició una conversación por su cuenta — ver
    /// docs/ADR/007-proactive-mode.md. El contenido del mensaje en sí
    /// llega por los mismos `AiResponseStarted`/`Token`/`Finished` que una
    /// respuesta normal; este evento solo existe para que el frontend
    /// pueda marcar "no leído" aunque el usuario esté en otra pestaña.
    ProactiveMessageSent {
        profile_id: String,
        conversation_id: String,
        /// El texto del mensaje — pedido del usuario para poder mostrar un
        /// aviso de escritorio con una vista previa (`notifyProactiveMessage`,
        /// frontend), en vez de solo el puntito de "no leído" del Sidebar.
        content: String,
    },
    /// Chat's optional "guardar audio de los mensajes" persistence
    /// (Configuración > Privacidad) finished writing a message's spoken
    /// reply to disk — lets the message list pick up the new attachment
    /// without polling or re-fetching the whole conversation. Fired only
    /// on success; a save failure is just logged (see
    /// `aivt_service_chat::save_audio_attachment`), never surfaced here,
    /// matching auto-play's own "best-effort, never blocks the reply"
    /// stance. Without this event the frontend's client-side message
    /// object (built purely from `AiResponse*` events, never re-fetched)
    /// would never learn the attachment exists — every "escuchar" click
    /// would keep re-synthesizing via the TTS backend instead of replaying
    /// the stored clip, silently burning API credits (ElevenLabs, etc.).
    ChatAudioSaved {
        conversation_id: String,
        message_id: String,
        attachment_id: String,
        mime_type: String,
    },

    /// `FailoverLLMProvider` (`aivt-provider-llm`) pasó de un proveedor de
    /// IA a otro dentro de la misma generación, porque el que estaba "en
    /// uso" se quedó sin crédito (`ProviderError::QuotaExceeded`) — ver
    /// `SettingsService::build_llm_provider`. El frontend escucha esto para
    /// refrescar sola la lista de Proveedores IA (el badge "En uso") sin
    /// que el usuario tenga que volver a esa pestaña o refrescar a mano.
    LlmProviderSwitched {
        from_provider_id: String,
        to_provider_id: String,
        /// Por ahora siempre `"quota_exceeded"` — string en vez de un enum
        /// para no acoplar `aivt-eventbus` a los detalles de
        /// `aivt-provider-llm` por una sola razón posible hoy.
        reason: String,
    },

    /// El usuario abrió un `.aivtprofile` desde el sistema operativo (doble
    /// clic con la app ya corriendo, o arranque en frío con el archivo como
    /// argumento) — el frontend escucha esto para mostrar el diálogo de
    /// confirmación antes de importar, nunca importa directo sin preguntar.
    /// `path` es la ruta absoluta al archivo tal cual llegó por `argv`.
    ProfileImportRequested {
        path: String,
    },
}
