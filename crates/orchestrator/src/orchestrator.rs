use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Datelike, Local, TimeZone, Weekday};
use futures::stream::BoxStream;
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;

use aivt_database::{Memory, Message as DbMessage, MessageRole as DbMessageRole, Profile, ProfileMood};
use aivt_provider_llm::{ChatChunk, ChatMessage, ChatRequest, ContextBlock, LLMProvider, MessageRole, ProviderError, ToolDefinition};
use aivt_provider_tools::Tool;

use crate::companion::{self, Channel};
use crate::{MoodPromptBuilder, PromptComposer};

/// Working-memory cap until Fase 4's `MemoryManager` does real token-aware
/// trimming (sección 29 del brief) — a simple message-count limit is a
/// reasonable stand-in that keeps context bounded today.
const MAX_HISTORY_MESSAGES: usize = 20;

/// How many of the user's latest messages feed the conversation-pace hint.
const RECENT_USER_MESSAGES: usize = 5;

/// Hard cap on how many times a single `stream_respond` call will call the
/// provider when tools are configured (Fase 10b) — guards against a model
/// that keeps requesting tool calls indefinitely. The last round is always
/// sent with no `tools` attached, forcing a direct answer instead of
/// another request; reaching the cap forwards whatever that produces
/// rather than erroring, since a plain answer beats an opaque failure.
const MAX_TOOL_ROUNDS: usize = 3;

/// The one place "the character thinks". Every channel (Chat now; Call/
/// Twitch/Discord in their fases) builds an `InteractionContext` and calls
/// this — never an `LLMProvider` directly (sección 73 del brief: Principio
/// de Identidad Única). Memory retrieval and channel-specific context
/// (Fase 4+) fold into `stream_respond` later without changing this
/// contract.
pub struct AIOrchestrator {
    provider: Arc<dyn LLMProvider>,
    model: String,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    tools: Vec<Arc<dyn Tool>>,
    channel: Channel,
}

impl AIOrchestrator {
    pub fn new(provider: Arc<dyn LLMProvider>, model: String, temperature: Option<f32>, max_tokens: Option<u32>) -> Self {
        Self { provider, model, temperature, max_tokens, tools: Vec::new(), channel: Channel::Chat }
    }

    /// Which channel the reply goes to (default `Chat`); only changes the
    /// conversation policy text, see `companion::instruction`.
    pub fn with_channel(mut self, channel: Channel) -> Self {
        self.channel = channel;
        self
    }

    /// Enables tool-calling (Fase 10b) — e.g. web search. Kept as a
    /// separate builder method rather than a `new` parameter so the ~25
    /// existing call sites (real channels and tests alike) that only ever
    /// wanted the four original arguments don't need to change; each
    /// channel service decides whether/which tools to attach based on its
    /// own settings before generating a reply.
    pub fn with_tools(mut self, tools: Vec<Arc<dyn Tool>>) -> Self {
        self.tools = tools;
        self
    }

    /// Composes the system prompt from `profile` (sección 12 del brief),
    /// appends recent history, and streams the response back.
    ///
    /// `latest_user_image` is a `data:` URL attached to the *last* message
    /// in `history` (always the just-sent user message at generation time —
    /// see `ChatService::run_generation`). Only that newest image is resent
    /// to the model on every turn; older attachments stay stored and
    /// visible in the UI but aren't replayed into every follow-up request,
    /// which would otherwise blow up token usage fast (documented scope
    /// decision, Fase 3).
    ///
    /// `relevant_memories` is whatever `MemoryManager::search` already
    /// returned for the current message (Fase 4) — the orchestrator only
    /// renders it into the prompt, it never queries memory itself, keeping
    /// this crate free of a dependency on `aivt-memory` (matches
    /// `PromptComposer::compose`'s own doc comment: memories fold in here,
    /// not by changing `compose`'s contract). Tools (Fase 10b) follow the
    /// same principle: this crate never decides *which* tools exist or
    /// whether they're enabled, it only calls the ones it's handed via
    /// `with_tools`.
    ///
    /// `extra_instructions` is free-form text a specific channel needs added
    /// to the system prompt that no other channel wants — e.g. Twitch's
    /// "no Markdown, chat doesn't render it" (`TwitchService::generate_reply`).
    /// `None` for every channel that doesn't need one.
    ///
    /// `language` is the app's current UI language (`SettingsService::language`,
    /// e.g. `"es"`/`"en"`/`"ja"`) — every piece of framework text this
    /// method (and the tools it's given) sends the model follows it:
    /// `PromptComposer`/`PersonalityPromptBuilder`, `MoodPromptBuilder`,
    /// `memory_section`, the tool-usage hint, the date/time line, and the
    /// tool-call fallback/error text below. Only the user's own data
    /// (profile fields, memory content, tool results' factual content)
    /// stays as stored — see each of those functions' own doc comments.
    /// Unrecognized values fall back to Spanish, same as
    /// `SettingsService::DEFAULT_LANGUAGE`. Deliberately consistent across
    /// every prompt fragment (not just the "identity" ones from the first
    /// pass at this) so nothing left in Spanish can nudge the model to
    /// answer in Spanish regardless of the app's language setting.
    pub fn stream_respond(
        &self,
        profile: &Profile,
        history: &[DbMessage],
        latest_user_image: Option<&str>,
        relevant_memories: &[Memory],
        mood: Option<&ProfileMood>,
        extra_instructions: Option<&str>,
        language: &str,
    ) -> BoxStream<'static, Result<ChatChunk, ProviderError>> {
        let mut system_prompt = PromptComposer::compose(profile, language);
        // Conversation policy for this turn: right after who the character is, and
        // repeated after the history below because long histories dilute the top.
        let mut recent_user: Vec<&str> =
            history.iter().rev().filter(|m| m.role == DbMessageRole::User).take(RECENT_USER_MESSAGES).map(|m| m.content.as_str()).collect();
        recent_user.reverse();
        let policy = companion::instruction(self.channel, &recent_user, language);
        system_prompt.push_str("

");
        system_prompt.push_str(&policy);
        // Estado de ánimo y recuerdos NO se pegan al prompt: viajan aparte, como
        // `ContextBlock`, y el `PrivacyGuardProvider` de cada proveedor anexa solo
        // lo que el modo de privacidad de ESE proveedor permite (el cambio
        // automático puede acabar en uno distinto del que se esperaba).
        // `None` for channels that don't fetch mood at all (Twitch/Discord/
        // Llamada, Fase de moods v1 — ver docs/ADR/006-mood.md); a `Some`
        // that happens to be a neutral/decayed-to-neutral mood renders as
        // "" via `MoodPromptBuilder`, so both cases are indistinguishable
        // in the final prompt.
        let mut context = Vec::new();
        if let Some(mood) = mood {
            let mood_text = MoodPromptBuilder::build(mood, language);
            if !mood_text.is_empty() {
                context.push(ContextBlock::Mood(mood_text));
            }
        }
        let memory_all = memory_section(relevant_memories, language);
        if !memory_all.is_empty() {
            let public: Vec<Memory> = relevant_memories.iter().filter(|m| m.scope == aivt_database::MEMORY_SCOPE_PUBLIC).cloned().collect();
            context.push(ContextBlock::Memories { all: memory_all, public_only: memory_section(&public, language) });
        }
        // Ground truth, not tool-gated: live testing found the model asked
        // "¿qué fecha es hoy?" with no place mentioned never even attempted
        // a tool call (its description scopes `get_current_time` to *other*
        // places) and instead answered from training data — a 2024 date.
        // The local date/time costs nothing to compute and is needed on
        // almost every turn, so it's injected directly instead of being
        // something the model has to remember to ask a tool for.
        system_prompt.push_str("\n\n");
        system_prompt.push_str(&current_datetime_line(language));
        if !self.tools.is_empty() {
            system_prompt.push_str("\n\n");
            system_prompt.push_str(&tool_usage_hint(language));
        }
        if let Some(extra) = extra_instructions.filter(|s| !s.is_empty()) {
            system_prompt.push_str("\n\n");
            system_prompt.push_str(extra);
        }
        let mut messages = vec![ChatMessage::text(MessageRole::System, system_prompt)];

        let recent = history.len().saturating_sub(MAX_HISTORY_MESSAGES);
        messages.extend(history[recent..].iter().map(db_message_to_chat_message));

        if let Some(image) = latest_user_image {
            if let Some(last) = messages.last_mut() {
                last.images.push(image.to_string());
            }
        }

        // A second copy of the date/time fact and (when tools are
        // configured) the tool-usage nudge, placed *after* the conversation
        // history instead of relying only on the system prompt at the very
        // top — live testing found the model reliably following an
        // instruction with a short conversation, but ignoring it once ~20
        // messages of casual history had built up, with an otherwise
        // identical prompt. LLMs attend more to recent context than to a
        // system prompt that's now many messages back, so repeating this
        // right before generation (a common mitigation for this exact
        // recency effect) is more reliable than trusting the one at the top
        // alone to still carry weight in a long conversation.
        let mut trailing_reminder = policy;
        trailing_reminder.push_str("\n");
        trailing_reminder.push_str(&current_datetime_line(language));
        if !self.tools.is_empty() {
            trailing_reminder.push('\n');
            trailing_reminder.push_str(&tool_usage_hint(language));
        }
        if let Some(extra) = extra_instructions.filter(|s| !s.is_empty()) {
            trailing_reminder.push('\n');
            trailing_reminder.push_str(extra);
        }
        messages.push(ChatMessage::text(MessageRole::System, trailing_reminder));

        if self.tools.is_empty() {
            // No tools configured — the only difference from before Fase
            // 10b existed is the date/time line above, which fixes a
            // hallucination bug unrelated to tools.
            let request = ChatRequest {
                model: self.model.clone(),
                messages,
                temperature: self.temperature,
                max_tokens: self.max_tokens,
                tools: Vec::new(),
                context,
                contains_stored_memories: false,
            };
            return self.provider.stream_chat(request);
        }

        self.stream_respond_with_tools(messages, context, language.to_string())
    }

