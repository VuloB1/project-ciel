use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

use aivt_database::{CachedEmote, DbPool, Message, MessageRole, MessageStatus, Profile, ProfilesRepository, TwitchEmotesRepository};
use aivt_eventbus::{AppEvent, EventBus, ServiceStatus};
use aivt_memory::{MemoryManager, PeopleManager};
use aivt_orchestrator::{AIOrchestrator, Channel};
use aivt_provider_llm::LLMProvider;
use aivt_provider_tools::{BraveSearchTool, CurrentTimeTool, Tool};
use aivt_provider_tts::{strip_markdown, SpeakRequest, TTSProvider};
use aivt_secrets::SecretStore;
use aivt_service_manager::ManagedService;
use aivt_service_settings::{SettingsService, TwitchConnectionSettings};

use crate::auth::{self, DeviceCodeFlow};
use crate::context::ChatterContext;
use crate::emotes;
use crate::error::TwitchError;
use crate::eventsub::{EventSubConnection, EventSubMessage};
use crate::helix::HelixClient;
use crate::policy::{contains_word, should_respond, CooldownTracker};
use crate::types::TwitchChatMessage;

/// Twitch chat messages are capped at 500 characters — truncate rather
/// than let `send_chat_message` fail outright over a response that ran
/// long. `AIOrchestrator`'s smaller `max_tokens` (see `respond_to`)
/// already biases toward short replies; this is just the hard floor.
const MAX_MESSAGE_LEN: usize = 480;

/// Twitch chat renders no Markdown at all — a `**bold**` or `_italic_` the
/// model would happily write for Chat/Discord shows up completely literal
/// there (pedido del usuario tras verlo en vivo). Always the first part of
/// `Inner::build_extra_instructions`'s result, passed as
/// `extra_instructions` to `AIOrchestrator::stream_respond` so it only
/// affects Twitch; `generate_reply` below also runs `strip_markdown` on the
/// reply as a belt-and-suspenders pass for whatever a weaker local model
/// writes anyway despite the instruction (same reasoning as the mood/tool
/// prompt sections needing a trailing reminder to actually stick).
///
/// `lang` follows `SettingsService::language`, same convention as
/// `PromptComposer` (falls back to Spanish) — see its doc comment.
fn no_markdown_instruction(lang: &str) -> &'static str {
    match lang {
        "en" => {
            "Twitch chat doesn't render Markdown: never use **bold**, _italic_, `code`, # \
             headers, or dash lists — they show up completely literal on screen, symbols \
             included. Always respond in plain text."
        }
        "pt" => {
            "O chat da Twitch não interpreta Markdown: nunca use **negrito**, _itálico_, \
             `código`, títulos com #, nem listas com traços — aparecem literalmente na \
             tela, símbolos incluídos. Responda sempre em texto simples."
        }
        "ja" => {
            "Twitchのチャットは Markdown を解釈しない。**太字**、_斜体_、`コード`、#の見出し、ハイフンの \
             リストは絶対に使わないこと — 記号も含めてそのまま画面に表示されてしまう。常にプレーンテキストで \
             返答すること。"
        }
        "ko" => {
            "트위치 채팅은 마크다운을 해석하지 않는다. **굵게**, _기울임_, `코드`, # 제목, 하이픈 목록은 \
             절대 사용하지 말 것 — 기호까지 그대로 화면에 표시된다. 항상 일반 텍스트로만 답할 것."
        }
        "zh" => {
            "Twitch 聊天不会渲染 Markdown：绝不要使用 **粗体**、_斜体_、`代码`、# 标题或短横线列表——它们会\
             连同符号一起原样显示在屏幕上。请始终使用纯文本回复。"
        }
        _ => {
            "El chat de Twitch no interpreta Markdown: nunca uses **negrita**, _cursiva_, \
             `código`, encabezados con #, ni listas con guiones — se ven literales en \
             pantalla, con los símbolos incluidos. Responde siempre en texto plano."
        }
    }
}

