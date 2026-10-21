use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use aivt_eventbus::EventBus;
use aivt_memory::{MemoryManager, PeopleManager};
use aivt_provider_embedding::EmbeddingModelManager;
use aivt_provider_stt::SttModelManager;
use aivt_provider_tools::ObscuraManager;
use aivt_provider_tts::TTSProvider;
use aivt_service_activity::ActivityService;
use aivt_service_avatar::AvatarServerService;
use aivt_service_call::CallService;
use aivt_service_chat::ChatService;
use aivt_service_discord::DiscordService;
use aivt_service_manager::ServiceManager;
use aivt_service_proactive::ProactiveService;
use aivt_service_profiles::ProfileService;
use aivt_service_settings::SettingsService;
use aivt_service_twitch::TwitchService;

/// Everything Tauri commands need, constructed once in `run()::setup` and
/// handed to the webview via `app.manage(...)`.
///
/// `_log_guard` is not read directly by any command (file logging just
/// needs to keep running) — the leading underscore tells the `dead_code`
/// lint that's intentional rather than a bug. `event_bus` used to be
/// `_event_bus` for the same reason (every service already holds its own
/// clone) until the `.aivtprofile` doble-clic feature needed to publish
/// `AppEvent::ProfileImportRequested` from the single-instance callback in
/// `run()`, which only has `&AppHandle` — `app.state::<AppState>().event_bus`
/// is how it reaches one.
pub struct AppState {
    /// Pool compartido de la base — lo usa `commands::backup` para la copia de seguridad.
    pub pool: aivt_database::DbPool,
    /// Carpeta de datos de la app (base, adjuntos, avatares...) — origen y destino de las copias.
    pub data_dir: std::path::PathBuf,
    pub settings: SettingsService,
    pub profiles: ProfileService,
    pub chat: ChatService,
    /// Same instance `ChatService` uses internally for retrieval/automatic
    /// consolidation — shared so the Memoria tab's CRUD (list/search/pin/
    /// delete/manual create) and chat's background extraction always see
    /// the same data.
    pub memory: Arc<dyn MemoryManager>,
    /// Registro de personas — shared with `twitch` (auto-registers
    /// chatters) and read directly by `call` (Whisper prompt priming) and
    /// the `commands::people` Tauri commands. See `aivt_memory::people`.
    pub people: Arc<dyn PeopleManager>,
    /// One long-lived instance for the whole app (native voice
    /// initialization and the dedicated audio-output thread only happen
    /// once) — which backend a given `speak` call actually uses is decided
    /// per-request from current Settings, not by swapping this out. See
    /// `aivt_provider_tts::AivtTtsProvider`.
    pub tts: Arc<dyn TTSProvider>,
    /// Owns the downloaded GGML model files + the one cached, loaded
    /// `WhisperContext` — see `aivt_provider_stt::SttModelManager`. Shared
    /// with `call` (constructed from the same instance) and read directly
    /// by the `commands::stt` Tauri commands for model download/selection.
    pub stt_models: SttModelManager,
    /// Optional headless browser behind the `read_url` tool (downloaded on demand) — see `aivt_provider_tools::ObscuraManager`.
    pub obscura: ObscuraManager,
    /// Owns the downloaded local embedding-model files + the one cached
    /// loaded model, for semantic memory search — see
    /// `aivt_provider_embedding::EmbeddingModelManager`. Read directly by
    /// the `commands::embedding` Tauri commands (download/delete/status);
    /// embedding *computation* goes through `memory`'s
    /// `LocalEmbeddingProvider`, built from a clone of this same manager
    /// at startup.
    pub embedding_models: EmbeddingModelManager,
    pub twitch: TwitchService,
    pub discord: DiscordService,
    pub call: CallService,
    /// Motor de "modo proactivo" — ver docs/ADR/007-proactive-mode.md. A
    /// diferencia de twitch/discord/call, arranca solo al iniciar la app
    /// (no espera una conexión explícita del usuario).
    pub proactive: ProactiveService,
    /// Servidor HTTP/WebSocket local que expone el avatar reactivo como
    /// fuente de navegador de OBS — ver `aivt_service_avatar`. Opt-in
    /// (arranca solo si `avatar_server_settings().enabled`), pero una vez
    /// activado se auto-reanuda en cada lanzamiento (ver `run()::setup`).
    /// Ningún comando lo toca directamente (todo pasa por `settings` +
    /// `service_manager.start/stop("avatar_server")`, igual que
    /// twitch/discord/call) — el guion bajo evita la advertencia de
    /// `dead_code`, mismo criterio que `_event_bus`/`_log_guard` arriba.
    pub _avatar_server: AvatarServerService,
    /// Registry for `twitch`/`discord`/`call` lifecycle — see
    /// docs/architecture.md §9.
    pub service_manager: Arc<ServiceManager>,
    pub event_bus: EventBus,
    /// Ruta de un `.aivtprofile` con el que se lanzó la app en frío (doble
    /// clic con la app cerrada) — `None` en el arranque normal. El
    /// frontend lo consume una sola vez al montar (`profiles_take_pending_import_path`,
    /// que lo deja en `None` después de leerlo) en vez de un `AppEvent`:
    /// publicar un evento acá arriesgaría perderlo si el frontend todavía
    /// no llegó a suscribirse (`App.tsx` recién monta después de que el
    /// webview termina de cargar). El caso de la app YA corriendo (segunda
    /// instancia) sí usa el evento — en ese caso el frontend ya está
    /// escuchando hace rato.
    pub pending_import_path: Mutex<Option<String>>,
    /// Backs the "Actividad" tab (Fase 11) — `commands::activity` reads
    /// recent events from this via `ActivityService::recent`.
    pub activity: ActivityService,
    /// Live flag the main window's `on_window_event` (see `run()::setup`)
    /// checks on every `WindowEvent::CloseRequested` to decide whether to
    /// hide to the system tray instead of actually closing. Seeded at
    /// startup from `SettingsService::background_settings` and kept in
    /// sync by `commands::settings::settings_set_background` — a plain
    /// `AtomicBool` (not an async settings read) because window-event
    /// callbacks are synchronous.
    pub minimize_to_tray_enabled: Arc<AtomicBool>,
    pub _log_guard: tracing_appender::non_blocking::WorkerGuard,
}