    /// The tool-calling loop (Fase 10b) — only reached when at least one
    /// tool is configured. Runs as a detached task feeding an unbounded
    /// channel, the same idiom `OpenAiCompatibleProvider::stream_chat`
    /// already uses (not a new one): dropping the returned stream (e.g.
    /// Llamada's barge-in cancelling a turn mid-search) makes the task's
    /// `tx.send` calls start failing, which is already treated below as
    /// "stop quietly" — no separate cancellation plumbing needed.
    fn stream_respond_with_tools(&self, mut messages: Vec<ChatMessage>, context: Vec<ContextBlock>, language: String) -> BoxStream<'static, Result<ChatChunk, ProviderError>> {
        let (tx, rx) = mpsc::unbounded_channel();
        let provider = self.provider.clone();
        let model = self.model.clone();
        let temperature = self.temperature;
        let max_tokens = self.max_tokens;
        let tool_definitions: Vec<ToolDefinition> = self
            .tools
            .iter()
            .map(|tool| ToolDefinition { name: tool.name().to_string(), description: tool.description().to_string(), parameters_schema: tool.parameters_schema() })
            .collect();
        let tools_by_name: HashMap<String, Arc<dyn Tool>> = self.tools.iter().map(|tool| (tool.name().to_string(), tool.clone())).collect();

        tokio::spawn(async move {
            for round in 0..MAX_TOOL_ROUNDS {
                let is_last_round = round + 1 == MAX_TOOL_ROUNDS;
                let request = ChatRequest {
                    model: model.clone(),
                    messages: messages.clone(),
                    temperature,
                    max_tokens,
                    tools: if is_last_round { Vec::new() } else { tool_definitions.clone() },
                    context: context.clone(),
                    contains_stored_memories: false,
                };

                let mut stream = provider.stream_chat(request);
                let mut requested_tool_calls = Vec::new();

                while let Some(item) = stream.next().await {
                    match item {
                        Ok(chunk) => {
                            if chunk.finish_reason.as_deref() == Some("tool_calls") {
                                requested_tool_calls = chunk.tool_calls;
                                break;
                            }
                            let is_final = chunk.finished;
                            if tx.send(Ok(chunk)).is_err() {
                                return;
                            }
                            if is_final {
                                return;
                            }
                        }
                        Err(err) => {
                            let _ = tx.send(Err(err));
                            return;
                        }
                    }
                }

                if requested_tool_calls.is_empty() {
                    // The provider's stream ended without an explicit
                    // finish (shouldn't happen against a real provider —
                    // see `openai_compatible.rs`'s own `Ok(None)` handling
                    // — but nothing left to do either way).
                    return;
                }

                messages.push(ChatMessage::assistant_tool_calls(requested_tool_calls.clone()));
                for call in requested_tool_calls {
                    let result = match tools_by_name.get(&call.name) {
                        Some(tool) => match tool.execute(&call.arguments).await {
                            Ok(result) => {
                                tracing::info!(target: "system", "orchestrator: herramienta '{}' ejecutada con argumentos {} — resultado: {result}", call.name, call.arguments);
                                result
                            }
                            Err(err) => {
                                tracing::warn!(target: "system", "orchestrator: la herramienta '{}' falló: {err}", call.name);
                                tool_execution_error_message(&err.to_string(), &language)
                            }
                        },
                        None => {
                            tracing::warn!(target: "system", "orchestrator: el modelo pidió una herramienta desconocida: '{}'", call.name);
                            unknown_tool_message(&language).to_string()
                        }
                    };
                    messages.push(ChatMessage::tool_result(call.id, result));
                }
            }
        });

        Box::pin(UnboundedReceiverStream::new(rx))
    }
}