/// Appended before the emote list itself in `Inner::build_extra_instructions`
/// — the emote names/descriptions that follow are 7TV/BTTV/FFZ community
/// data, never translated (same principle as a memory's own content).
fn emote_list_instruction(lang: &str) -> &'static str {
    match lang {
        "en" => {
            "\n\nTwitch emotes available in this channel — they're plain text, not images. \
             To use one, type its EXACT name as it appears to the left of the dash \
             (matching capitalization), worked into the sentence. The text after the dash \
             is only so you know when each one fits — never write it or put it in \
             parentheses in your reply, and don't make up emotes that aren't on this list. \
             Example: if the list says 'GAMBA — casino, gambling, addiction' and you want \
             to use it, you literally type GAMBA in your reply — never '(casino, gambling, \
             addiction)' or '(GAMBA)'.\n"
        }
        "pt" => {
            "\n\nEmotes da Twitch disponíveis neste canal — são texto simples, não imagens. \
             Para usar um, escreva o nome EXATO tal como aparece à esquerda do traço \
             (respeitando maiúsculas), integrado na frase. O texto depois do traço é só \
             para você saber quando cada um se encaixa — nunca o escreva nem o coloque \
             entre parênteses na sua resposta, e não invente emotes que não estejam nesta \
             lista. Exemplo: se a lista disser 'GAMBA — cassino, apostas, vício' e você \
             quiser usá-lo, escreva literalmente GAMBA na sua resposta — nunca '(cassino, \
             apostas, vício)' nem '(GAMBA)'.\n"
        }
        "ja" => {
            "\n\nこのチャンネルで使えるTwitchエモート一覧 — これらはプレーンテキストであり、画像ではない。\
             使うときはハイフンの左側にある名前をそのまま正確に（大文字小文字も含めて）文中に入れて書くこと。\
             ハイフンの後ろの文章はどの場面に合うかの参考情報でしかない — それを返信にそのまま書いたり、\
             括弧で囲んだりしないこと。またこのリストにないエモートを勝手に作らないこと。例：リストに\
             「GAMBA — カジノ、ギャンブル、依存症」とあり、それを使いたい場合は返信に文字通り GAMBA と書く \
             — 「（カジノ、ギャンブル、依存症）」や「（GAMBA）」とは絶対に書かない。\n"
        }
        "ko" => {
            "\n\n이 채널에서 사용할 수 있는 트위치 이모트 목록 — 이것들은 이미지가 아니라 일반 텍스트다. \
             사용하려면 하이픈 왼쪽에 있는 이름을 (대소문자 그대로) 정확히 문장 속에 넣어서 쓸 것. 하이픈 \
             뒤의 텍스트는 언제 어울리는지 참고하기 위한 것일 뿐이다 — 답변에 그대로 쓰거나 괄호로 묶지 \
             말 것. 또한 이 목록에 없는 이모트를 만들어내지 말 것. 예: 목록에 'GAMBA — 카지노, 도박, \
             중독'이라고 되어 있고 이를 사용하고 싶다면, 답변에 그대로 GAMBA라고 쓸 것 — '(카지노, 도박, \
             중독)'이나 '(GAMBA)'라고 쓰지 말 것.\n"
        }
        "zh" => {
            "\n\n此频道可用的 Twitch 表情——它们是纯文本，不是图片。要使用某个表情，请在句子中准确输入横线\
             左边的名称（大小写要一致）。横线后面的文字只是帮助你判断什么场合适合用——绝不要把它写进回复里\
             或加上括号，也不要编造这个列表里没有的表情。例如：如果列表写着 'GAMBA — 赌场、赌博、上瘾'，\
             你想用它，就在回复里原样写 GAMBA——绝不要写成 '(赌场、赌博、上瘾)' 或 '(GAMBA)'。\n"
        }
        _ => {
            "\n\nEmotes de Twitch disponibles en este canal — son texto plano, no imágenes. Para usar uno, \
             escribí su nombre EXACTO tal cual aparece a la izquierda del guion (respetando mayúsculas), \
             integrado en la oración. El texto después del guion es solo para que sepas cuándo encaja cada \
             uno — nunca lo escribas ni lo pongas entre paréntesis en tu respuesta, y no inventes emotes que \
             no estén en esta lista. Ejemplo: si la lista dice 'GAMBA — casino, apuestas, adicción' y querés \
             usarlo, escribís literalmente GAMBA en tu respuesta — nunca '(casino, apuestas, adicción)' ni \
             '(GAMBA)'.\n"
        }
    }
}

/// Caps how many cached emotes go into the prompt — a channel with a huge
/// 7TV/BTTV/FFZ set shouldn't bloat every single Twitch turn. Described
/// emotes sort first (`TwitchEmotesRepository::list_for_broadcaster`), so a
/// channel over the cap loses its *least* useful entries (bare names with
/// no description or tags) first.
const MAX_EMOTES_IN_PROMPT: usize = 50;

/// Pocos recuerdos a propósito: un mensaje del chat es corto y el prompt no
/// debe llenarse de contexto (mismo criterio que `ChatService`).
const MEMORY_RETRIEVAL_LIMIT: i64 = 5;

fn box_err(err: TwitchError) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(err)
}

