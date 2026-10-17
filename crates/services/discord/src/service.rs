use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

use aivt_database::{DbPool, Message, MessageRole, MessageStatus, ProfilesRepository};
use aivt_eventbus::{AppEvent, EventBus, ServiceStatus};
use aivt_memory::{MemoryManager, PeopleManager};
use aivt_orchestrator::{AIOrchestrator, Channel};
use aivt_provider_llm::LLMProvider;
use aivt_provider_tools::{BraveSearchTool, CurrentTimeTool, Tool};
use aivt_secrets::SecretStore;
use aivt_service_manager::ManagedService;
use aivt_service_settings::{DiscordConnectionSettings, SettingsService};

use crate::error::DiscordError;
use crate::gateway::DiscordGateway;
use crate::http::DiscordHttpClient;
use crate::policy::{contains_word, should_respond, CooldownTracker};
use crate::types::DiscordChatMessage;

/// Discord messages are capped at 2000 characters — truncate rather than
/// let `send_message` fail outright over a response that ran long. See
/// `aivt_service_twitch::service::MAX_MESSAGE_LEN` for the same reasoning
/// at Twitch's own (much lower) limit.
const MAX_MESSAGE_LEN: usize = 2000;

fn box_err(err: DiscordError) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(err)
}

struct Inner {
    pool: DbPool,
    event_bus: EventBus,
    secrets: Arc<dyn SecretStore>,
    /// Which profile answers in Discord — kept live-synced from the
    /// frontend (`discord_set_active_profile`), same reasoning as
    /// `aivt_service_twitch::Inner::active_profile_id`.
    active_profile_id: RwLock<Option<String>>,
    cooldown: CooldownTracker,
    /// Set while `Running`; cancelling it is how `stop()` tears down the
    /// background Gateway message loop.
    cancellation: Mutex<Option<CancellationToken>>,
    /// Registers real message authors in Personas (see
    /// `run_message_loop`'s `observe_discord_author` call) — mirrors
    /// `aivt_service_twitch::Inner::people`.
    people: Arc<dyn PeopleManager>,
    /// Discord está abierto a terceros: solo puede usar memorias de alcance
    /// público (`search_public`). Lo que el dueño dijo en una charla privada
    /// nunca debe aparecer acá.
    memory: Arc<dyn MemoryManager>,
}

/// Pocos recuerdos a propósito: un mensaje de Discord es corto y el prompt no
/// debe llenarse de contexto (mismo criterio que `ChatService`).
const MEMORY_RETRIEVAL_LIMIT: i64 = 5;

impl Inner {
    fn settings(&self) -> SettingsService {
        SettingsService::new(self.pool.clone(), self.secrets.clone())
    }