/// Renders retrieved memories as a natural-language section appended to the
/// system prompt — empty string (never appended) when there's nothing
/// relevant, so a profile with no memory yet produces the exact same prompt
/// as before Fase 4.
///
/// Only ever sends `summary` (the compact "resumen") — `content` ("detalle")
/// never reaches the model. An earlier version tried to include `content`
/// too as a secondary line the model was told to use "only if the user
/// asks for more depth", asking a text prompt to do conditional attention
/// on its own — this predates Fase 10b's tool-calling below, and in
/// practice it didn't work regardless: probado en vivo, el modelo seguía
/// sin poder recuperar el detalle de forma fiable. Pedido explícito del
/// usuario tras esa prueba: dejar de intentar que la IA "lea" el detalle y
/// usarlo en cambio como contexto extendido para que el propio usuario
/// organice mejor sus memorias en la Memoria tab — nunca viaja al chat, ni
/// siquiera como algo que una herramienta podría buscar. Falls back to
/// `content` only for memories that have no `summary` at all (created
/// before the redesign).
///
/// `lang` only localizes the section header below — `title`/`summary`/
/// `content` are the user's own memory data and always pass through as
/// stored, same principle as the profile's free-text fields in
/// `PromptComposer`.
fn memory_section(memories: &[Memory], lang: &str) -> String {
    if memories.is_empty() {
        return String::new();
    }

    let lines: Vec<String> = memories
        .iter()
        .map(|m| {
            let body = m.summary.as_deref().filter(|s| !s.trim().is_empty()).unwrap_or(&m.content);
            format!("- {}: {body}", m.title)
        })
        .collect();
    format!("{}:\n{}", memory_section_header(lang), lines.join("\n"))
}

fn memory_section_header(lang: &str) -> &'static str {
    match lang {
        "en" => "Relevant memories from previous conversations",
        "pt" => "Lembranças relevantes de conversas anteriores",
        "ja" => "過去の会話からの関連する記憶",
        "ko" => "이전 대화에서의 관련 기억",
        "zh" => "以往对话中的相关记忆",
        _ => "Recuerdos relevantes de conversaciones anteriores",
    }
}

/// A nudge appended to the system prompt only when tools are actually
/// configured (Fase 10b) — a live-testing session found the model (a small
/// local one, run via Ollama) simply never attempting a tool call for a
/// question that clearly needed one ("qué hora es en Chile"), guessing an
/// answer instead even though the tool was correctly offered in the
/// request. The tool definitions themselves already tell the model *how*
/// to call them; what a less naturally agentic model needs spelled out is
/// *when* it should bother — this is the standard fix for that, not a
/// workaround for a bug (`stream_respond`'s own tests confirm the tools
/// were being offered correctly before this was added).
///
/// `lang` follows `SettingsService::language` (falls back to Spanish),
/// same convention as `PromptComposer` — see its doc comment. Localizing
/// this (and the rest of this file's framework text) was a deliberate
/// follow-up request: a stray Spanish paragraph in an otherwise-localized
/// prompt risks nudging the model to answer in Spanish regardless of the
/// app's language setting.
fn tool_usage_hint(lang: &str) -> String {
    match lang {
        "en" => {
            "You have tools available to answer with real data instead of guessing. \
             You've already been given the real current date and time (previous message) \
             — you don't need any tool for that, use that data directly. For everything \
             else you can't know for certain from memory — the time or date in ANOTHER \
             country or city, the weather, news, or any current fact — always use the \
             tools. Never make that kind of data up: if there's a tool that can get it, \
             call it first."
        }
        "pt" => {
            "Você tem ferramentas disponíveis para responder com dados reais em vez de \
             adivinhar. Já te foi dada a data e hora atuais reais (mensagem anterior) — \
             você não precisa de nenhuma ferramenta para isso, use esse dado diretamente. \
             Para tudo o mais que você não pode saber de memória com certeza — a hora ou \
             data em OUTRO país ou cidade, o clima, notícias, ou qualquer fato atual — \
             sempre use as ferramentas. Nunca invente esse tipo de dado: se há uma \
             ferramenta que pode conseguir isso, chame-a primeiro."
        }
        "ja" => {
            "推測ではなく実データで答えるためのツールが利用できる。現在の本当の日付と時刻はすでに渡してある \
             （前のメッセージ）ので、それにはツールを使う必要はなく、その情報をそのまま使うこと。それ以外の \
             確実に知らないこと — ユーザーとは別の国や都市の時刻・日付、天気、ニュース、その他の最新の事実 \
             — には必ずツールを使うこと。そのような情報を絶対に作り話しないこと。取得できるツールがあれば、 \
             まずそれを呼び出すこと。"
        }
        "ko" => {
            "추측 대신 실제 데이터로 답할 수 있는 도구가 있다. 현재의 실제 날짜와 시간은 이미 전달했으므로 \
             (이전 메시지), 그것을 위해 도구를 쓸 필요는 없으며 그 데이터를 그대로 사용할 것. 확실히 알 수 \
             없는 다른 모든 것 — 사용자와 다른 나라나 도시의 시간/날짜, 날씨, 뉴스, 기타 최신 사실 — 에는 \
             항상 도구를 사용할 것. 그런 정보를 절대 지어내지 말 것. 가져올 수 있는 도구가 있다면 먼저 그것을 \
             호출할 것."
        }
        "zh" => {
            "你可以使用工具来给出真实数据，而不是靠猜测。当前真实的日期和时间已经提供给你了（上一条消息），\
             这方面不需要使用任何工具，直接使用那个数据即可。对于其他任何你无法确定记住的信息——用户所在地\
             以外的其他国家或城市的时间/日期、天气、新闻，或任何时事——始终使用工具。绝不要凭空编造这类数据：\
             如果有工具能获取它，就先调用那个工具。"
        }
        _ => {
            "Tenés herramientas disponibles para responder con datos reales en vez de \
             adivinar. Ya se te dio la fecha y hora actual reales (mensaje anterior) — \
             no necesitás ninguna herramienta para eso, usá ese dato directamente. Para \
             todo lo demás que no podés saber de memoria con certeza — la hora o fecha \
             en OTRO país o ciudad, el clima, noticias, o cualquier hecho actual — usá \
             las herramientas siempre. Nunca inventes ese tipo de dato: si hay una \
             herramienta que puede conseguirlo, llamala primero."
        }
    }
    .to_string()
}

/// The real current date/time, injected as ground truth the model doesn't
/// have to remember to ask a tool for — see the call sites in
/// `stream_respond` for why this exists. Uses the system's local timezone
/// (this runs on the user's own machine, so "local" already matches their
/// actual location) rather than UTC, since the whole point is answering
/// "qué hora/fecha es" without the model having to reason about an offset.
fn current_datetime_line(lang: &str) -> String {
    // Best-effort: live testing found a bare "clima" (no city) made the
    // model search for "clima en mi ubicación" — a query with no real
    // location in it — and Brave happily returned results for a
    // different city (Buenos Aires) that the model then presented as
    // fact. The system's own configured IANA zone (e.g.
    // "America/Santiago") is a real signal of roughly where the user is,
    // already available for free — better to hand it over than let the
    // model guess a city out of thin air. `get_timezone` can fail on some
    // platforms/sandboxes; when it does, the line just omits the hint
    // rather than blocking the response on it.
    let tz_name = iana_time_zone::get_timezone().ok();
    format_current_datetime_line(Local::now(), tz_name.as_deref(), lang)
}