struct Inner {
    pool: DbPool,
    event_bus: EventBus,
    secrets: Arc<dyn SecretStore>,
    tts: Arc<dyn TTSProvider>,
    /// Auto-registers chatters by name (registro de personas — see
    /// `aivt_memory::PeopleManager`), feeding `CallService::
    /// transcription_prompt`'s Whisper priming. Twitch never reads the
    /// `PeoplePolicy` toggle itself — `observe_twitch_chatter` already
    /// no-ops when the user disabled auto-learn.
    people: Arc<dyn PeopleManager>,
    /// El chat de Twitch está abierto a terceros: solo puede usar memorias de
    /// alcance público (`search_public`). Lo que el dueño dijo en una charla
    /// privada nunca debe aparecer acá.
    memory: Arc<dyn MemoryManager>,
    http: reqwest::Client,
    /// Which profile answers in chat — kept live-synced from the frontend
    /// (`twitch_set_active_profile`), since this service runs independently
    /// of whatever tab the user currently has open (see
    /// docs/DEVELOPMENT_STATUS.md, Fase 8, for why "active profile" isn't
    /// otherwise known to the backend at all).
    active_profile_id: RwLock<Option<String>>,
    cooldown: CooldownTracker,
    /// Per-chatter rolling conversation memory, in-memory only — see
    /// `ChatterContext`'s doc comment for why a reply otherwise has no idea
    /// what that same person just said a message or two ago.
    chatter_context: ChatterContext,
    /// Set while `Running`; cancelling it is how `stop()` tears down the
    /// background message loop.
    cancellation: Mutex<Option<CancellationToken>>,
}

impl Inner {
    fn settings(&self) -> SettingsService {
        SettingsService::new(self.pool.clone(), self.secrets.clone())
    }

    /// The no-Markdown line, plus — only when the user turned emotes on
    /// and the currently-connected channel actually has some cached — a
    /// list of what's available. Any failure reading settings/the cache
    /// just falls back to the no-Markdown line alone rather than failing
    /// the whole reply over an optional feature.
    async fn build_extra_instructions(&self, lang: &str) -> String {
        let mut instructions = no_markdown_instruction(lang).to_string();

        let emote_settings = self.settings().twitch_emote_settings().await.unwrap_or_default();
        if !emote_settings.enabled {
            return instructions;
        }

        let Ok(connection) = self.settings().twitch_connection().await else {
            return instructions;
        };
        let broadcaster_id = effective_broadcaster_id(&connection);
        if broadcaster_id.is_empty() {
            return instructions;
        }

        let emotes = match TwitchEmotesRepository::new(&self.pool).list_for_broadcaster(broadcaster_id).await {
            Ok(emotes) if !emotes.is_empty() => emotes,
            _ => return instructions,
        };

        instructions.push_str(emote_list_instruction(lang));
        let lines: Vec<String> = emotes.iter().take(MAX_EMOTES_IN_PROMPT).map(describe_emote).collect();
        instructions.push_str(&lines.join("\n"));

        instructions
    }