    /// Loads the active profile, calls the LLM, and returns the
    /// Discord-length-capped reply — shared by a real message
    /// (`respond_to`, which then posts it to Discord) and a manual test
    /// (`DiscordService::send_test_message`, which never touches the
    /// Discord API at all). Text-only on purpose: unlike Twitch/Chat/
    /// Llamada, Discord replies are never spoken, regardless of the
    /// global voice mute (`AivtTtsProvider::is_muted`) — a live-testing
    /// round found a bot suddenly talking (no voice channel involved,
    /// just text) surprising, so this integration doesn't touch
    /// `TTSProvider` at all rather than needing yet another mute toggle.
    async fn generate_reply(&self, trigger_text: &str, author_display_name: &str) -> Result<(String, String), DiscordError> {
        let Some(profile_id) = self.active_profile_id.read().await.clone() else {
            tracing::info!(target: "system", "discord: no se puede responder, no hay un personaje activo (selecciónalo en Identidad)");
            return Err(DiscordError::NoActiveProfile);
        };

        let profile = ProfilesRepository::new(&self.pool).get(&profile_id).await?.ok_or(DiscordError::NoActiveProfile)?;

        let llm_connection = match self.settings().llm_connection().await {
            Ok(Some(connection)) => connection,
            Ok(None) => {
                tracing::warn!(target: "system", "discord: no hay proveedor de IA configurado, se omite la respuesta");
                return Err(DiscordError::NoLlmConnection);
            }
            Err(err) => {
                tracing::warn!(target: "system", "discord: no se pudo leer la conexión de IA: {err}");
                return Err(DiscordError::Settings(err.to_string()));
            }
        };
        let provider: Arc<dyn LLMProvider> = match self.settings().build_llm_provider(&self.event_bus).await {
            Ok(provider) => provider,
            Err(err) => {
                tracing::warn!(target: "system", "discord: no se pudo armar el proveedor de IA: {err}");
                return Err(DiscordError::Settings(err.to_string()));
            }
        };
        // Same `max_tokens` the user configured for Chat, not an
        // artificially tight cap — Discord messages are capped at 2000
        // characters, but `truncate_for_discord` already enforces that on
        // the *visible* output. A low token cap here instead used to
        // starve reasoning models (which stream their "thinking" through
        // a separate field this provider ignores) of enough budget to
        // ever reach visible content, producing `EmptyResponse` — see
        // `aivt_service_twitch::service`'s identical fix for the full
        // story (a live-testing session hit this with a Qwen3 model).
        let language = self.settings().language().await.unwrap_or_else(|_| "es".to_string());
        let mut orchestrator = AIOrchestrator::new(provider, llm_connection.model, Some(llm_connection.temperature), Some(llm_connection.max_tokens)).with_channel(Channel::Discord);
        // `get_current_time` is always available — pure local calculation,
        // no network/API key, so it doesn't need the same opt-in as
        // `web_search` (Fase 10b). `web_search` itself stays behind
        // `WebSearchSettings.enabled` — see its doc comment.
        let mut tools: Vec<Arc<dyn Tool>> = vec![Arc::new(CurrentTimeTool::new(language.as_str()))];
        if let Ok(web_search) = self.settings().web_search_settings().await {
            if web_search.enabled {
                if let Ok(Some(key)) = self.settings().web_search_api_key().await {
                    tools.push(Arc::new(BraveSearchTool::new(key, language.as_str())));
                }
            }
        }
        orchestrator = orchestrator.with_tools(tools);

        let synthetic_history = [Message {
            id: "discord-trigger".to_string(),
            conversation_id: "discord".to_string(),
            role: MessageRole::User,
            content: format!("[Mensaje de {author_display_name} en Discord] {trigger_text}"),
            status: MessageStatus::Complete,
            provider_id: None,
            model: None,
            input_tokens: None,
            output_tokens: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        }];

        // Solo memorias públicas — ver `Inner::memory`. Un fallo degrada a "sin
        // recuerdos", nunca bloquea la respuesta. Sin mood: es un canal público
        // y su estado de ánimo viene de charlas privadas con el dueño.
        let public_memories = match self.memory.search_public(&profile.id, trigger_text, MEMORY_RETRIEVAL_LIMIT).await {
            Ok(memories) => memories,
            Err(err) => {
                tracing::warn!(target: "system", "discord: no se pudieron buscar recuerdos públicos, se responde sin ellos: {err}");
                Vec::new()
            }
        };

        use futures::StreamExt;
        let mut stream = orchestrator.stream_respond(&profile, &synthetic_history, None, &public_memories, None, None, &language);
        let mut full_text = String::new();
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(chunk) => {
                    full_text.push_str(&chunk.delta);
                    if chunk.finished {
                        break;
                    }
                }
                Err(err) => {
                    tracing::warn!(target: "system", "discord: la generación falló: {err}");
                    return Err(DiscordError::Generation(err.to_string()));
                }
            }
        }

        let reply = truncate_for_discord(full_text.trim());
        if reply.is_empty() {
            tracing::warn!(target: "system", "discord: la IA generó una respuesta vacía, se omite");
            return Err(DiscordError::EmptyResponse);
        }
        Ok((profile.name, reply))
    }

    async fn respond_to(&self, message: &DiscordChatMessage, http: &DiscordHttpClient) {
        let Ok((character_name, reply)) = self.generate_reply(&message.text, &message.author_display_name).await else {
            return;
        };

        match http.send_message(&message.channel_id, &reply).await {
            Ok(()) => {
                self.cooldown.mark_responded();
                self.event_bus.publish(AppEvent::DiscordMessageSent { character_name, text: reply });
                let language = self.settings().language().await.unwrap_or_else(|_| "es".to_string());
                tracing::info!(
                    target: "activity",
                    category = "DISCORD",
                    module = "discord",
                    action = "respond",
                    status = "success",
                    summary = %replied_to_message(&message.author_display_name, &language),
                );
            }
            Err(err) => {
                tracing::warn!(target: "system", "discord: no se pudo publicar la respuesta: {err}");
            }
        }
    }

    /// Whether `text` calls out the active character by its profile name
    /// or nickname (Identidad tab) — checked in addition to Discord's own
    /// structured `mentions_bot` signal, mirroring
    /// `aivt_service_twitch::Inner::mentions_active_character`.
    async fn mentions_active_character(&self, text: &str) -> bool {
        let Some(profile_id) = self.active_profile_id.read().await.clone() else {
            return false;
        };
        let profile = match ProfilesRepository::new(&self.pool).get(&profile_id).await {
            Ok(Some(profile)) => profile,
            _ => return false,
        };
        if contains_word(text, &profile.name) {
            return true;
        }
        match profile.nickname {
            Some(nickname) => contains_word(text, &nickname),
            None => false,
        }
    }

    /// Builds the Gateway connection and REST client from the saved bot
    /// token — the synchronous part of "starting", awaited inside
    /// `ManagedService::start` so a missing token surfaces there instead of
    /// silently inside a detached task. Unlike
    /// `aivt_service_twitch::Inner::establish_connection`, this never talks
    /// to the network itself: `DiscordGateway::connect` only opens the
    /// socket lazily on the first `next_message` call, and the bot's own
    /// identity was already resolved once by `DiscordService::connect_bot`
    /// and is simply reused from settings here.
    async fn establish_connection(&self) -> Result<(DiscordGateway, DiscordHttpClient), DiscordError> {
        let connection = self.settings().discord_connection().await.map_err(|err| DiscordError::Settings(err.to_string()))?;
        if !connection.has_token {
            return Err(DiscordError::NotConnected);
        }
        let Some(token) = self.settings().discord_token().await.map_err(|err| DiscordError::Settings(err.to_string()))? else {
            return Err(DiscordError::NotConnected);
        };
        let own_user_id: u64 = connection.bot_user_id.parse().map_err(|_| DiscordError::NotConnected)?;

        let gateway = DiscordGateway::connect(token.clone(), own_user_id);
        let http = DiscordHttpClient::new(token);
        Ok((gateway, http))
    }

    async fn run_message_loop(self: Arc<Self>, mut gateway: DiscordGateway, http: DiscordHttpClient, token: CancellationToken) {
        loop {
            let message = tokio::select! {
                () = token.cancelled() => return,
                message = gateway.next_message() => match message {
                    Ok(message) => message,
                    Err(err) => {
                        tracing::warn!(target: "system", "discord: la conexión con el Gateway se cerró: {err}");
                        self.event_bus.publish(AppEvent::ServiceStatusChanged { service: "discord".to_string(), status: ServiceStatus::Error });
                        return;
                    }
                },
            };

            // Every bot account's message is skipped here, before it's
            // ever shown in the feed — not just a self-login check like
            // Twitch's, since Discord servers can have other bots too.
            // Without this, the character's own reply would appear twice
            // in the feed: once as the "Personaje" entry `respond_to`
            // publishes, and again here, since Discord's Gateway echoes
            // every message back to the bot, including its own. See
            // `policy::should_respond`'s doc comment for the matching
            // reasoning on the response-triggering side.
            if message.author_is_bot {
                continue;
            }

            self.event_bus.publish(AppEvent::DiscordMessageReceived {
                author_display_name: message.author_display_name.clone(),
                text: message.text.clone(),
            });
            if let Err(err) = self.people.observe_discord_author(&message.author_id, &message.author_display_name).await {
                tracing::warn!(target: "system", "discord: no se pudo registrar al autor en Personas: {err}");
            }

            let policy = self.settings().discord_policy().await.unwrap_or_default();
            // Only worth the profile DB read when it could actually change
            // the outcome — see `should_respond`'s doc comment. (Bot
            // messages are already filtered out above, so `should_respond`'s
            // own `author_is_bot` check is now purely defense-in-depth.)
            let mentions_character = policy.enabled && !policy.respond_to_all && policy.mentioned_only && self.mentions_active_character(&message.text).await;
            if should_respond(&policy, &message, mentions_character) {
                if self.cooldown.ready(policy.cooldown_seconds) {
                    self.respond_to(&message, &http).await;
                } else {
                    tracing::info!(target: "system", "discord: mensaje calificaba pero está en cooldown, se omite");
                }
            }
        }
    }
}