/// Split out from `current_datetime_line` so a test can pin the instant
/// (and the timezone name) instead of racing the real clock/machine — same
/// pattern as `aivt_provider_tools::CurrentTimeTool`'s
/// `format_current_time_at`. The numeric date/time itself (`%d/%m/%Y`,
/// 24h `%H:%M`) stays in the same unambiguous digits-only format
/// regardless of `lang` — only the surrounding prose and the weekday name
/// are localized; reformatting digits per locale would add ambiguity
/// (MM/DD vs. DD/MM) for no real benefit over what's already
/// language-neutral.
fn format_current_datetime_line<Tz: TimeZone>(now: DateTime<Tz>, tz_name: Option<&str>, lang: &str) -> String
where
    Tz::Offset: std::fmt::Display,
{
    let mut line = match lang {
        "en" => format!(
            "Real, current fact (not from your training data, don't question it): today is \
             {}, {} and the time is {} (user's local time, UTC{}). Use this directly if \
             asked for the date, the day of the week, or the current time — never answer \
             with a different date than this.",
            weekday_name(now.weekday(), lang),
            now.format("%d/%m/%Y"),
            now.format("%H:%M"),
            now.format("%:z"),
        ),
        "pt" => format!(
            "Fato real e atual (não vem do seu treinamento, não questione): hoje é {}, {} e \
             são {} (hora local do usuário, UTC{}). Use isso diretamente se perguntarem a \
             data, o dia da semana ou a hora atual — nunca responda com uma data diferente \
             desta.",
            weekday_name(now.weekday(), lang),
            now.format("%d/%m/%Y"),
            now.format("%H:%M"),
            now.format("%:z"),
        ),
        "ja" => format!(
            "これは実際の最新の事実であり（学習データではないので疑わないこと）、今日は{}、{}、現在時刻は{}\
             （ユーザーのローカル時刻、UTC{}）。日付・曜日・現在時刻を聞かれたら、これをそのまま使うこと — \
             絶対にこれと違う日付を答えないこと。",
            weekday_name(now.weekday(), lang),
            now.format("%d/%m/%Y"),
            now.format("%H:%M"),
            now.format("%:z"),
        ),
        "ko" => format!(
            "이것은 실제 최신 사실이며(학습 데이터가 아니므로 의심하지 말 것), 오늘은 {}, {}이고 현재 \
             시간은 {}(사용자의 현지 시간, UTC{})이다. 날짜, 요일, 현재 시각을 물어보면 이 정보를 그대로 \
             사용할 것 — 절대 이것과 다른 날짜로 답하지 말 것.",
            weekday_name(now.weekday(), lang),
            now.format("%d/%m/%Y"),
            now.format("%H:%M"),
            now.format("%:z"),
        ),
        "zh" => format!(
            "这是真实的最新事实（不是来自你的训练数据，不要质疑它）：今天是{}，{}，现在时间是{}（用户本地\
             时间，UTC{}）。如果被问到日期、星期几或现在的时间，请直接使用这个信息——绝不要给出与此不同的\
             日期。",
            weekday_name(now.weekday(), lang),
            now.format("%d/%m/%Y"),
            now.format("%H:%M"),
            now.format("%:z"),
        ),
        _ => format!(
            "Dato real y actual (no es de tu entrenamiento, no lo cuestiones): hoy es \
             {}, {} y son las {} (hora local del usuario, UTC{}). Usá esto directamente \
             si te preguntan la fecha, el día de la semana o la hora de ahora mismo — \
             nunca respondas con una fecha distinta a esta.",
            weekday_name(now.weekday(), lang),
            now.format("%d/%m/%Y"),
            now.format("%H:%M"),
            now.format("%:z"),
        ),
    };
    if let Some(name) = tz_name {
        line.push_str(&timezone_hint(name, lang));
    }
    line
}

fn timezone_hint(name: &str, lang: &str) -> String {
    match lang {
        "en" => format!(
            " The user's system timezone: {name} — this is your only real clue to where \
             they live. If they ask about the weather or anything else that depends on \
             their location and don't give you a city, use this timezone as a guide (or \
             ask them for the exact city if you need more precision) — never make up or \
             assume a city nobody mentioned, or search for it as literally 'my location'."
        ),
        "pt" => format!(
            " Fuso horário do sistema do usuário: {name} — é sua única pista real de onde \
             ele mora. Se ele perguntar sobre o clima ou qualquer outra coisa que dependa \
             da localização e não disser uma cidade, oriente-se por esse fuso horário (ou \
             peça a cidade exata se precisar de mais precisão) — nunca invente nem \
             presuma uma cidade que ninguém mencionou, nem busque por ela como 'minha \
             localização' literalmente."
        ),
        "ja" => format!(
            " ユーザーのシステムのタイムゾーン: {name} — これはユーザーの居場所を知る唯一の実際の手がかりだ。\
             天気など場所に依存する質問をされて都市名を教えてもらえない場合は、このタイムゾーンを目安にすること\
             （より正確な情報が必要なら正確な都市名を尋ねること）。誰も言っていない都市を勝手に決めつけたり、\
             文字通り「自分の場所」として検索したりしないこと。"
        ),
        "ko" => format!(
            " 사용자 시스템의 시간대: {name} — 이는 사용자가 어디 사는지 알 수 있는 유일한 실제 단서다. \
             날씨 등 위치에 따라 달라지는 질문을 받았는데 도시를 알려주지 않았다면, 이 시간대를 참고할 것\
             (더 정확한 정보가 필요하면 정확한 도시를 물어볼 것). 아무도 언급하지 않은 도시를 지어내거나 \
             가정하지 말고, 문자 그대로 '내 위치'로 검색하지도 말 것."
        ),
        "zh" => format!(
            " 用户系统的时区：{name}——这是你了解用户所在地的唯一真实线索。如果用户询问天气等依赖位置的问题\
             却没有告诉你城市，就以这个时区作为参考（如果需要更精确的信息，可以直接问用户具体城市）——绝不要\
             编造或假设一个没人提到过的城市，也不要照字面意思搜索“我的位置”。"
        ),
        _ => format!(
            " Zona horaria del sistema del usuario: {name} — es tu única pista real de \
             dónde vive. Si te pregunta por el clima o cualquier otra cosa que dependa de \
             su ubicación y no te dice una ciudad, orientate con esta zona (o pedile la \
             ciudad exacta si necesitás más precisión) — nunca inventes ni asumas una \
             ciudad que nadie mencionó, ni la busques como 'mi ubicación' literalmente."
        ),
    }
}