    /// Loads the active profile, calls the LLM, and returns the
    /// Twitch-length-capped reply — the part shared by a real chat reply
    /// (`respond_to`, which then posts it to Twitch) and a manual test
    /// (`TwitchService::send_test_message`, which never touches Helix at
    /// all). Every failure path logs *why* at a level that shows up in the
    /// log file without `RUST_LOG=debug` (sección "Problemas encontrados",
    /// Fase 8 — silent failures here were previously indistinguishable
    /// from "working as configured").
    ///
    /// `chatter_id` keys `self.chatter_context` — that same person's recent
    /// messages (and the character's own replies to them) are prepended to
    /// the prompt as real history instead of just the current trigger, so
    /// a reply to someone's second message actually knows what their first
    /// one said. Nothing is recorded (and no history looked up) on a
    /// failure path — a chatter whose message never got a reply shouldn't
    /// have a one-sided gap saved into their context.
    async fn generate_reply(&self, chatter_id: &str, trigger_text: &str, chatter_display_name: &str) -> Result<(Profile, String), TwitchError> {
        let Some(profile_id) = self.active_profile_id.read().await.clone() else {
            tracing::info!(target: "system", "twitch: no se puede responder, no hay un personaje activo (selecciónalo en Identidad)");
            return Err(TwitchError::NoActiveProfile);
        };

        let profile = ProfilesRepository::new(&self.pool).get(&profile_id).await?.ok_or(TwitchError::NoActiveProfile)?;

        let llm_connection = match self.settings().llm_connection().await {
            Ok(Some(connection)) => connection,
            Ok(None) => {
                tracing::warn!(target: "system", "twitch: no hay proveedor de IA configurado, se omite la respuesta");
                return Err(TwitchError::NoLlmConnection);
            }
            Err(err) => {
                tracing::warn!(target: "system", "twitch: no se pudo leer la conexión de IA: {err}");
                return Err(TwitchError::Settings(err.to_string()));
            }
        };
        let provider: Arc<dyn LLMProvider> = match self.settings().build_llm_provider(&self.event_bus).await {
            Ok(provider) => provider,
            Err(err) => {
                tracing::warn!(target: "system", "twitch: no se pudo armar el proveedor de IA: {err}");
                return Err(TwitchError::Settings(err.to_string()));
            }
        };
        // Same `max_tokens` the user configured for Chat, not an
        // artificially tight cap — Twitch messages are capped at 500
        // characters, but `truncate_for_twitch` already enforces that on
        // the *visible* output. A low token cap here instead used to
        // starve reasoning models (which stream their "thinking" through
        // a separate field this provider ignores, see `StreamDelta`) of
        // enough budget to ever reach visible content, producing
        // `EmptyResponse` almost every time — a live-testing session hit
        // exactly this with a Qwen3 model.
        let language = self.settings().language().await.unwrap_or_else(|_| "es".to_string());
        let mut orchestrator = AIOrchestrator::new(provider, llm_connection.model, Some(llm_connection.temperature), Some(llm_connection.max_tokens)).with_channel(Channel::Twitch);
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

        let trigger_message = Message {
            id: "twitch-trigger".to_string(),
            conversation_id: "twitch".to_string(),
            role: MessageRole::User,
            content: format!("[Mensaje de {chatter_display_name} en el chat de Twitch] {trigger_text}"),
            status: MessageStatus::Complete,
            provider_id: None,
            model: None,
            input_tokens: None,
            output_tokens: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let mut synthetic_history = self.chatter_context.history_for(chatter_id);
        synthetic_history.push(trigger_message.clone());

        let extra_instructions = self.build_extra_instructions(&language).await;

        // Solo memorias públicas — ver `Inner::memory`. Un fallo degrada a "sin
        // recuerdos", nunca bloquea la respuesta. Sin mood: es un canal público
        // y su estado de ánimo viene de charlas privadas con el dueño.
        let public_memories = match self.memory.search_public(&profile.id, trigger_text, MEMORY_RETRIEVAL_LIMIT).await {
            Ok(memories) => memories,
            Err(err) => {
                tracing::warn!(target: "system", "twitch: no se pudieron buscar recuerdos públicos, se responde sin ellos: {err}");
                Vec::new()
            }
        };

        use futures::StreamExt;
        let mut stream = orchestrator.stream_respond(&profile, &synthetic_history, None, &public_memories, None, Some(&extra_instructions), &language);
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
                    tracing::warn!(target: "system", "twitch: la generación falló: {err}");
                    return Err(TwitchError::Generation(err.to_string()));
                }
            }
        }

        let reply = truncate_for_twitch(strip_markdown(full_text.trim()).trim());
        if reply.is_empty() {
            tracing::warn!(target: "system", "twitch: la IA generó una respuesta vacía, se omite");
            return Err(TwitchError::EmptyResponse);
        }

        let character_message = Message {
            id: "twitch-reply".to_string(),
            conversation_id: "twitch".to_string(),
            role: MessageRole::Assistant,
            content: reply.clone(),
            status: MessageStatus::Complete,
            provider_id: None,
            model: None,
            input_tokens: None,
            output_tokens: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        self.chatter_context.record_exchange(chatter_id, trigger_message, character_message);

        Ok((profile, reply))
    }

    async fn respond_to(&self, message: &TwitchChatMessage, target_broadcaster_id: &str, sender_id: &str, access_token: &str) {
        let Ok((profile, reply)) = self.generate_reply(&message.chatter_user_id, &message.text, &message.chatter_display_name).await else {
            return;
        };

        let helix = HelixClient::new(&self.http, crate::CLIENT_ID, access_token);
        match helix.send_chat_message(target_broadcaster_id, sender_id, &reply).await {
            Ok(()) => {
                self.cooldown.mark_responded();
                self.event_bus.publish(AppEvent::TwitchMessageSent { character_name: profile.name.clone(), text: reply.clone() });
                let language = self.settings().language().await.unwrap_or_else(|_| "es".to_string());
                tracing::info!(
                    target: "activity",
                    category = "TWITCH",
                    module = "twitch",
                    action = "respond",
                    status = "success",
                    summary = %replied_to_message(&message.chatter_display_name, &language),
                );
                self.spawn_speak(profile, reply);
            }
            Err(err) => {
                tracing::warn!(target: "system", "twitch: no se pudo publicar la respuesta: {err}");
            }
        }
    }

    /// Fires TTS playback as a detached task — same reasoning as
    /// `ChatService::maybe_speak`: a slow or failing speech engine must
    /// never delay the next message in `run_message_loop`, which awaits
    /// this call's caller directly. Silently does nothing when the active
    /// profile has voice turned off, matching Chat's auto-play behavior
    /// so "does this character have a voice at all" stays a single Identidad
    /// setting shared by every channel, not a separate Twitch-only toggle.
    fn spawn_speak(&self, profile: Profile, text: String) {
        if !profile.voice.enabled || text.trim().is_empty() {
            tracing::info!(
                target: "system",
                "twitch: TTS omitido (voz activada={}, texto vacío={})",
                profile.voice.enabled,
                text.trim().is_empty(),
            );
            return;
        }

        let event_bus = self.event_bus.clone();
        let tts = self.tts.clone();
        let settings = self.settings();
        let profile_id = profile.id.clone();
        let voice = profile.voice.clone();

        tokio::spawn(async move {
            let backend = match settings.build_tts_backend().await {
                Ok(backend) => backend,
                Err(err) => {
                    tracing::warn!(target: "system", "twitch: no se pudo leer la configuración de voz: {err}");
                    return;
                }
            };

            tracing::info!(target: "system", profile_id = %profile_id, "twitch: reproduciendo respuesta por voz");
            let request = SpeakRequest {
                text,
                voice_id: voice.voice_id,
                rate: voice.rate as f32,
                pitch: voice.pitch as f32,
                volume: voice.volume as f32,
                backend,
                profile_id: profile_id.clone(),
                message_id: None,
            };
            if let Err(err) = tts.speak(request).await {
                tracing::warn!(target: "system", "twitch: TTS falló: {err}");
                event_bus.publish(AppEvent::VoicePlaybackError { profile_id, message_id: None, message: err.to_string() });
            }
        });
    }

    /// Whether `text` calls out the active character by its profile name
    /// or nickname (Identidad tab) — viewers can't officially @-mention
    /// the character on Twitch since only the broadcaster's own account
    /// exists there, so this plain-text match is what actually drives
    /// `mentioned_only` mode day-to-day. Returns `false` with no DB read
    /// wasted when there's no active profile.
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

    /// Connects to EventSub and subscribes to chat for the target channel
    /// — the synchronous part of "starting", awaited inside
    /// `ManagedService::start` so a bad token or network failure surfaces
    /// there instead of silently inside a detached task.
    async fn establish_connection(&self) -> Result<(EventSubConnection, TwitchConnectionSettings, String), TwitchError> {
        let connection = self.settings().twitch_connection().await.map_err(|err| TwitchError::Settings(err.to_string()))?;
        if !connection.has_tokens {
            return Err(TwitchError::NotConnected);
        }
        let Some((mut access_token, refresh_token)) =
            self.settings().twitch_tokens().await.map_err(|err| TwitchError::Settings(err.to_string()))?
        else {
            return Err(TwitchError::NotConnected);
        };

        let (socket, session_id) = EventSubConnection::connect(connection.broadcaster_id.clone()).await?;

        let condition = serde_json::json!({
            "broadcaster_user_id": effective_broadcaster_id(&connection),
            "user_id": connection.broadcaster_id,
        });
        let subscribe_result =
            HelixClient::new(&self.http, crate::CLIENT_ID, &access_token).create_eventsub_subscription("channel.chat.message", "1", condition.clone(), &session_id).await;

        // A stale access token surfaces here as a 401 from Helix — refresh
        // once and retry before giving up entirely.
        if let Err(TwitchError::Api { status: 401, .. }) = subscribe_result {
            let refreshed = auth::refresh_tokens(&self.http, crate::CLIENT_ID, &refresh_token).await?;
            self.settings()
                .set_twitch_tokens(&refreshed.access_token, &refreshed.refresh_token)
                .await
                .map_err(|err| TwitchError::Settings(err.to_string()))?;
            access_token = refreshed.access_token;
            HelixClient::new(&self.http, crate::CLIENT_ID, &access_token)
                .create_eventsub_subscription("channel.chat.message", "1", condition, &session_id)
                .await?;
        } else {
            subscribe_result?;
        }

        Ok((socket, connection, access_token))
    }

    async fn run_message_loop(
        self: Arc<Self>,
        mut socket: EventSubConnection,
        mut connection: TwitchConnectionSettings,
        mut access_token: String,
        token: CancellationToken,
    ) {
        loop {
            let message = tokio::select! {
                () = token.cancelled() => return,
                message = socket.next_message() => message,
            };

            let message = match message {
                Ok(message) => message,
                Err(err) => {
                    // Unlike Discord's `twilight_gateway::Shard`, this
                    // hand-rolled WebSocket has no built-in auto-resume —
                    // once it closes it's genuinely dead, so recovering
                    // means reconnecting from scratch (new session,
                    // re-subscribed via Helix), not just polling again.
                    // Live testing found a slow tool-calling response
                    // (Fase 10b) blocking this loop long enough for Twitch
                    // to consider the connection unresponsive and close
                    // it — treating that as fatal (as an earlier version
                    // of this loop did) killed chat listening for good
                    // until the user manually reconnected. Same
                    // reconnect-then-continue shape `EventSubMessage::
                    // Reconnect` already uses below for Twitch-initiated
                    // reconnects.
                    tracing::warn!(target: "system", "twitch: la conexión de chat se cerró ({err}), reconectando...");
                    match self.establish_connection().await {
                        Ok((new_socket, new_connection, new_access_token)) => {
                            socket = new_socket;
                            connection = new_connection;
                            access_token = new_access_token;
                            continue;
                        }
                        Err(reconnect_err) => {
                            tracing::warn!(target: "system", "twitch: no se pudo reconectar: {reconnect_err}");
                            self.event_bus.publish(AppEvent::ServiceStatusChanged { service: "twitch".to_string(), status: ServiceStatus::Error });
                            return;
                        }
                    }
                }
            };

            match message {
                EventSubMessage::ChatMessage(chat_message) => {
                    // The connected Twitch account (whether the streamer's
                    // own, or — Fase 8b — a dedicated bot account) is
                    // always skipped here, before it's ever shown in the
                    // feed or registered in Personas — Twitch echoes every
                    // message posted to a channel back through this same
                    // subscription, including the character's own replies,
                    // so without this check its own reply would appear
                    // twice: once as the "Personaje" entry `respond_to`
                    // publishes, and again here under whatever account
                    // actually posted it. When no target channel is set,
                    // the connected account is also the only one posting
                    // there, which makes solo testing via real Twitch chat
                    // impossible from that same account —
                    // `TwitchService::send_test_message` exists
                    // specifically to test the reply pipeline without
                    // hitting this.
                    if chat_message.chatter_login.eq_ignore_ascii_case(&connection.channel_login) {
                        tracing::info!(
                            target: "system",
                            "twitch: mensaje propio ignorado (mismo login que la cuenta conectada) — usa \"Probar respuesta\" en la pestaña de Twitch, u otra cuenta, para probar las respuestas automáticas",
                        );
                        continue;
                    }

                    self.event_bus.publish(AppEvent::TwitchMessageReceived {
                        chatter_display_name: chat_message.chatter_display_name.clone(),
                        text: chat_message.text.clone(),
                    });
                    if let Err(err) =
                        self.people.observe_twitch_chatter(&chat_message.chatter_user_id, &chat_message.chatter_display_name).await
                    {
                        tracing::warn!(target: "system", "twitch: no se pudo registrar al chatter en Personas: {err}");
                    }

                    let policy = self.settings().twitch_policy().await.unwrap_or_default();
                    // Only worth the profile DB read when it could actually
                    // change the outcome — see `should_respond`'s doc comment.
                    let mentions_character = policy.enabled && !policy.respond_to_all && policy.mentioned_only && self.mentions_active_character(&chat_message.text).await;
                    if should_respond(&policy, &chat_message, &connection.channel_login, mentions_character) {
                        if self.cooldown.ready(policy.cooldown_seconds) {
                            self.respond_to(&chat_message, effective_broadcaster_id(&connection), &connection.broadcaster_id, &access_token).await;
                        } else {
                            tracing::info!(target: "system", "twitch: mensaje calificaba pero está en cooldown, se omite");
                        }
                    }
                }
                EventSubMessage::Reconnect(url) => match EventSubConnection::connect_to(&url, connection.broadcaster_id.clone()).await {
                    Ok((new_socket, _session_id)) => socket = new_socket,
                    Err(err) => {
                        tracing::warn!(target: "system", "twitch: falló la reconexión solicitada por Twitch: {err}");
                        self.event_bus.publish(AppEvent::ServiceStatusChanged { service: "twitch".to_string(), status: ServiceStatus::Error });
                        return;
                    }
                },
                EventSubMessage::Other => {}
            }
        }
    }
}