fn replied_to_message(display_name: &str, lang: &str) -> String {
    match lang {
        "en" => format!("Replied to {display_name}"),
        "pt" => format!("Respondeu a {display_name}"),
        "ja" => format!("{display_name}に返信しました"),
        "ko" => format!("{display_name}에게 답장했습니다"),
        "zh" => format!("已回复 {display_name}"),
        _ => format!("Respondió a {display_name}"),
    }
}

fn truncate_for_discord(text: &str) -> String {
    if text.chars().count() <= MAX_MESSAGE_LEN {
        return text.to_string();
    }
    let truncated: String = text.chars().take(MAX_MESSAGE_LEN.saturating_sub(1)).collect();
    format!("{truncated}…")
}

/// `Arc`-wrapped from the start, same reasoning as
/// `aivt_service_twitch::TwitchService`: the background Gateway loop it
/// spawns needs an owned, `'static` handle back into this service's state.
#[derive(Clone)]
pub struct DiscordService(Arc<Inner>);

impl DiscordService {
    pub fn new(
        pool: DbPool,
        event_bus: EventBus,
        secrets: Arc<dyn SecretStore>,
        people: Arc<dyn PeopleManager>,
        memory: Arc<dyn MemoryManager>,
    ) -> Self {
        Self(Arc::new(Inner {
            pool,
            event_bus,
            secrets,
            active_profile_id: RwLock::new(None),
            cooldown: CooldownTracker::new(),
            cancellation: Mutex::new(None),
            people,
            memory,
        }))
    }