fn weekday_name(day: Weekday, lang: &str) -> &'static str {
    match lang {
        "en" => match day {
            Weekday::Mon => "Monday",
            Weekday::Tue => "Tuesday",
            Weekday::Wed => "Wednesday",
            Weekday::Thu => "Thursday",
            Weekday::Fri => "Friday",
            Weekday::Sat => "Saturday",
            Weekday::Sun => "Sunday",
        },
        "pt" => match day {
            Weekday::Mon => "segunda-feira",
            Weekday::Tue => "terça-feira",
            Weekday::Wed => "quarta-feira",
            Weekday::Thu => "quinta-feira",
            Weekday::Fri => "sexta-feira",
            Weekday::Sat => "sábado",
            Weekday::Sun => "domingo",
        },
        "ja" => match day {
            Weekday::Mon => "月曜日",
            Weekday::Tue => "火曜日",
            Weekday::Wed => "水曜日",
            Weekday::Thu => "木曜日",
            Weekday::Fri => "金曜日",
            Weekday::Sat => "土曜日",
            Weekday::Sun => "日曜日",
        },
        "ko" => match day {
            Weekday::Mon => "월요일",
            Weekday::Tue => "화요일",
            Weekday::Wed => "수요일",
            Weekday::Thu => "목요일",
            Weekday::Fri => "금요일",
            Weekday::Sat => "토요일",
            Weekday::Sun => "일요일",
        },
        "zh" => match day {
            Weekday::Mon => "星期一",
            Weekday::Tue => "星期二",
            Weekday::Wed => "星期三",
            Weekday::Thu => "星期四",
            Weekday::Fri => "星期五",
            Weekday::Sat => "星期六",
            Weekday::Sun => "星期日",
        },
        _ => match day {
            Weekday::Mon => "lunes",
            Weekday::Tue => "martes",
            Weekday::Wed => "miércoles",
            Weekday::Thu => "jueves",
            Weekday::Fri => "viernes",
            Weekday::Sat => "sábado",
            Weekday::Sun => "domingo",
        },
    }
}

fn tool_execution_error_message(err: &str, lang: &str) -> String {
    match lang {
        "en" => format!("Error running the tool: {err}"),
        "pt" => format!("Erro ao executar a ferramenta: {err}"),
        "ja" => format!("ツールの実行エラー: {err}"),
        "ko" => format!("도구 실행 오류: {err}"),
        "zh" => format!("工具执行出错：{err}"),
        _ => format!("Error ejecutando la herramienta: {err}"),
    }
}

fn unknown_tool_message(lang: &str) -> &'static str {
    match lang {
        "en" => "Unknown tool.",
        "pt" => "Ferramenta desconhecida.",
        "ja" => "不明なツールです。",
        "ko" => "알 수 없는 도구입니다.",
        "zh" => "未知工具。",
        _ => "Herramienta desconocida.",
    }
}