/// Which channel EventSub should subscribe to and Helix should post
/// into — the connected account's own id when no target channel is set
/// (reproduces the pre-Fase-8b, "posts in its own channel" behavior
/// exactly), or the resolved target otherwise (see
/// `TwitchService::set_target_channel`). A pure function, kept separate
/// from `establish_connection`/`respond_to` so the fallback logic itself
/// is unit-testable without a live connection.
pub fn effective_broadcaster_id(connection: &TwitchConnectionSettings) -> &str {
    if connection.target_broadcaster_id.is_empty() {
        &connection.broadcaster_id
    } else {
        &connection.target_broadcaster_id
    }
}

/// One line for `Inner::build_extra_instructions`'s emote list — the
/// vision-generated `description` if there is one, else 7TV's `tags`
/// (comma-joined at fetch time), else just the bare name when neither
/// exists (BTTV/FFZ never have tags, and nothing's been captioned yet).
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

fn describe_emote(emote: &CachedEmote) -> String {
    match (&emote.description, &emote.tags) {
        (Some(description), _) if !description.is_empty() => format!("{} — {description}", emote.name),
        (_, Some(tags)) if !tags.is_empty() => format!("{} — {tags}", emote.name),
        _ => emote.name.clone(),
    }
}

fn truncate_for_twitch(text: &str) -> String {
    if text.chars().count() <= MAX_MESSAGE_LEN {
        return text.to_string();
    }
    let truncated: String = text.chars().take(MAX_MESSAGE_LEN.saturating_sub(1)).collect();
    format!("{truncated}…")
}