    pub async fn set_active_profile(&self, profile_id: Option<String>) {
        *self.0.active_profile_id.write().await = profile_id;
    }

    /// Runs the exact same generation pipeline `respond_to` uses for a
    /// real message, but never touches the Discord API — useful for
    /// seeing a reply without inviting the bot to a real server first.
    /// Mirrors `aivt_service_twitch::TwitchService::send_test_message`,
    /// though unlike Twitch this isn't the *only* way to test — a real
    /// account other than the bot's own can just talk to it directly.
    pub async fn send_test_message(&self, text: String) -> Result<(), DiscordError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(DiscordError::EmptyResponse);
        }

        self.0.event_bus.publish(AppEvent::DiscordMessageReceived { author_display_name: "Tú (prueba)".to_string(), text: trimmed.to_string() });

        let (character_name, reply) = self.0.generate_reply(trimmed, "Tú (prueba)").await?;

        self.0.event_bus.publish(AppEvent::DiscordMessageSent { character_name, text: reply });
        Ok(())
    }

    /// Validates a freshly-pasted bot token by resolving its own identity
    /// (`GET /users/@me`) before ever saving it or opening the Gateway —
    /// an invalid token fails immediately here with a clear error, rather
    /// than silently inside `start()`'s background task. No OAuth round
    /// trip at all: the token itself, generated once in the Discord
    /// Developer Portal, is the whole credential.
    pub async fn connect_bot(&self, token: String) -> Result<DiscordConnectionSettings, DiscordError> {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            return Err(DiscordError::MissingToken);
        }

        let http = DiscordHttpClient::new(trimmed.to_string());
        let (username, user_id, avatar_url) = http.current_user().await?;

        self.0
            .settings()
            .set_discord_token(trimmed, &username, &user_id.to_string(), &avatar_url)
            .await
            .map_err(|err| DiscordError::Settings(err.to_string()))
    }

    pub async fn disconnect(&self) -> Result<(), DiscordError> {
        self.0.settings().disconnect_discord().await.map_err(|err| DiscordError::Settings(err.to_string()))?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl ManagedService for DiscordService {
    fn name(&self) -> &str {
        "discord"
    }

    async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (gateway, http) = self.0.establish_connection().await.map_err(box_err)?;

        let token = CancellationToken::new();
        *self.0.cancellation.lock().await = Some(token.clone());

        let inner = self.0.clone();
        tokio::spawn(async move {
            inner.run_message_loop(gateway, http, token).await;
        });

        Ok(())
    }

    async fn stop(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(token) = self.0.cancellation.lock().await.take() {
            token.cancel();
        }
        Ok(())
    }
}