fn db_message_to_chat_message(message: &DbMessage) -> ChatMessage {
    ChatMessage::text(
        match message.role {
            DbMessageRole::User => MessageRole::User,
            DbMessageRole::Assistant => MessageRole::Assistant,
            DbMessageRole::System => MessageRole::System,
        },
        message.content.clone(),
    )
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use async_trait::async_trait;

    use aivt_database::{Message, MessageRole as DbMessageRole, MessageStatus, ProfileIdentity, ProfilePersonality};
    use aivt_provider_llm::ToolCall;
    use aivt_provider_tools::ToolError;
    use aivt_testing::{text_response, tool_call_response, MockLLMProvider};
    use futures::StreamExt;

    use super::*;

    /// A canned `Tool` for exercising `stream_respond_with_tools` without
    /// any real network call — records every call it receives so tests can
    /// assert on the exact arguments the model produced.
    struct FakeSearchTool {
        result: &'static str,
        calls: Arc<AtomicUsize>,
    }

    impl FakeSearchTool {
        fn new(result: &'static str) -> Self {
            Self { result, calls: Arc::new(AtomicUsize::new(0)) }
        }
    }

    #[async_trait]
    impl Tool for FakeSearchTool {
        fn name(&self) -> &str {
            "web_search"
        }

        fn description(&self) -> &str {
            "busca en la web"
        }

        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object", "properties": {"query": {"type": "string"}}})
        }

        async fn execute(&self, _arguments_json: &str) -> Result<String, ToolError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.result.to_string())
        }
    }

    fn sample_profile() -> Profile {
        Profile {
            id: "p1".to_string(),
            name: "Ciel".to_string(),
            nickname: None,
            avatar_path: None,
            description: None,
            age_fictional: None,
            pronouns: None,
            occupation: None,
            species: None,
            accent_color: None,
            theme: None,
            is_archived: false,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            identity: ProfileIdentity::default(),
            personality: ProfilePersonality::default(),
            voice: Default::default(),
            avatar_idle_path: None,
            avatar_thinking_path: None,
            avatar_talking_path: None,
        }
    }

    fn sample_message(role: DbMessageRole, content: &str) -> Message {
        Message {
            id: "m1".to_string(),
            conversation_id: "c1".to_string(),
            role,
            content: content.to_string(),
            status: MessageStatus::Complete,
            provider_id: None,
            model: None,
            input_tokens: None,
            output_tokens: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn streams_the_mock_providers_response_back() {
        let mock = Arc::new(MockLLMProvider::with_response(["Hola", " mundo"]));
        let orchestrator = AIOrchestrator::new(mock.clone(), "mock-model".to_string(), None, None);

        let chunks: Vec<_> = orchestrator
            .stream_respond(&sample_profile(), &[], None, &[], None, None, "es")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        let text: String = chunks.iter().map(|c| c.delta.as_str()).collect();
        assert_eq!(text, "Hola mundo");
        assert!(chunks.last().unwrap().finished);
    }

    #[tokio::test]
    async fn sends_a_system_prompt_built_from_the_profile() {
        let mock = Arc::new(MockLLMProvider::with_response(["ok"]));
        let orchestrator = AIOrchestrator::new(mock.clone(), "mock-model".to_string(), None, None);

        let _ = orchestrator.stream_respond(&sample_profile(), &[], None, &[], None, None, "es").collect::<Vec<_>>().await;

        let requests = mock.received_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].messages[0].role, MessageRole::System);
        assert!(requests[0].messages[0].content.starts_with("Eres Ciel."));
    }

    #[tokio::test]
    async fn the_conversation_policy_is_in_the_system_prompt_and_repeated_after_the_history() {
        let mock = Arc::new(MockLLMProvider::with_response(["ok"]));
        let orchestrator = AIOrchestrator::new(mock.clone(), "mock-model".to_string(), None, None).with_channel(Channel::Call);
        let history = vec![sample_message(DbMessageRole::User, "jajaja sii")];

        let _ = orchestrator.stream_respond(&sample_profile(), &history, None, &[], None, None, "es").collect::<Vec<_>>().await;

        let messages = &mock.received_requests()[0].messages;
        assert!(messages[0].content.contains("conversar y acompañar"));
        assert!(messages[0].content.contains("conversación hablada"));
        assert!(messages[0].content.contains("frase corta"));
        assert!(messages.last().unwrap().content.contains("conversar y acompañar"));
    }

    #[tokio::test]
    async fn includes_conversation_history_after_the_system_prompt() {
        let mock = Arc::new(MockLLMProvider::with_response(["ok"]));
        let orchestrator = AIOrchestrator::new(mock.clone(), "mock-model".to_string(), None, None);
        let history = vec![sample_message(DbMessageRole::User, "Hola"), sample_message(DbMessageRole::Assistant, "¡Hola!")];

        let _ = orchestrator.stream_respond(&sample_profile(), &history, None, &[], None, None, "es").collect::<Vec<_>>().await;

        let requests = mock.received_requests();
        // System prompt + 2 history messages + the trailing date/time reminder.
        assert_eq!(requests[0].messages.len(), 4);
        assert_eq!(requests[0].messages[1].role, MessageRole::User);
        assert_eq!(requests[0].messages[1].content, "Hola");
        assert_eq!(requests[0].messages[2].role, MessageRole::Assistant);
    }

    #[tokio::test]
    async fn truncates_history_to_the_most_recent_messages() {
        let mock = Arc::new(MockLLMProvider::with_response(["ok"]));
        let orchestrator = AIOrchestrator::new(mock.clone(), "mock-model".to_string(), None, None);
        let history: Vec<_> = (0..30).map(|i| sample_message(DbMessageRole::User, &format!("msg-{i}"))).collect();

        let _ = orchestrator.stream_respond(&sample_profile(), &history, None, &[], None, None, "es").collect::<Vec<_>>().await;

        let requests = mock.received_requests();
        // System prompt + last 20 messages + the trailing date/time reminder.
        let messages = &requests[0].messages;
        assert_eq!(messages.len(), 22);
        assert_eq!(messages[1].content, "msg-10");
        assert_eq!(messages[messages.len() - 2].content, "msg-29");
    }

    #[tokio::test]
    async fn propagates_provider_errors() {
        let mock = Arc::new(MockLLMProvider::with_error(ProviderError::ConnectionRefused));
        let orchestrator = AIOrchestrator::new(mock, "mock-model".to_string(), None, None);

        let results: Vec<_> = orchestrator.stream_respond(&sample_profile(), &[], None, &[], None, None, "es").collect().await;
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0], Err(ProviderError::ConnectionRefused)));
    }

    #[tokio::test]
    async fn attaches_the_latest_image_only_to_the_last_message() {
        let mock = Arc::new(MockLLMProvider::with_response(["ok"]));
        let orchestrator = AIOrchestrator::new(mock.clone(), "mock-model".to_string(), None, None);
        let history = vec![sample_message(DbMessageRole::User, "Hola"), sample_message(DbMessageRole::Assistant, "¡Hola!")];

        let _ = orchestrator
            .stream_respond(&sample_profile(), &history, Some("data:image/png;base64,AAAA"), &[], None, None, "es")
            .collect::<Vec<_>>()
            .await;

        let requests = mock.received_requests();
        // system, "Hola" (no image), "¡Hola!" (the last message — gets the image).
        assert!(requests[0].messages[1].images.is_empty());
        assert_eq!(requests[0].messages[2].images, vec!["data:image/png;base64,AAAA".to_string()]);
    }

    fn sample_memory(content: &str) -> Memory {
        Memory {
            id: "mem1".to_string(),
            profile_id: "p1".to_string(),
            title: "Café".to_string(),
            content: content.to_string(),
            summary: None,
            category: "fact".to_string(),
            importance: 50,
            confidence: 80,
            source: "chat".to_string(),
            source_id: None,
            is_pinned: false,
            scope: "private".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            last_accessed_at: None,
            access_count: 0,
        }
    }

    /// El mensaje de sistema que de verdad le llegaría a un proveedor con modo
    /// de privacidad "completo": el prompt más el contexto (recuerdos, ánimo) que
    /// el orquestador manda aparte para que cada proveedor reciba solo lo suyo.
    fn full_privacy_system_prompt(request: &ChatRequest) -> String {
        let sent = aivt_provider_llm::PrivacyGuardProvider::apply(aivt_provider_llm::PrivacyMode::Full, request.clone()).unwrap();
        sent.messages[0].content.clone()
    }

    #[tokio::test]
    async fn relevant_memories_are_appended_to_the_system_prompt() {
        let mock = Arc::new(MockLLMProvider::with_response(["ok"]));
        let orchestrator = AIOrchestrator::new(mock.clone(), "mock-model".to_string(), None, None);
        let memories = vec![sample_memory("Le encanta el café")];

        let _ = orchestrator.stream_respond(&sample_profile(), &[], None, &memories, None, None, "es").collect::<Vec<_>>().await;

        let requests = mock.received_requests();
        let prompt = full_privacy_system_prompt(&requests[0]);
        assert!(prompt.contains("Recuerdos relevantes"));
        assert!(prompt.contains("Café"));
        assert!(prompt.contains("Le encanta el café"));
        // Y viajan aparte, no pegados al prompt: así cada proveedor decide.
        assert!(!requests[0].messages[0].content.contains("Le encanta el café"));
    }

    #[tokio::test]
    async fn private_and_public_memories_are_offered_separately_so_a_provider_can_get_only_the_public_ones() {
        let mock = Arc::new(MockLLMProvider::with_response(["ok"]));
        let orchestrator = AIOrchestrator::new(mock.clone(), "mock-model".to_string(), None, None);
        let private = sample_memory("Su contraseña de wifi es secreta");
        let public = Memory { scope: "public".to_string(), ..sample_memory("Le gusta el café") };

        let _ = orchestrator.stream_respond(&sample_profile(), &[], None, &[private, public], None, None, "es").collect::<Vec<_>>().await;

        let request = mock.received_requests().remove(0);
        let public_only = aivt_provider_llm::PrivacyGuardProvider::apply(aivt_provider_llm::PrivacyMode::PublicOnly, request.clone()).unwrap();
        let text = &public_only.messages[0].content;
        assert!(text.contains("Le gusta el café"));
        assert!(!text.contains("contraseña de wifi"));

        let minimal = aivt_provider_llm::PrivacyGuardProvider::apply(aivt_provider_llm::PrivacyMode::Minimal, request.clone()).unwrap();
        assert!(!minimal.messages[0].content.contains("Le gusta el café"));
        assert!(full_privacy_system_prompt(&request).contains("contraseña de wifi"));
    }

    #[tokio::test]
    async fn content_never_reaches_the_model_when_a_summary_is_present() {
        // Reverted: an earlier version also sent `content` as a secondary
        // "use only if asked" line, but that didn't hold up in live use —
        // the model still couldn't reliably recover the detail. Pedido del
        // usuario: `content` stays Memoria-tab-only context, never sent.
        let mock = Arc::new(MockLLMProvider::with_response(["ok"]));
        let orchestrator = AIOrchestrator::new(mock.clone(), "mock-model".to_string(), None, None);
        let memory = Memory {
            summary: Some("Le gusta el café".to_string()),
            ..sample_memory("Un párrafo mucho más largo y detallado sobre sus gustos de café")
        };

        let _ = orchestrator.stream_respond(&sample_profile(), &[], None, &[memory], None, None, "es").collect::<Vec<_>>().await;

        let prompt = &full_privacy_system_prompt(&mock.received_requests()[0]);
        assert!(prompt.contains("Le gusta el café"));
        assert!(!prompt.contains("Un párrafo mucho más largo"));
    }

    #[tokio::test]
    async fn content_is_used_as_a_fallback_when_there_is_no_summary_at_all() {
        let mock = Arc::new(MockLLMProvider::with_response(["ok"]));
        let orchestrator = AIOrchestrator::new(mock.clone(), "mock-model".to_string(), None, None);
        // No summary at all (e.g. a memory created before the redesign) —
        // content is the only thing there is to show.
        let memory = sample_memory("Le encanta el café");

        let _ = orchestrator.stream_respond(&sample_profile(), &[], None, &[memory], None, None, "es").collect::<Vec<_>>().await;

        let prompt = &full_privacy_system_prompt(&mock.received_requests()[0]);
        assert!(prompt.contains("Le encanta el café"));
    }

    #[tokio::test]
    async fn no_memories_means_no_memory_section_at_all() {
        let mock = Arc::new(MockLLMProvider::with_response(["ok"]));
        let orchestrator = AIOrchestrator::new(mock.clone(), "mock-model".to_string(), None, None);

        let _ = orchestrator.stream_respond(&sample_profile(), &[], None, &[], None, None, "es").collect::<Vec<_>>().await;

        let requests = mock.received_requests();
        assert!(!requests[0].messages[0].content.contains("Recuerdos relevantes"));
    }

    #[tokio::test]
    async fn a_non_neutral_mood_is_reflected_in_the_system_prompt() {
        let mock = Arc::new(MockLLMProvider::with_response(["ok"]));
        let orchestrator = AIOrchestrator::new(mock.clone(), "mock-model".to_string(), None, None);
        let mood = aivt_database::ProfileMood {
            profile_id: "p1".to_string(),
            valence: 90,
            energy: 90,
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        };

        let _ = orchestrator.stream_respond(&sample_profile(), &[], None, &[], Some(&mood), None, "es").collect::<Vec<_>>().await;

        let requests = mock.received_requests();
        assert!(full_privacy_system_prompt(&requests[0]).contains("muy buen humor"));
    }

    #[tokio::test]
    async fn no_mood_argument_means_no_mood_section_at_all() {
        let mock = Arc::new(MockLLMProvider::with_response(["ok"]));
        let orchestrator = AIOrchestrator::new(mock.clone(), "mock-model".to_string(), None, None);

        let _ = orchestrator.stream_respond(&sample_profile(), &[], None, &[], None, None, "es").collect::<Vec<_>>().await;

        let requests = mock.received_requests();
        assert!(!requests[0].messages[0].content.contains("En este momento está"));
    }

    #[tokio::test]
    async fn a_neutral_mood_renders_no_mood_section_either() {
        let mock = Arc::new(MockLLMProvider::with_response(["ok"]));
        let orchestrator = AIOrchestrator::new(mock.clone(), "mock-model".to_string(), None, None);
        let mood = aivt_database::ProfileMood {
            profile_id: "p1".to_string(),
            valence: 50,
            energy: 50,
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        };

        let _ = orchestrator.stream_respond(&sample_profile(), &[], None, &[], Some(&mood), None, "es").collect::<Vec<_>>().await;

        let requests = mock.received_requests();
        assert!(!requests[0].messages[0].content.contains("En este momento está"));
    }

    #[tokio::test]
    async fn with_no_tools_configured_behaves_exactly_as_before() {
        let mock = Arc::new(MockLLMProvider::with_response(["ok"]));
        let orchestrator = AIOrchestrator::new(mock.clone(), "mock-model".to_string(), None, None);

        let _ = orchestrator.stream_respond(&sample_profile(), &[], None, &[], None, None, "es").collect::<Vec<_>>().await;

        // Exactly one call, and it never even mentions tools.
        assert_eq!(mock.received_requests().len(), 1);
        assert!(mock.received_requests()[0].tools.is_empty());
        assert!(!mock.received_requests()[0].messages[0].content.contains("Tenés herramientas disponibles"));
    }

    #[tokio::test]
    async fn with_tools_configured_the_system_prompt_nudges_the_model_to_use_them() {
        let mock = Arc::new(MockLLMProvider::with_response(["ok"]));
        let tool = Arc::new(FakeSearchTool::new("resultado"));
        let orchestrator = AIOrchestrator::new(mock.clone(), "mock-model".to_string(), None, None).with_tools(vec![tool]);

        let _ = orchestrator.stream_respond(&sample_profile(), &[], None, &[], None, None, "es").collect::<Vec<_>>().await;

        assert!(mock.received_requests()[0].messages[0].content.contains("Tenés herramientas disponibles"));
    }

    #[tokio::test]
    async fn with_tools_configured_the_hint_is_repeated_after_a_long_history() {
        // Live testing found the model reliably calling a tool with a
        // short conversation, but never attempting one with ~20 messages
        // of history and an otherwise-identical prompt — a repeated
        // reminder placed after history (not just in the distant system
        // prompt) is the fix; this pins that it's actually the *last*
        // message the provider receives, where it carries the most
        // attention weight.
        let mock = Arc::new(MockLLMProvider::with_response(["ok"]));
        let tool = Arc::new(FakeSearchTool::new("resultado"));
        let orchestrator = AIOrchestrator::new(mock.clone(), "mock-model".to_string(), None, None).with_tools(vec![tool]);
        let history: Vec<_> = (0..20).map(|i| sample_message(DbMessageRole::User, &format!("msg-{i}"))).collect();

        let _ = orchestrator.stream_respond(&sample_profile(), &history, None, &[], None, None, "es").collect::<Vec<_>>().await;

        let messages = &mock.received_requests()[0].messages;
        let last = messages.last().unwrap();
        assert_eq!(last.role, MessageRole::System);
        assert!(last.content.contains("Tenés herramientas disponibles"));
    }

    #[tokio::test]
    async fn a_requested_tool_call_is_executed_and_its_result_is_sent_back() {
        let tool_call = ToolCall { id: "call_1".to_string(), name: "web_search".to_string(), arguments: r#"{"query":"clima en Bogotá"}"#.to_string() };
        let mock = Arc::new(MockLLMProvider::with_call_sequence([tool_call_response(vec![tool_call]), text_response(["18°C, nublado"])]));
        let tool = Arc::new(FakeSearchTool::new("18°C, nublado, humedad 80%"));
        let orchestrator = AIOrchestrator::new(mock.clone(), "mock-model".to_string(), None, None).with_tools(vec![tool.clone()]);

        let chunks: Vec<_> = orchestrator
            .stream_respond(&sample_profile(), &[], None, &[], None, None, "es")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        let text: String = chunks.iter().map(|c| c.delta.as_str()).collect();
        assert_eq!(text, "18°C, nublado");
        assert_eq!(tool.calls.load(Ordering::SeqCst), 1);

        let requests = mock.received_requests();
        assert_eq!(requests.len(), 2);
        // First call offered the tool.
        assert_eq!(requests[0].tools.len(), 1);
        assert_eq!(requests[0].tools[0].name, "web_search");
        // Second call replayed the assistant's tool_calls message, then
        // the tool's own result addressed to that same call id.
        let assistant_echo = requests[1].messages.iter().find(|m| !m.tool_calls.is_empty()).unwrap();
        assert_eq!(assistant_echo.tool_calls[0].id, "call_1");
        let tool_result = requests[1].messages.iter().find(|m| m.role == MessageRole::Tool).unwrap();
        assert_eq!(tool_result.tool_call_id.as_deref(), Some("call_1"));
        assert_eq!(tool_result.content, "18°C, nublado, humedad 80%");
    }

    #[tokio::test]
    async fn an_unknown_tool_name_gets_a_placeholder_result_instead_of_failing() {
        let tool_call = ToolCall { id: "call_1".to_string(), name: "does_not_exist".to_string(), arguments: "{}".to_string() };
        let mock = Arc::new(MockLLMProvider::with_call_sequence([tool_call_response(vec![tool_call]), text_response(["ok"])]));
        let tool = Arc::new(FakeSearchTool::new("nunca debería llamarse"));
        let orchestrator = AIOrchestrator::new(mock.clone(), "mock-model".to_string(), None, None).with_tools(vec![tool.clone()]);

        let results: Vec<_> = orchestrator.stream_respond(&sample_profile(), &[], None, &[], None, None, "es").collect::<Vec<_>>().await;
        assert!(results.into_iter().all(|r| r.is_ok()));
        assert_eq!(tool.calls.load(Ordering::SeqCst), 0);

        let requests = mock.received_requests();
        let tool_result = requests[1].messages.iter().find(|m| m.role == MessageRole::Tool).unwrap();
        assert_eq!(tool_result.content, "Herramienta desconocida.");
    }

    #[tokio::test]
    async fn tool_call_round_trips_are_capped_so_a_stubborn_model_cant_loop_forever() {
        // Always asks for a tool call, never gives a plain answer.
        let responses: Vec<_> = (0..10)
            .map(|i| tool_call_response(vec![ToolCall { id: format!("call_{i}"), name: "web_search".to_string(), arguments: "{}".to_string() }]))
            .collect();
        let mock = Arc::new(MockLLMProvider::with_call_sequence(responses));
        let tool = Arc::new(FakeSearchTool::new("resultado"));
        let orchestrator = AIOrchestrator::new(mock.clone(), "mock-model".to_string(), None, None).with_tools(vec![tool]);

        let _ = orchestrator.stream_respond(&sample_profile(), &[], None, &[], None, None, "es").collect::<Vec<_>>().await;

        let requests = mock.received_requests();
        assert_eq!(requests.len(), MAX_TOOL_ROUNDS);
        // The final round forces a direct answer by not offering tools.
        assert!(requests.last().unwrap().tools.is_empty());
    }

    #[tokio::test]
    async fn current_date_and_time_are_in_the_system_prompt_even_with_no_tools() {
        // Regression: live testing found a bare "¿qué fecha es hoy?" (no
        // place mentioned) never triggered a tool call at all —
        // `get_current_time` is scoped to *other* places — and the model
        // answered from training data instead, giving a 2024 date. The
        // real date/time is now injected as ground truth independent of
        // whether any tool is configured.
        let mock = Arc::new(MockLLMProvider::with_response(["ok"]));
        let orchestrator = AIOrchestrator::new(mock.clone(), "mock-model".to_string(), None, None);

        let _ = orchestrator.stream_respond(&sample_profile(), &[], None, &[], None, None, "es").collect::<Vec<_>>().await;

        let prompt = &mock.received_requests()[0].messages[0].content;
        assert!(prompt.contains("Dato real y actual"));
        assert!(prompt.contains("hora local del usuario"));
    }

    #[tokio::test]
    async fn current_date_and_time_are_repeated_after_a_long_history_regardless_of_tools() {
        // Same recency concern as the tool-usage hint: a fact stated only
        // at the very top of the system prompt loses attention weight once
        // ~20 messages of history pile up, so it's repeated as the last
        // message too.
        let mock = Arc::new(MockLLMProvider::with_response(["ok"]));
        let orchestrator = AIOrchestrator::new(mock.clone(), "mock-model".to_string(), None, None);
        let history: Vec<_> = (0..20).map(|i| sample_message(DbMessageRole::User, &format!("msg-{i}"))).collect();

        let _ = orchestrator.stream_respond(&sample_profile(), &history, None, &[], None, None, "es").collect::<Vec<_>>().await;

        let messages = &mock.received_requests()[0].messages;
        let last = messages.last().unwrap();
        assert_eq!(last.role, MessageRole::System);
        assert!(last.content.contains("Dato real y actual"));
    }

    #[test]
    fn formats_a_known_date_in_spanish() {
        let now = chrono::Utc.with_ymd_and_hms(2024, 1, 1, 14, 30, 0).unwrap();
        let text = format_current_datetime_line(now, None, "es");
        assert!(text.contains("lunes"));
        assert!(text.contains("01/01/2024"));
        assert!(text.contains("14:30"));
    }

    #[test]
    fn includes_the_system_timezone_as_a_location_hint_when_available() {
        // Regression: a bare "clima" (no city) made the model search for
        // "clima en mi ubicación" — no real location — and Brave returned
        // a different city's forecast (Buenos Aires) that the model then
        // presented as fact. The system's own IANA zone is a real signal
        // of roughly where the user is.
        let now = chrono::Utc.with_ymd_and_hms(2024, 1, 1, 14, 30, 0).unwrap();
        let text = format_current_datetime_line(now, Some("America/Santiago"), "es");
        assert!(text.contains("America/Santiago"));
        assert!(text.contains("nunca inventes"));
    }

    #[test]
    fn omits_the_location_hint_when_the_system_timezone_is_unavailable() {
        let now = chrono::Utc.with_ymd_and_hms(2024, 1, 1, 14, 30, 0).unwrap();
        let text = format_current_datetime_line(now, None, "es");
        assert!(!text.contains("Zona horaria del sistema"));
    }

    #[test]
    fn spanish_weekday_covers_every_day() {
        assert_eq!(weekday_name(Weekday::Mon, "es"), "lunes");
        assert_eq!(weekday_name(Weekday::Tue, "es"), "martes");
        assert_eq!(weekday_name(Weekday::Wed, "es"), "miércoles");
        assert_eq!(weekday_name(Weekday::Thu, "es"), "jueves");
        assert_eq!(weekday_name(Weekday::Fri, "es"), "viernes");
        assert_eq!(weekday_name(Weekday::Sat, "es"), "sábado");
        assert_eq!(weekday_name(Weekday::Sun, "es"), "domingo");
    }

    #[test]
    fn datetime_line_and_tool_hint_follow_the_configured_language() {
        let now = chrono::Utc.with_ymd_and_hms(2024, 1, 1, 14, 30, 0).unwrap();
        let text = format_current_datetime_line(now, Some("America/Santiago"), "en");
        assert!(text.contains("Monday"));
        assert!(text.contains("never make up"));
        assert!(!text.contains("lunes"));

        let hint = tool_usage_hint("en");
        assert!(hint.contains("You have tools available"));
        assert!(!hint.contains("Tenés herramientas"));
    }

    #[test]
    fn unrecognized_language_falls_back_to_spanish_for_datetime_and_tool_hint() {
        let now = chrono::Utc.with_ymd_and_hms(2024, 1, 1, 14, 30, 0).unwrap();
        let text = format_current_datetime_line(now, None, "fr");
        assert!(text.contains("lunes"));
        assert!(tool_usage_hint("fr").contains("Tenés herramientas"));
    }
}