/// `Arc`-wrapped from the start (not just when `ServiceManager` stores it)
/// because `ManagedService::start` only gets `&self`, but the background
/// message loop it spawns needs an owned, `'static` handle back into this
/// service's state — see `Inner::run_message_loop`'s `self: Arc<Self>`.
#[derive(Clone)]
pub struct TwitchService(Arc<Inner>);

impl TwitchService {
    pub fn new(
        pool: DbPool,
        event_bus: EventBus,
        secrets: Arc<dyn SecretStore>,
        tts: Arc<dyn TTSProvider>,
        people: Arc<dyn PeopleManager>,
        memory: Arc<dyn MemoryManager>,
    ) -> Self {
        Self(Arc::new(Inner {
            pool,
            event_bus,
            secrets,
            tts,
            people,
            memory,
            http: reqwest::Client::new(),
            active_profile_id: RwLock::new(None),
            cooldown: CooldownTracker::new(),
            chatter_context: ChatterContext::new(),
            cancellation: Mutex::new(None),
        }))
    }

    pub async fn set_active_profile(&self, profile_id: Option<String>) {
        *self.0.active_profile_id.write().await = profile_id;
    }

    /// Runs the exact same generation + TTS pipeline `respond_to` uses for
    /// a real chat message, but never touches Helix — no access token or
    /// live EventSub connection required, so this works even before the
    /// user has clicked "Escuchar". Exists because the connected Twitch
    /// account is also the one the character posts as (see
    /// `run_message_loop`'s doc comment on the self-message check), which
    /// makes solo testing via real Twitch chat impossible: this is the
    /// supported way to see/hear a response without a second account.
    pub async fn send_test_message(&self, text: String) -> Result<(), TwitchError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(TwitchError::EmptyResponse);
        }

        self.0.event_bus.publish(AppEvent::TwitchMessageReceived {
            chatter_display_name: "Tú (prueba)".to_string(),
            text: trimmed.to_string(),
        });

        let (profile, reply) = self.0.generate_reply("twitch-test-sender", trimmed, "Tú (prueba)").await?;

        self.0.event_bus.publish(AppEvent::TwitchMessageSent { character_name: profile.name.clone(), text: reply.clone() });
        self.0.spawn_speak(profile, reply);
        Ok(())
    }

    /// Requests a device code — the caller (Tauri command layer) opens
    /// `verification_uri` in the system browser (the `user_code` is
    /// pre-filled in that URL per Twitch's own convention) and then hands
    /// the flow back to `complete_oauth` to poll until the user finishes
    /// authorizing there. Split in two only so this crate never needs a
    /// Tauri dependency to open a browser window itself.
    pub async fn begin_oauth(&self) -> Result<DeviceCodeFlow, TwitchError> {
        auth::request_device_code(&self.0.http, crate::CLIENT_ID).await
    }

    pub async fn complete_oauth(&self, flow: DeviceCodeFlow) -> Result<(), TwitchError> {
        let tokens = auth::poll_for_token(&self.0.http, crate::CLIENT_ID, &flow).await?;

        let helix = HelixClient::new(&self.0.http, crate::CLIENT_ID, &tokens.access_token);
        let user = helix.get_authorized_user().await?;

        self.0
            .settings()
            .set_twitch_tokens(&tokens.access_token, &tokens.refresh_token)
            .await
            .map_err(|err| TwitchError::Settings(err.to_string()))?;
        self.0
            .settings()
            .mark_twitch_connected(&user.login, &user.id, &user.profile_image_url)
            .await
            .map_err(|err| TwitchError::Settings(err.to_string()))?;

        Ok(())
    }

    pub async fn disconnect(&self) -> Result<(), TwitchError> {
        self.0.settings().disconnect_twitch().await.map_err(|err| TwitchError::Settings(err.to_string()))?;
        Ok(())
    }

    /// Resolves `channel_login` via Helix and persists it as the channel
    /// the connected account joins/posts in from now on — Fase 8b's "bot
    /// account" support (see `effective_broadcaster_id`). Empty input
    /// clears both target fields (revert to "posts in its own channel")
    /// without a Helix round trip. Takes effect on the next `start()` —
    /// `establish_connection` reads settings once per run, so a change
    /// while already `Running` needs a stop/start to apply (same as
    /// switching which account is connected).
    pub async fn set_target_channel(&self, channel_login: String) -> Result<TwitchConnectionSettings, TwitchError> {
        let trimmed = channel_login.trim();
        if trimmed.is_empty() {
            return self.0.settings().set_twitch_target_channel("", "").await.map_err(|err| TwitchError::Settings(err.to_string()));
        }

        let Some((access_token, _refresh_token)) =
            self.0.settings().twitch_tokens().await.map_err(|err| TwitchError::Settings(err.to_string()))?
        else {
            return Err(TwitchError::NotConnected);
        };
        let helix = HelixClient::new(&self.0.http, crate::CLIENT_ID, &access_token);
        let user = helix.get_user_by_login(trimmed).await?;

        self.0.settings().set_twitch_target_channel(&user.login, &user.id).await.map_err(|err| TwitchError::Settings(err.to_string()))
    }

    /// The broadcaster_id emotes are fetched/cached/described for — the
    /// channel the character actually posts in, same resolution
    /// `establish_connection`/`respond_to` use. `None` when there's no
    /// connected account at all yet.
    async fn emote_broadcaster_id(&self) -> Result<Option<String>, TwitchError> {
        let connection = self.0.settings().twitch_connection().await.map_err(|err| TwitchError::Settings(err.to_string()))?;
        if !connection.has_tokens {
            return Ok(None);
        }
        let id = effective_broadcaster_id(&connection).to_string();
        Ok(if id.is_empty() { None } else { Some(id) })
    }

    /// Fetches 7TV/BTTV/FFZ for the connected channel and caches the
    /// result — see `emotes::refresh`. Awaited directly rather than
    /// fire-and-forget: three HTTP round trips finish in a couple of
    /// seconds, not long enough to need progress events.
    pub async fn refresh_emotes(&self) -> Result<(i64, i64), TwitchError> {
        let Some(broadcaster_id) = self.emote_broadcaster_id().await? else {
            return Err(TwitchError::NotConnected);
        };
        emotes::refresh(&self.0.pool, &self.0.http, &broadcaster_id).await
    }

    /// Current cached/described counts without fetching anything new —
    /// for the settings dialog to show on open.
    pub async fn emote_status(&self) -> Result<(i64, i64), TwitchError> {
        let Some(broadcaster_id) = self.emote_broadcaster_id().await? else {
            return Ok((0, 0));
        };
        Ok(TwitchEmotesRepository::new(&self.0.pool).count_status(&broadcaster_id).await?)
    }

    /// The cached emote set (name → image URL, plus tags/description) for
    /// the connected channel — used by the frontend to render emotes as
    /// images in the live Twitch feed (`TwitchPage`), same cache
    /// `build_extra_instructions` reads from. Empty when there's no
    /// connected channel yet, not an error.
    pub async fn list_emotes(&self) -> Result<Vec<CachedEmote>, TwitchError> {
        let Some(broadcaster_id) = self.emote_broadcaster_id().await? else {
            return Ok(Vec::new());
        };
        Ok(TwitchEmotesRepository::new(&self.0.pool).list_for_broadcaster(&broadcaster_id).await?)
    }

    /// Validates preconditions (a connected channel, a configured AI
    /// provider) synchronously so the Tauri command can surface a clear
    /// error right away, then detaches the actual captioning loop
    /// (`emotes::caption`) — real progress travels only through the
    /// `TwitchEmoteCaption*` events it publishes, same "fire-and-forget,
    /// events carry the result" shape as `memory_backfill_embeddings`. Uses
    /// the same connection as everyday chat (Configuración > Proveedores
    /// IA) — no separate vision connection to set up.
    pub async fn start_emote_captioning(&self) -> Result<(), TwitchError> {
        let Some(broadcaster_id) = self.emote_broadcaster_id().await? else {
            return Err(TwitchError::NotConnected);
        };
        let llm = self.0.settings().llm_connection().await.map_err(|err| TwitchError::Settings(err.to_string()))?;
        let Some(llm) = llm.filter(|c| !c.base_url.is_empty() && !c.model.is_empty()) else {
            return Err(TwitchError::NoLlmConnection);
        };
        let provider = self.0.settings().build_llm_provider(&self.0.event_bus).await.map_err(|err| TwitchError::Settings(err.to_string()))?;

        let pool = self.0.pool.clone();
        let http = self.0.http.clone();
        let event_bus = self.0.event_bus.clone();
        tokio::spawn(emotes::caption(pool, http, event_bus, broadcaster_id, provider, llm.model));
        Ok(())
    }
}

#[async_trait::async_trait]
impl ManagedService for TwitchService {
    fn name(&self) -> &str {
        "twitch"
    }

    async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (socket, connection, access_token) = self.0.establish_connection().await.map_err(box_err)?;

        let token = CancellationToken::new();
        *self.0.cancellation.lock().await = Some(token.clone());

        let inner = self.0.clone();
        tokio::spawn(async move {
            inner.run_message_loop(socket, connection, access_token, token).await;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn connection(broadcaster_id: &str, target_broadcaster_id: &str) -> TwitchConnectionSettings {
        TwitchConnectionSettings {
            channel_login: "mi_bot".to_string(),
            broadcaster_id: broadcaster_id.to_string(),
            has_tokens: true,
            target_channel_login: if target_broadcaster_id.is_empty() { String::new() } else { "otro_canal".to_string() },
            target_broadcaster_id: target_broadcaster_id.to_string(),
            profile_image_url: String::new(),
        }
    }

    #[test]
    fn falls_back_to_the_connected_accounts_own_id_when_no_target_is_set() {
        assert_eq!(effective_broadcaster_id(&connection("999", "")), "999");
    }

    #[test]
    fn uses_the_target_broadcaster_id_when_one_is_set() {
        assert_eq!(effective_broadcaster_id(&connection("999", "555")), "555");
    }
}
