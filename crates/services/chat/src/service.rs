use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use chrono::{Duration, Utc};
use futures::StreamExt;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use aivt_database::{
    Attachment, AttachmentsRepository, Conversation, ConversationsRepository, DbPool, Memory,
    Message, MessageRole, MessageStatus, MessagesRepository, NewAttachment, NewMessage, Profile,
    ProfileVoice,
};
use aivt_eventbus::{AppEvent, EventBus};
use aivt_memory::MemoryManager;
use aivt_orchestrator::{AIOrchestrator, SessionContext};
use crate::summary;
use aivt_provider_llm::LLMProvider;
use aivt_provider_tts::{SpeakRequest, TtsBackend, TTSProvider};

use crate::title::title_from_first_message;
use crate::ChatError;

const CHAT_SOURCE: &str = "chat";
/// `conversations.source` for a conversation created by "Exportar a Chat"
/// from the Llamada tab — the value `0004_chat.sql`'s own comment already
/// anticipated ("hoy solo 'chat' existe; 'call'... se usan a partir de sus
/// fases respectivas sin necesitar cambios de esquema"), so no migration
/// was needed to add this.
const CALL_SOURCE: &str = "call";

/// How many memories to fold into the prompt for a single message — kept
/// small so the memory section stays a nudge, not a wall of text competing
/// with the actual conversation for the model's attention.
const MEMORY_RETRIEVAL_LIMIT: i64 = 5;

/// How many of the most recent messages feed the retrieval query. A vague
/// follow-up like "cuéntame más sobre eso" shares almost no words with the
/// actual memory — the topic was named a message or two earlier, not in
/// this one. Using only `history.last()` meant that turn matched nothing,
/// so no memory ever reached the prompt and the model made something up
/// instead of a real memory being in context at all (pedido del usuario:
/// "no llega a conseguirla, se inventa información"). A short window keeps
/// the topic's real keywords in the query for a few turns after they were
/// last mentioned.
const RETRIEVAL_QUERY_WINDOW: usize = 4;

/// Consolidation runs after this many *new* messages pile up since the last
/// pass (sección 39 del brief: nunca en cada mensaje) — 6 is three
/// user/assistant turns, frequent enough to feel responsive without calling
/// the LLM for extraction on every single reply.
const CONSOLIDATION_BATCH_SIZE: i64 = 6;

/// A message plus the (optional) image attached to it — the shape the
/// frontend actually consumes. Kept separate from `aivt_database::Message`
/// so the repository layer stays SQL-only; joining in attachment metadata
/// is this service's job, not the database crate's (sección 3 del brief,
/// Fase 3: Multimodal).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageView {
    #[serde(flatten)]
    pub message: Message,
    pub attachment_id: Option<String>,
    pub attachment_mime_type: Option<String>,
}

/// What the command layer hands in after decoding the frontend's base64
/// payload — mirrors how `profiles_set_avatar` decodes once at the IPC
/// boundary and passes raw bytes inward. `data_url` is the *original*
/// base64 string (already prefixed with `data:{mime};base64,`), kept around
/// only to hand to the orchestrator for this one generation — never
/// persisted as-is (the file on disk is the source of truth afterwards).
pub struct NewAttachmentInput {
    pub bytes: Vec<u8>,
    pub mime_type: String,
    pub extension: String,
    pub data_url: String,
}

/// Everything one assistant generation needs, built by the Tauri command
/// layer from current settings + the active profile — `ChatService` itself
/// never reads settings directly.
pub struct GenerationContext {
    pub profile: Profile,
    pub orchestrator: Arc<AIOrchestrator>,
    pub model: String,
    pub provider_label: String,
    /// Same provider `orchestrator` wraps, kept as a separate handle —
    /// memory extraction (Fase 4) is a one-shot analysis call, not a
    /// streamed chat response, so it talks to the provider directly rather
    /// than through `AIOrchestrator`.
    pub llm_provider: Arc<dyn LLMProvider>,
    /// `None` when the profile has voice disabled or no TTS provider could
    /// be resolved — auto-play is best-effort and skipped entirely rather
    /// than blocking or erroring the chat response over it, same stance as
    /// `memory` retrieval failures (sección 81 del brief).
    pub tts: Option<TtsInvocation>,
    /// Resolved from `SettingsService::language` by the command layer, same
    /// division of labor as the rest of this struct — forwarded to
    /// `AIOrchestrator::stream_respond` so the identity/personality portion
    /// of the system prompt follows the app's current language.
    pub language: String,
}

/// What `run_generation` needs to auto-speak a completed response: an
/// engine to call, which backend it should use for *this* call (native vs.
/// the globally-configured HTTP server), and the profile's own voice
/// settings — resolved once by the command layer from current Settings +
/// the profile row, same division of labor as `llm_provider`/`model`.
pub struct TtsInvocation {
    pub provider: Arc<dyn TTSProvider>,
    pub backend: TtsBackend,
    pub voice: ProfileVoice,
    /// Resolved from `SettingsService::chat_audio_settings` by the command
    /// layer, same division of labor as `backend`/`voice` — `maybe_speak`
    /// only saves the bytes `speak_and_capture` hands back when this is
    /// `true` (and only if the active backend produced any: `Native` never
    /// does, see that method's doc comment).
    pub save_audio: bool,
}

/// Every field is already cheap to clone (`DbPool`/`EventBus` wrap an
/// `Arc` internally, `memory`/`cancellations` are already `Arc`-backed) —
/// `ProactiveService` (a different crate) holds a clone to call
/// `send_proactive_message` without needing its own copy of every
/// dependency `ChatService` already wires up.
#[derive(Clone)]
pub struct ChatService {
    pool: DbPool,
    event_bus: EventBus,
    /// App data directory (same root as the SQLite file) — attachments live
    /// at `<data_dir>/attachments/<hash-prefix>/<hash>.<ext>`, never as
    /// base64 inside SQLite (sección 75 del brief), mirroring how
    /// `ProfileService` stores avatars.
    data_dir: PathBuf,
    memory: Arc<dyn MemoryManager>,
    /// One in-flight generation per conversation at a time (sección 88 del
    /// brief) — a second `send_message` on the same conversation is
    /// rejected while an entry exists here, rather than queued or
    /// interrupted.
    cancellations: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

impl ChatService {
    pub fn new(pool: DbPool, event_bus: EventBus, data_dir: PathBuf, memory: Arc<dyn MemoryManager>) -> Self {
        Self { pool, event_bus, data_dir, memory, cancellations: Arc::new(Mutex::new(HashMap::new())) }
    }

    /// Handle al mismo bus en el que este servicio publica sus propios
    /// eventos — usado por `ProactiveService`, que solo guarda un
    /// `ChatService` (no su propio `EventBus`) pero igual necesita uno para
    /// `SettingsService::build_llm_provider`.
    pub fn event_bus(&self) -> EventBus {
        self.event_bus.clone()
    }

    fn attachments_dir(&self) -> PathBuf {
        self.data_dir.join("attachments")
    }

    pub async fn list_conversations(&self, profile_id: &str) -> Result<Vec<Conversation>, ChatError> {
        Ok(ConversationsRepository::new(&self.pool).list_for_profile(profile_id).await?)
    }

    pub async fn create_conversation(&self, profile_id: &str) -> Result<Conversation, ChatError> {
        Ok(ConversationsRepository::new(&self.pool)
            .create(profile_id, "Nueva conversación", CHAT_SOURCE)
            .await?)
    }

    /// "Exportar a Chat" desde la pestaña Llamada (Fase 6) — convierte la
    /// transcripción en vivo (que nunca se persiste, ver `CallStore`) en
    /// una conversación normal para poder seguirla por escrito. Inserta
    /// cada entrada directo como `Message`, sin pasar por el LLM ni por
    /// `run_generation` — mismo patrón que `send_proactive_message` usa
    /// para insertar la respuesta ya decidida de una IA proactiva.
    /// `entries` ya viene sin las líneas de error del feed de la llamada
    /// (filtradas por la capa de comando) — a este método no le interesa
    /// qué es o no un error de llamada, solo insertar diálogo real.
    pub async fn import_call_transcript(
        &self,
        profile_id: &str,
        entries: Vec<(MessageRole, String)>,
    ) -> Result<Conversation, ChatError> {
        let title = entries
            .iter()
            .find(|(role, _)| *role == MessageRole::User)
            .map(|(_, text)| title_from_first_message(text))
            .unwrap_or_else(|| "Nueva conversación".to_string());

        let conversation = ConversationsRepository::new(&self.pool).create(profile_id, &title, CALL_SOURCE).await?;

        let messages = MessagesRepository::new(&self.pool);
        for (role, content) in entries {
            messages
                .create(NewMessage {
                    conversation_id: conversation.id.clone(),
                    role,
                    content,
                    status: MessageStatus::Complete,
                    provider_id: None,
                    model: None,
                })
                .await?;
        }

        ConversationsRepository::new(&self.pool).touch(&conversation.id).await?;
        Ok(conversation)
    }

    pub async fn set_conversation_pinned(&self, id: &str, pinned: bool) -> Result<(), ChatError> {
        Ok(ConversationsRepository::new(&self.pool).set_pinned(id, pinned).await?)
    }

    pub async fn reorder_pinned_conversations(&self, ordered_ids: &[String]) -> Result<(), ChatError> {
        Ok(ConversationsRepository::new(&self.pool).reorder_pinned(ordered_ids).await?)
    }

    /// Inserta y transmite un mensaje del personaje sin que exista un
    /// mensaje de usuario que lo preceda — usado por `ProactiveService`
    /// (ver docs/ADR/007-proactive-mode.md). A diferencia de
    /// `run_generation`, el texto ya está completo (lo decidió el LLM en
    /// la llamada de decisión), así que no hace falta el streaming
    /// incremental: se inserta ya completo y se publican los mismos tres
    /// eventos (`AiResponseStarted`/`Token`/`Finished`) que una respuesta
    /// normal, para que se renderice y anime el avatar exactamente igual
    /// sin un camino de UI paralelo. Usa la conversación más reciente no
    /// archivada del perfil, o crea una nueva si no hay ninguna — la
    /// política de "no abrir en frío" vive en `ProactiveService`, no acá.
    pub async fn send_proactive_message(
        &self,
        profile: &Profile,
        content: &str,
        provider_label: &str,
        model: &str,
        tts: Option<TtsInvocation>,
    ) -> Result<(), ChatError> {
        let conversation = match ConversationsRepository::new(&self.pool).list_for_profile(&profile.id).await?.into_iter().next() {
            Some(conversation) => conversation,
            None => self.create_conversation(&profile.id).await?,
        };

        let assistant_message = MessagesRepository::new(&self.pool)
            .create(NewMessage {
                conversation_id: conversation.id.clone(),
                role: MessageRole::Assistant,
                content: content.to_string(),
                status: MessageStatus::Complete,
                provider_id: Some(provider_label.to_string()),
                model: Some(model.to_string()),
            })
            .await?;
        ConversationsRepository::new(&self.pool).touch(&conversation.id).await?;

        let request_id = Uuid::new_v4().to_string();
        self.event_bus.publish(AppEvent::AiResponseStarted {
            request_id: request_id.clone(),
            profile_id: profile.id.clone(),
            conversation_id: conversation.id.clone(),
            message_id: assistant_message.id.clone(),
        });
        self.event_bus.publish(AppEvent::AiResponseToken { request_id: request_id.clone(), token: content.to_string() });
        self.event_bus.publish(AppEvent::AiResponseFinished { request_id: request_id.clone() });
        self.event_bus.publish(AppEvent::ProactiveMessageSent {
            profile_id: profile.id.clone(),
            conversation_id: conversation.id.clone(),
            content: content.to_string(),
        });

        maybe_speak(&self.pool, &self.data_dir, &self.event_bus, &profile.id, tts.as_ref(), &conversation.id, &assistant_message.id, content);

        Ok(())
    }

    /// On-demand version of the automatic consolidation `run_generation`
    /// triggers every `CONSOLIDATION_BATCH_SIZE` messages — same "new tail
    /// since last pass" logic, but runs immediately (no threshold) and
    /// awaited, so the caller can show the user what got extracted right
    /// away instead of waiting for enough messages to pile up naturally.
    pub async fn extract_memories_now(
        &self,
        conversation_id: &str,
        profile_id: &str,
        provider: Arc<dyn LLMProvider>,
        model: &str,
        language: &str,
    ) -> Result<Vec<Memory>, ChatError> {
        let conversation = ConversationsRepository::new(&self.pool)
            .get(conversation_id)
            .await?
            .ok_or(ChatError::ConversationNotFound)?;
        let all_messages = MessagesRepository::new(&self.pool).list_for_conversation(conversation_id).await?;
        let total = all_messages.len() as i64;
        let since = conversation.memory_consolidated_through;
        if total <= since {
            return Ok(Vec::new());
        }

        let new_tail = &all_messages[since.max(0) as usize..];
        let created = self
            .memory
            .extract_and_store(profile_id, Some(conversation_id), new_tail, provider, model, language)
            .await?;
        ConversationsRepository::new(&self.pool).set_memory_consolidated_through(conversation_id, total).await?;
        self.event_bus.publish(AppEvent::MemoriesUpdated { profile_id: profile_id.to_string() });
        Ok(created)
    }

    pub async fn get_messages(&self, conversation_id: &str) -> Result<Vec<MessageView>, ChatError> {
        let messages = MessagesRepository::new(&self.pool).list_for_conversation(conversation_id).await?;
        let attachments = AttachmentsRepository::new(&self.pool).list_for_conversation(conversation_id).await?;
        Ok(merge_attachments(messages, attachments))
    }

    /// Reads a stored attachment back into memory for display — see
    /// `ProfileService::read_avatar` for why this returns raw bytes for the
    /// frontend to wrap in a `data:` URI rather than going through Tauri's
    /// asset protocol.
    pub async fn get_attachment(&self, id: &str) -> Result<(Vec<u8>, String), ChatError> {
        let attachment = AttachmentsRepository::new(&self.pool).get(id).await?.ok_or(ChatError::AttachmentNotFound)?;
        let bytes = tokio::fs::read(self.data_dir.join(&attachment.path)).await?;
        Ok((bytes, attachment.mime_type))
    }

    /// Deletes saved audio attachments older than `retention_days` — called
    /// once at startup (see `src-tauri/src/lib.rs`), pedido del usuario: "ir
    /// borrando los audios tras meses" (and later, finer-grained options
    /// down to 1 día) to bound how much disk space this feature can
    /// accumulate. Best-effort per attachment: a file or row that fails to
    /// delete is logged and skipped rather than aborting the sweep; returns
    /// how many were actually removed.
    pub async fn prune_old_audio(&self, retention_days: i64) -> Result<u64, ChatError> {
        let cutoff = (Utc::now() - Duration::days(retention_days.max(0))).to_rfc3339();
        let repo = AttachmentsRepository::new(&self.pool);
        let stale = repo.list_older_than("audio", &cutoff).await?;

        let mut deleted = 0u64;
        for attachment in stale {
            let _ = tokio::fs::remove_file(self.data_dir.join(&attachment.path)).await;
            match repo.delete(&attachment.id).await {
                Ok(()) => deleted += 1,
                Err(err) => tracing::warn!(target: "system", "no se pudo borrar el registro de audio vencido {}: {err}", attachment.id),
            }
        }
        Ok(deleted)
    }

    pub async fn rename_conversation(&self, id: &str, title: &str) -> Result<(), ChatError> {
        Ok(ConversationsRepository::new(&self.pool).rename(id, title).await?)
    }

    pub async fn archive_conversation(&self, id: &str) -> Result<(), ChatError> {
        Ok(ConversationsRepository::new(&self.pool).archive(id).await?)
    }

    pub async fn delete_conversation(&self, id: &str) -> Result<(), ChatError> {
        Ok(ConversationsRepository::new(&self.pool).delete(id).await?)
    }

    pub async fn cancel_generation(&self, conversation_id: &str) {
        if let Some(token) = self.cancellations.lock().await.remove(conversation_id) {
            token.cancel();
        }
    }

    /// Persists the user's message (and its image, if any), kicks off the
    /// assistant response in the background (streamed to the frontend via
    /// `AppEvent`s — see `run_generation`), and returns immediately with
    /// the persisted user message without waiting for a reply.
    pub async fn send_message(
        &self,
        conversation_id: &str,
        content: &str,
        attachment: Option<NewAttachmentInput>,
        ctx: GenerationContext,
    ) -> Result<MessageView, ChatError> {
        if self.cancellations.lock().await.contains_key(conversation_id) {
            return Err(ChatError::GenerationInProgress);
        }

        let conversation = ConversationsRepository::new(&self.pool)
            .get(conversation_id)
            .await?
            .ok_or(ChatError::ConversationNotFound)?;

        let is_first_message =
            MessagesRepository::new(&self.pool).list_for_conversation(conversation_id).await?.is_empty();

        let user_message = MessagesRepository::new(&self.pool)
            .create(NewMessage {
                conversation_id: conversation_id.to_string(),
                role: MessageRole::User,
                content: content.to_string(),
                status: MessageStatus::Complete,
                provider_id: None,
                model: None,
            })
            .await?;

        let mut view = MessageView { message: user_message.clone(), attachment_id: None, attachment_mime_type: None };
        let mut latest_user_image = None;

        if let Some(input) = attachment {
            let saved = self.save_attachment(&user_message.id, &input).await?;
            view.attachment_id = Some(saved.id);
            view.attachment_mime_type = Some(saved.mime_type);
            latest_user_image = Some(input.data_url);
        }

        ConversationsRepository::new(&self.pool).touch(conversation_id).await?;
        if is_first_message {
            let title = title_from_first_message(content);
            ConversationsRepository::new(&self.pool).rename(conversation_id, &title).await?;
        }

        self.spawn_generation(conversation, ctx, latest_user_image).await;

        Ok(view)
    }

    async fn save_attachment(&self, message_id: &str, input: &NewAttachmentInput) -> Result<Attachment, ChatError> {
        let hash = hash_bytes(&input.bytes);
        let extension = sanitize_extension(&input.extension);
        let prefix = &hash[..2];
        let dir = self.attachments_dir().join(prefix);
        tokio::fs::create_dir_all(&dir).await?;

        let file_name = format!("{hash}.{extension}");
        let file_path = dir.join(&file_name);
        // Content-addressed: identical bytes already on disk don't need
        // writing again.
        if tokio::fs::metadata(&file_path).await.is_err() {
            tokio::fs::write(&file_path, &input.bytes).await?;
        }

        Ok(AttachmentsRepository::new(&self.pool)
            .create(NewAttachment {
                message_id: message_id.to_string(),
                kind: "image".to_string(),
                path: format!("attachments/{prefix}/{file_name}"),
                mime_type: input.mime_type.clone(),
                size_bytes: input.bytes.len() as i64,
                hash,
            })
            .await?)
    }

    async fn spawn_generation(&self, conversation: Conversation, ctx: GenerationContext, latest_user_image: Option<String>) {
        let token = CancellationToken::new();
        self.cancellations.lock().await.insert(conversation.id.clone(), token.clone());

        let pool = self.pool.clone();
        let event_bus = self.event_bus.clone();
        let memory = self.memory.clone();
        let data_dir = self.data_dir.clone();
        let cancellations = self.cancellations.clone();
        let conversation_id = conversation.id.clone();

        tokio::spawn(async move {
            run_generation(&pool, &event_bus, memory, &data_dir, conversation, ctx, latest_user_image, token).await;
            cancellations.lock().await.remove(&conversation_id);
        });
    }
}

/// Builds the memory retrieval query from the last `RETRIEVAL_QUERY_WINDOW`
/// messages rather than just the newest one — see the constant's doc
/// comment for why a single message isn't enough once the conversation
/// moves into vague follow-ups.
///
/// Only the user's messages count: the character's own words must not steer
/// what it remembers, or something it invented in one reply ("creo que se
/// llamaba Toby") would pull memories about that invention into the next one.
fn retrieval_query(history: &[Message]) -> String {
    let mut user_messages: Vec<&str> =
        history.iter().rev().filter(|m| m.role == MessageRole::User).take(RETRIEVAL_QUERY_WINDOW).map(|m| m.content.as_str()).collect();
    user_messages.reverse();
    user_messages.join(" ")
}

fn hash_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn sanitize_extension(extension: &str) -> String {
    let cleaned: String = extension.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    if cleaned.is_empty() {
        "png".to_string()
    } else {
        cleaned.to_lowercase()
    }
}

fn merge_attachments(messages: Vec<Message>, attachments: Vec<Attachment>) -> Vec<MessageView> {
    let mut by_message: HashMap<String, Attachment> =
        attachments.into_iter().map(|a| (a.message_id.clone(), a)).collect();

    messages
        .into_iter()
        .map(|message| {
            let attachment = by_message.remove(&message.id);
            MessageView {
                attachment_id: attachment.as_ref().map(|a| a.id.clone()),
                attachment_mime_type: attachment.map(|a| a.mime_type),
                message,
            }
        })
        .collect()
}

async fn run_generation(
    pool: &DbPool,
    event_bus: &EventBus,
    memory: Arc<dyn MemoryManager>,
    data_dir: &Path,
    conversation: Conversation,
    ctx: GenerationContext,
    latest_user_image: Option<String>,
    token: CancellationToken,
) {
    let request_id = Uuid::new_v4().to_string();

    let history = match MessagesRepository::new(pool).list_for_conversation(&conversation.id).await {
        Ok(history) => history,
        Err(err) => {
            tracing::warn!(target: "system", "failed to load conversation history: {err}");
            return;
        }
    };

    // A retrieval failure is never fatal to the chat response: memory is
    // an enhancement, not a hard dependency (sección 81 del brief).
    let query = retrieval_query(&history);
    let relevant_memories = match memory.search(&ctx.profile.id, &query, MEMORY_RETRIEVAL_LIMIT).await {
        Ok(memories) => memories,
        Err(err) => {
            tracing::warn!(target: "system", "memory retrieval failed, continuing without it: {err}");
            Vec::new()
        }
    };

    let assistant_message = match MessagesRepository::new(pool)
        .create(NewMessage {
            conversation_id: conversation.id.clone(),
            role: MessageRole::Assistant,
            content: String::new(),
            status: MessageStatus::Streaming,
            provider_id: Some(ctx.provider_label.clone()),
            model: Some(ctx.model.clone()),
        })
        .await
    {
        Ok(message) => message,
        Err(err) => {
            tracing::warn!(target: "system", "failed to create assistant message row: {err}");
            return;
        }
    };

    event_bus.publish(AppEvent::AiResponseStarted {
        request_id: request_id.clone(),
        profile_id: ctx.profile.id.clone(),
        conversation_id: conversation.id.clone(),
        message_id: assistant_message.id.clone(),
    });

    // Igual que la memoria de arriba: un fallo al leer el mood nunca
    // bloquea la respuesta, solo degrada a "sin sección de mood" (ver
    // docs/ADR/006-mood.md).
    let mood = memory.get_mood(&ctx.profile.id).await.ok();

    let started_at = Instant::now();
    let session = SessionContext { summary: &conversation.session_summary, open_threads: &conversation.open_threads };
    let mut stream = ctx.orchestrator.stream_respond_with_session(
        &ctx.profile,
        &history,
        latest_user_image.as_deref(),
        &relevant_memories,
        mood.as_ref(),
        None,
        &ctx.language,
        Some(&session),
    );
    let mut full_text = String::new();
    let mut final_status = MessageStatus::Complete;
    let mut error_message: Option<String> = None;

    loop {
        tokio::select! {
            () = token.cancelled() => {
                final_status = MessageStatus::Cancelled;
                break;
            }
            next = stream.next() => {
                match next {
                    Some(Ok(chunk)) => {
                        if !chunk.delta.is_empty() {
                            full_text.push_str(&chunk.delta);
                            event_bus.publish(AppEvent::AiResponseToken {
                                request_id: request_id.clone(),
                                token: chunk.delta,
                            });
                        }
                        if chunk.finished {
                            break;
                        }
                    }
                    Some(Err(err)) => {
                        final_status = MessageStatus::Error;
                        error_message = Some(err.to_string());
                        break;
                    }
                    None => break,
                }
            }
        }
    }

    let _ = MessagesRepository::new(pool)
        .finalize(&assistant_message.id, &full_text, final_status, None)
        .await;
    let _ = ConversationsRepository::new(pool).touch(&conversation.id).await;

    let duration_ms = started_at.elapsed().as_millis() as i64;

    match final_status {
        MessageStatus::Complete => {
            event_bus.publish(AppEvent::AiResponseFinished { request_id: request_id.clone() });
            tracing::info!(
                target: "activity",
                category = "LLM",
                module = "chat",
                action = "generate",
                profile_id = %ctx.profile.id,
                conversation_id = %conversation.id,
                request_id = %request_id,
                duration_ms = duration_ms,
                status = "success",
                summary = %response_generated_message(&ctx.language),
            );
        }
        MessageStatus::Cancelled => {
            event_bus.publish(AppEvent::AiResponseCancelled { request_id: request_id.clone() });
            tracing::info!(
                target: "activity",
                category = "LLM",
                module = "chat",
                action = "generate",
                profile_id = %ctx.profile.id,
                conversation_id = %conversation.id,
                request_id = %request_id,
                duration_ms = duration_ms,
                status = "cancelled",
                summary = %generation_cancelled_message(&ctx.language),
            );
        }
        MessageStatus::Error => {
            let message = error_message.unwrap_or_default();
            event_bus.publish(AppEvent::AiResponseError {
                request_id: request_id.clone(),
                message: message.clone(),
            });
            tracing::warn!(
                target: "activity",
                category = "LLM",
                module = "chat",
                action = "generate",
                profile_id = %ctx.profile.id,
                conversation_id = %conversation.id,
                request_id = %request_id,
                duration_ms = duration_ms,
                status = "error",
                summary = %generation_error_message(&message, &ctx.language),
            );
        }
        MessageStatus::Streaming => unreachable!("the loop above always ends in a terminal status"),
    }

    if final_status == MessageStatus::Complete {
        maybe_consolidate(pool, event_bus, memory, &conversation, &ctx);
        maybe_summarize(pool, &conversation.id, &ctx);
        maybe_speak(pool, data_dir, event_bus, &ctx.profile.id, ctx.tts.as_ref(), &conversation.id, &assistant_message.id, &full_text);
    }
}

/// Fires TTS playback as a detached task, same reasoning as
/// `maybe_consolidate`: a slow or failing speech engine must never delay
/// the `AiResponseFinished` event or hold the conversation's generation
/// slot open. When `tts.save_audio` is set, also persists whatever bytes
/// `speak_and_capture` hands back as an attachment on `message_id` and
/// publishes `AppEvent::ChatAudioSaved` — see `save_audio_attachment`'s
/// doc comment for why that event matters (without it, the frontend never
/// learns the attachment exists and keeps re-synthesizing on every replay).
#[allow(clippy::too_many_arguments)]
/// Takes `profile_id`/`tts` directly rather than a full `GenerationContext`
/// — those are the only two fields this ever reads, and
/// `ChatService::send_proactive_message` needs to trigger the same
/// auto-play without an `orchestrator`/`llm_provider` to build one.
fn maybe_speak(
    pool: &DbPool,
    data_dir: &Path,
    event_bus: &EventBus,
    profile_id: &str,
    tts: Option<&TtsInvocation>,
    conversation_id: &str,
    message_id: &str,
    text: &str,
) {
    let Some(tts) = tts else {
        tracing::info!(target: "system", "auto-play skipped: no TTS invocation resolved for this generation");
        return;
    };
    if !tts.voice.enabled || text.trim().is_empty() {
        tracing::info!(
            target: "system",
            "auto-play skipped: voice.enabled={} text_empty={}",
            tts.voice.enabled,
            text.trim().is_empty(),
        );
        return;
    }
    tracing::info!(
        target: "system",
        profile_id = %profile_id,
        message_id = %message_id,
        save_audio = tts.save_audio,
        backend = ?tts.backend,
        "auto-play: dispatching speak request",
    );

    let provider = tts.provider.clone();
    let backend = tts.backend.clone();
    let voice = tts.voice.clone();
    let save_audio = tts.save_audio;
    let profile_id = profile_id.to_string();
    let conversation_id = conversation_id.to_string();
    let message_id = message_id.to_string();
    let text = text.to_string();
    let event_bus = event_bus.clone();
    let pool = pool.clone();
    let data_dir = data_dir.to_path_buf();

    tokio::spawn(async move {
        let request = SpeakRequest {
            text,
            voice_id: voice.voice_id,
            rate: voice.rate as f32,
            pitch: voice.pitch as f32,
            volume: voice.volume as f32,
            backend,
            profile_id: profile_id.clone(),
            message_id: Some(message_id.clone()),
        };
        match provider.speak_and_capture(request).await {
            Ok(Some(bytes)) if save_audio => match save_audio_attachment(&pool, &data_dir, &message_id, &bytes).await {
                Ok(attachment) => {
                    tracing::info!(
                        target: "system",
                        message_id = %message_id,
                        attachment_id = %attachment.id,
                        bytes = bytes.len(),
                        "auto-play: audio guardado, publicando ChatAudioSaved",
                    );
                    event_bus.publish(AppEvent::ChatAudioSaved {
                        conversation_id,
                        message_id,
                        attachment_id: attachment.id,
                        mime_type: attachment.mime_type,
                    });
                }
                Err(err) => {
                    tracing::warn!(target: "system", "no se pudo guardar el audio generado para el mensaje {message_id}: {err}");
                }
            },
            Ok(Some(_)) => {
                tracing::info!(target: "system", message_id = %message_id, "auto-play: audio capturado pero save_audio está desactivado, no se guarda");
            }
            Ok(None) => {
                tracing::info!(target: "system", message_id = %message_id, "auto-play: el backend activo no produce bytes capturables (voz nativa), nada que guardar");
            }
            Err(err) => {
                tracing::warn!(target: "system", "auto-play TTS failed: {err}");
                event_bus.publish(AppEvent::VoicePlaybackError {
                    profile_id,
                    message_id: Some(message_id),
                    message: err.to_string(),
                });
            }
        }
    });
}

/// Saves a TTS-generated clip as an attachment on `message_id`, the same
/// content-addressed `<data_dir>/attachments/<hash-prefix>/<hash>.<ext>`
/// layout `ChatService::save_attachment` uses for images — a free function
/// (not a `ChatService` method) because it only ever runs inside
/// `maybe_speak`'s detached task, which has `pool`/`data_dir` but not
/// `&ChatService`. The format is sniffed from the bytes (`audio_format`):
/// most backends return MP3, but the Groq preset asks for WAV.
/// Returns the created `Attachment` (not just `()`) so the caller can
/// publish its id in `AppEvent::ChatAudioSaved` — the assistant message
/// object the frontend already has (built from `AiResponse*` streaming
/// events, never re-fetched from the DB) has no way to find out about this
/// attachment otherwise, which is exactly the bug reported live: every
/// "escuchar" click called the TTS backend again instead of replaying the
/// saved clip, spending ElevenLabs credits on every repeat.
async fn save_audio_attachment(pool: &DbPool, data_dir: &Path, message_id: &str, bytes: &[u8]) -> Result<Attachment, ChatError> {
    let hash = hash_bytes(bytes);
    let prefix = &hash[..2];
    let dir = data_dir.join("attachments").join(prefix);
    tokio::fs::create_dir_all(&dir).await?;

    let (ext, mime_type) = audio_format(bytes);
    let file_name = format!("{hash}.{ext}");
    let file_path = dir.join(&file_name);
    if tokio::fs::metadata(&file_path).await.is_err() {
        tokio::fs::write(&file_path, bytes).await?;
    }

    Ok(AttachmentsRepository::new(pool)
        .create(NewAttachment {
            message_id: message_id.to_string(),
            kind: "audio".to_string(),
            path: format!("attachments/{prefix}/{file_name}"),
            mime_type: mime_type.to_string(),
            size_bytes: bytes.len() as i64,
            hash,
        })
        .await?)
}

/// `(extension, mime)` of a TTS clip; anything that isn't a RIFF/WAVE header is MP3.
fn audio_format(bytes: &[u8]) -> (&'static str, &'static str) {
    if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WAVE" {
        ("wav", "audio/wav")
    } else {
        ("mp3", "audio/mpeg")
    }
}

/// Fires memory extraction as a fully detached task once enough new
/// messages have piled up since the last pass — never awaited here, so a
/// slow (or hung) extraction call can't keep the conversation's
/// generation slot locked (see `ChatService::cancellations`) or delay the
/// `AiResponseFinished` event the user already saw.
fn maybe_consolidate(
    pool: &DbPool,
    event_bus: &EventBus,
    memory: Arc<dyn MemoryManager>,
    conversation: &Conversation,
    ctx: &GenerationContext,
) {
    let pool = pool.clone();
    let event_bus = event_bus.clone();
    let provider = ctx.llm_provider.clone();
    let model = ctx.model.clone();
    let language = ctx.language.clone();
    let profile_id = ctx.profile.id.clone();
    let conversation_id = conversation.id.clone();
    let consolidated_through = conversation.memory_consolidated_through;

    tokio::spawn(async move {
        let all_messages = match MessagesRepository::new(&pool).list_for_conversation(&conversation_id).await {
            Ok(messages) => messages,
            Err(err) => {
                tracing::warn!(target: "system", "consolidation: failed to reload messages: {err}");
                return;
            }
        };

        let total = all_messages.len() as i64;
        if total - consolidated_through < CONSOLIDATION_BATCH_SIZE {
            return;
        }

        let new_tail: Vec<Message> = all_messages[consolidated_through.max(0) as usize..].to_vec();
        match memory.extract_and_store(&profile_id, Some(&conversation_id), &new_tail, provider, &model, &language).await {
            Ok(created) => {
                let _ = ConversationsRepository::new(&pool).set_memory_consolidated_through(&conversation_id, total).await;
                if !created.is_empty() {
                    event_bus.publish(AppEvent::MemoriesUpdated { profile_id: profile_id.clone() });
                }
                tracing::info!(
                    target: "activity",
                    category = "MEMORY",
                    module = "chat",
                    action = "consolidate",
                    profile_id = %profile_id,
                    conversation_id = %conversation_id,
                    status = "success",
                    summary = %memories_extracted_message(created.len(), &language),
                );
            }
            Err(err) => {
                tracing::warn!(
                    target: "activity",
                    category = "MEMORY",
                    module = "chat",
                    action = "consolidate",
                    profile_id = %profile_id,
                    conversation_id = %conversation_id,
                    status = "error",
                    summary = %memory_extraction_failed_message(&err.to_string(), &language),
                );
            }
        }
    });
}

/// Refreshes the conversation's running summary and open threads as a detached task,
/// same reasoning as `maybe_consolidate`: a slow or failing model call must never delay
/// the response the user already saw. A failure or an unusable reply keeps the old notes.
fn maybe_summarize(pool: &DbPool, conversation_id: &str, ctx: &GenerationContext) {
    let pool = pool.clone();
    let provider = ctx.llm_provider.clone();
    let model = ctx.model.clone();
    let conversation_id = conversation_id.to_string();

    tokio::spawn(async move {
        let repo = ConversationsRepository::new(&pool);
        let Ok(Some(conversation)) = repo.get(&conversation_id).await else { return };
        let Ok(messages) = MessagesRepository::new(&pool).list_for_conversation(&conversation_id).await else { return };
        let Some(upto) = summary::due(messages.len() as i64, conversation.summarized_through) else { return };

        let previous = summary::SessionNotes { summary: conversation.session_summary.clone(), open_threads: conversation.open_threads.clone() };
        let segment = &messages[conversation.summarized_through.max(0) as usize..upto as usize];
        match summary::update(provider, &model, &previous, segment).await {
            Ok(Some(notes)) => {
                let _ = repo.set_session_summary(&conversation_id, &notes.summary, &notes.open_threads, upto).await;
            }
            Ok(None) => tracing::warn!(target: "system", "session summary: the model's reply was not usable, keeping the previous notes"),
            Err(err) => tracing::warn!(target: "system", "session summary failed, keeping the previous notes: {err}"),
        }
    });
}

fn response_generated_message(lang: &str) -> &'static str {
    match lang {
        "en" => "Response generated",
        "pt" => "Resposta gerada",
        "ja" => "返答を生成しました",
        "ko" => "응답이 생성되었습니다",
        "zh" => "已生成回复",
        _ => "Respuesta generada",
    }
}

fn generation_cancelled_message(lang: &str) -> &'static str {
    match lang {
        "en" => "Generation cancelled by the user",
        "pt" => "Geração cancelada pelo usuário",
        "ja" => "ユーザーが生成をキャンセルしました",
        "ko" => "사용자가 생성을 취소했습니다",
        "zh" => "用户取消了生成",
        _ => "Generación cancelada por el usuario",
    }
}

fn generation_error_message(err: &str, lang: &str) -> String {
    match lang {
        "en" => format!("Error generating response: {err}"),
        "pt" => format!("Erro ao gerar resposta: {err}"),
        "ja" => format!("返答の生成エラー: {err}"),
        "ko" => format!("응답 생성 오류: {err}"),
        "zh" => format!("生成回复出错：{err}"),
        _ => format!("Error al generar respuesta: {err}"),
    }
}

fn memories_extracted_message(count: usize, lang: &str) -> String {
    match lang {
        "en" => format!("{count} new memories extracted"),
        "pt" => format!("{count} novas memórias extraídas"),
        "ja" => format!("新しい記憶を{count}件抽出しました"),
        "ko" => format!("새로운 기억 {count}개를 추출했습니다"),
        "zh" => format!("提取了 {count} 条新记忆"),
        _ => format!("{count} memorias nuevas extraídas"),
    }
}

fn memory_extraction_failed_message(err: &str, lang: &str) -> String {
    match lang {
        "en" => format!("Memory extraction failed: {err}"),
        "pt" => format!("Falha na extração de memórias: {err}"),
        "ja" => format!("記憶の抽出に失敗しました: {err}"),
        "ko" => format!("기억 추출에 실패했습니다: {err}"),
        "zh" => format!("记忆提取失败：{err}"),
        _ => format!("Falló la extracción de memorias: {err}"),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn audio_format_detects_wav_and_defaults_to_mp3() {
        assert_eq!(audio_format(b"RIFF    WAVEfmt "), ("wav", "audio/wav"));
        assert_eq!(audio_format(b"ID3         "), ("mp3", "audio/mpeg"));
        assert_eq!(audio_format(b"fake-mp3-bytes"), ("mp3", "audio/mpeg"));
    }

    use std::time::Duration;

    use aivt_database::{NewProfile, ProfilesRepository};
    use aivt_orchestrator::AIOrchestrator;
    use aivt_provider_llm::ProviderError;
    use aivt_testing::{MockLLMProvider, MockMemoryManager, MockTTSProvider};

    use super::*;

    async fn test_pool() -> DbPool {
        let pool = DbPool::connect("sqlite::memory:").await.unwrap();
        aivt_database::migrate(&pool).await.unwrap();
        pool
    }

    async fn seed_profile(pool: &DbPool) -> Profile {
        ProfilesRepository::new(pool)
            .create(NewProfile { name: "Ciel".to_string(), nickname: None, description: None })
            .await
            .unwrap()
    }

    fn context_with(orchestrator: AIOrchestrator) -> GenerationContext {
        GenerationContext {
            profile: Profile {
                id: String::new(),
                name: String::new(),
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
                created_at: String::new(),
                updated_at: String::new(),
                identity: Default::default(),
                personality: Default::default(),
                voice: Default::default(),
                avatar_idle_path: None,
                avatar_thinking_path: None,
                avatar_talking_path: None,
            },
            orchestrator: Arc::new(orchestrator),
            model: "mock-model".to_string(),
            provider_label: "mock".to_string(),
            // Only exercised by consolidation, which none of the tests below
            // trigger (they send far fewer than `CONSOLIDATION_BATCH_SIZE`
            // messages) — the memory-specific tests further down build their
            // own context with a real handle to assert against instead.
            llm_provider: Arc::new(MockLLMProvider::with_response(["unused"])),
            tts: None,
            language: "es".to_string(),
        }
    }

    /// Polls until the conversation's last message reaches `expected`, or
    /// panics. A plain `.await` loop on the *same* task — no nested
    /// executor — so the background generation task (driven by the shared
    /// current-thread runtime) actually gets scheduled between iterations.
    async fn wait_for_last_message_status(pool: &DbPool, conversation_id: &str, expected_len: usize, expected: MessageStatus) {
        for _ in 0..200 {
            let messages = MessagesRepository::new(pool).list_for_conversation(conversation_id).await.unwrap();
            if messages.len() == expected_len && messages.last().map(|m| m.status) == Some(expected) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("message never reached status {expected:?}");
    }

    /// `wait_for_last_message_status` only proves the *message* reached its
    /// final status — `spawn_generation`'s wrapping task still has one more
    /// `.await` after that (removing the conversation from
    /// `ChatService::cancellations`) before a *new* `send_message` for the
    /// same conversation is allowed. Under normal load that gap is
    /// negligible; under heavy scheduler contention (e.g. many concurrent
    /// `cargo test`/build processes) it's occasionally wide enough that the
    /// very next call in a tight loop still finds the slot marked busy and
    /// gets `ChatError::GenerationInProgress` — a real but harmless race in
    /// the *test's* polling assumption, not a bug in `ChatService` (a
    /// second concurrent send being rejected is exactly the intended
    /// behavior, see `a_second_send_while_generating_is_rejected`). Builds
    /// a fresh mock-backed `ctx` per attempt (the previous one is consumed/
    /// dropped by the failed call) and retries a bounded number of times
    /// rather than looping forever, so a genuine regression still fails
    /// loudly instead of hanging.
    async fn send_message_retrying(service: &ChatService, conversation_id: &str, content: &str, profile: &Profile) -> MessageView {
        for attempt in 0..50 {
            let mock = Arc::new(MockLLMProvider::with_response(["ok"]));
            let orchestrator = AIOrchestrator::new(mock, "mock-model".to_string(), None, None);
            let mut ctx = context_with(orchestrator);
            ctx.profile = profile.clone();

            match service.send_message(conversation_id, content, None, ctx).await {
                Ok(message) => return message,
                Err(ChatError::GenerationInProgress) if attempt < 49 => {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(err) => panic!("send_message failed: {err}"),
            }
        }
        unreachable!("loop above always returns or panics");
    }

    #[tokio::test]
    async fn send_message_persists_user_message_and_titles_the_conversation() {
        let pool = test_pool().await;
        let profile = seed_profile(&pool).await;
        let event_bus = EventBus::new();
        let service = ChatService::new(pool.clone(), event_bus, std::env::temp_dir(), Arc::new(MockMemoryManager::new()));
        let conversation = service.create_conversation(&profile.id).await.unwrap();

        let mock = MockLLMProvider::with_response(["Hola"]);
        let orchestrator = AIOrchestrator::new(Arc::new(mock), "mock-model".to_string(), None, None);
        let mut ctx = context_with(orchestrator);
        ctx.profile = profile.clone();

        let user_message = service.send_message(&conversation.id, "Hola, ¿cómo estás?", None, ctx).await.unwrap();
        assert_eq!(user_message.message.role, MessageRole::User);
        assert_eq!(user_message.message.content, "Hola, ¿cómo estás?");
        assert!(user_message.attachment_id.is_none());

        let renamed = ConversationsRepository::new(&pool).get(&conversation.id).await.unwrap().unwrap();
        assert_eq!(renamed.title, "Hola, ¿cómo estás?");
    }

    #[tokio::test]
    async fn assistant_response_streams_in_and_finalizes() {
        let pool = test_pool().await;
        let profile = seed_profile(&pool).await;
        let event_bus = EventBus::new();
        let mut rx = event_bus.subscribe();
        let service = ChatService::new(pool.clone(), event_bus, std::env::temp_dir(), Arc::new(MockMemoryManager::new()));
        let conversation = service.create_conversation(&profile.id).await.unwrap();

        let mock = MockLLMProvider::with_response(["Hola", " mundo"]);
        let orchestrator = AIOrchestrator::new(Arc::new(mock), "mock-model".to_string(), None, None);
        let mut ctx = context_with(orchestrator);
        ctx.profile = profile.clone();

        service.send_message(&conversation.id, "Hola", None, ctx).await.unwrap();

        // Started, two tokens, finished.
        let mut events = Vec::new();
        for _ in 0..4 {
            events.push(rx.recv().await.unwrap());
        }
        assert!(matches!(events[0], AppEvent::AiResponseStarted { .. }));
        assert!(matches!(events[1], AppEvent::AiResponseToken { .. }));
        assert!(matches!(events[2], AppEvent::AiResponseToken { .. }));
        assert!(matches!(events[3], AppEvent::AiResponseFinished { .. }));

        wait_for_last_message_status(&pool, &conversation.id, 2, MessageStatus::Complete).await;

        let messages = service.get_messages(&conversation.id).await.unwrap();
        assert_eq!(messages[1].message.content, "Hola mundo");
        assert_eq!(messages[1].message.status, MessageStatus::Complete);
    }

    #[tokio::test]
    async fn provider_error_marks_the_message_as_errored() {
        let pool = test_pool().await;
        let profile = seed_profile(&pool).await;
        let event_bus = EventBus::new();
        let mut rx = event_bus.subscribe();
        let service = ChatService::new(pool.clone(), event_bus, std::env::temp_dir(), Arc::new(MockMemoryManager::new()));
        let conversation = service.create_conversation(&profile.id).await.unwrap();

        let mock = MockLLMProvider::with_error(ProviderError::ConnectionRefused);
        let orchestrator = AIOrchestrator::new(Arc::new(mock), "mock-model".to_string(), None, None);
        let mut ctx = context_with(orchestrator);
        ctx.profile = profile.clone();

        service.send_message(&conversation.id, "Hola", None, ctx).await.unwrap();

        let started = rx.recv().await.unwrap();
        assert!(matches!(started, AppEvent::AiResponseStarted { .. }));
        let errored = rx.recv().await.unwrap();
        assert!(matches!(errored, AppEvent::AiResponseError { .. }));

        wait_for_last_message_status(&pool, &conversation.id, 2, MessageStatus::Error).await;
    }

    #[tokio::test]
    async fn a_second_send_while_generating_is_rejected() {
        let pool = test_pool().await;
        let profile = seed_profile(&pool).await;
        let event_bus = EventBus::new();
        let service = ChatService::new(pool.clone(), event_bus, std::env::temp_dir(), Arc::new(MockMemoryManager::new()));
        let conversation = service.create_conversation(&profile.id).await.unwrap();

        // A response with many pieces so the generation task is still
        // running when we try to send a second message.
        let mock = MockLLMProvider::with_response(vec!["a"; 200]);
        let orchestrator = AIOrchestrator::new(Arc::new(mock), "mock-model".to_string(), None, None);
        let mut ctx1 = context_with(orchestrator);
        ctx1.profile = profile.clone();

        service.send_message(&conversation.id, "Primero", None, ctx1).await.unwrap();

        let mock2 = MockLLMProvider::with_response(["b"]);
        let orchestrator2 = AIOrchestrator::new(Arc::new(mock2), "mock-model".to_string(), None, None);
        let mut ctx2 = context_with(orchestrator2);
        ctx2.profile = profile.clone();

        let result = service.send_message(&conversation.id, "Segundo", None, ctx2).await;
        assert!(matches!(result, Err(ChatError::GenerationInProgress)));
    }

    #[tokio::test]
    async fn cancel_generation_stops_the_stream_and_marks_it_cancelled() {
        let pool = test_pool().await;
        let profile = seed_profile(&pool).await;
        let event_bus = EventBus::new();
        let mut rx = event_bus.subscribe();
        let service = ChatService::new(pool.clone(), event_bus, std::env::temp_dir(), Arc::new(MockMemoryManager::new()));
        let conversation = service.create_conversation(&profile.id).await.unwrap();

        let mock = MockLLMProvider::with_response(vec!["a"; 1000]);
        let orchestrator = AIOrchestrator::new(Arc::new(mock), "mock-model".to_string(), None, None);
        let mut ctx = context_with(orchestrator);
        ctx.profile = profile.clone();

        service.send_message(&conversation.id, "Hola", None, ctx).await.unwrap();
        assert!(matches!(rx.recv().await.unwrap(), AppEvent::AiResponseStarted { .. }));

        service.cancel_generation(&conversation.id).await;

        let mut saw_cancelled = false;
        for _ in 0..2000 {
            match rx.recv().await.unwrap() {
                AppEvent::AiResponseCancelled { .. } => {
                    saw_cancelled = true;
                    break;
                }
                _ => continue,
            }
        }
        assert!(saw_cancelled, "expected an AiResponseCancelled event");

        wait_for_last_message_status(&pool, &conversation.id, 2, MessageStatus::Cancelled).await;

        // The cancellation slot must be freed so a new message can be sent.
        let mock2 = MockLLMProvider::with_response(["ok"]);
        let orchestrator2 = AIOrchestrator::new(Arc::new(mock2), "mock-model".to_string(), None, None);
        let mut ctx2 = context_with(orchestrator2);
        ctx2.profile = profile.clone();
        assert!(service.send_message(&conversation.id, "Otra vez", None, ctx2).await.is_ok());
    }

    #[tokio::test]
    async fn sending_an_attachment_saves_it_and_forwards_it_to_the_orchestrator() {
        let pool = test_pool().await;
        let profile = seed_profile(&pool).await;
        let event_bus = EventBus::new();
        let dir = tempfile::TempDir::new().unwrap();
        let service = ChatService::new(pool.clone(), event_bus, dir.path().to_path_buf(), Arc::new(MockMemoryManager::new()));
        let conversation = service.create_conversation(&profile.id).await.unwrap();

        let mock = Arc::new(MockLLMProvider::with_response(["Veo un gato"]));
        let orchestrator = AIOrchestrator::new(mock.clone(), "mock-model".to_string(), None, None);
        let mut ctx = context_with(orchestrator);
        ctx.profile = profile.clone();

        let attachment = NewAttachmentInput {
            bytes: vec![1, 2, 3, 4],
            mime_type: "image/png".to_string(),
            extension: "png".to_string(),
            data_url: "data:image/png;base64,AQIDBA==".to_string(),
        };

        let user_message = service
            .send_message(&conversation.id, "¿Qué ves?", Some(attachment), ctx)
            .await
            .unwrap();

        assert!(user_message.attachment_id.is_some());
        assert_eq!(user_message.attachment_mime_type.as_deref(), Some("image/png"));

        // The file actually landed on disk, content-addressed by hash.
        let saved_files: Vec<_> = std::fs::read_dir(dir.path().join("attachments"))
            .unwrap()
            .flat_map(|entry| std::fs::read_dir(entry.unwrap().path()).unwrap())
            .collect();
        assert_eq!(saved_files.len(), 1);

        wait_for_last_message_status(&pool, &conversation.id, 2, MessageStatus::Complete).await;

        let requests = mock.received_requests();
        // [system, user, trailing date/time reminder] go out — the assistant
        // reply is the response, not part of the outgoing request.
        let sent_user_message = &requests[0].messages[1];
        assert_eq!(sent_user_message.images, vec!["data:image/png;base64,AQIDBA==".to_string()]);

        // Listing the conversation surfaces the attachment on the same message.
        let messages = service.get_messages(&conversation.id).await.unwrap();
        assert_eq!(messages[0].attachment_id, user_message.attachment_id);
    }

    fn sample_memory(profile_id: &str, content: &str) -> Memory {
        Memory {
            id: "mem1".to_string(),
            profile_id: profile_id.to_string(),
            title: "Título de prueba".to_string(),
            content: content.to_string(),
            summary: None,
            category: "preference".to_string(),
            importance: 70,
            confidence: 90,
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

    async fn wait_for_extract_calls(memory: &MockMemoryManager, min_count: usize) -> Vec<aivt_testing::ExtractCall> {
        for _ in 0..200 {
            let calls = memory.extract_calls();
            if calls.len() >= min_count {
                return calls;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("extract_and_store was never called");
    }

    #[tokio::test]
    async fn retrieved_memories_are_folded_into_the_request_sent_to_the_provider() {
        let pool = test_pool().await;
        let profile = seed_profile(&pool).await;
        let event_bus = EventBus::new();
        let memory = Arc::new(MockMemoryManager::with_search_results(vec![sample_memory(&profile.id, "Le encanta el café")]));
        let service = ChatService::new(pool.clone(), event_bus, std::env::temp_dir(), memory);
        let conversation = service.create_conversation(&profile.id).await.unwrap();

        let mock = Arc::new(MockLLMProvider::with_response(["ok"]));
        let orchestrator = AIOrchestrator::new(mock.clone(), "mock-model".to_string(), None, None);
        let mut ctx = context_with(orchestrator);
        ctx.profile = profile.clone();

        service.send_message(&conversation.id, "Hola", None, ctx).await.unwrap();
        wait_for_last_message_status(&pool, &conversation.id, 2, MessageStatus::Complete).await;

        // Los recuerdos viajan como contexto aparte; un proveedor con privacidad
        // "completa" (lo normal) los recibe anexados al mensaje de sistema.
        let sent = aivt_provider_llm::PrivacyGuardProvider::apply(aivt_provider_llm::PrivacyMode::Full, mock.received_requests().remove(0)).unwrap();
        assert!(sent.messages[0].content.contains("Le encanta el café"));
    }

    #[test]
    fn retrieval_query_pulls_from_a_window_not_just_the_last_message() {
        let history = vec![
            sample_db_message(MessageRole::User, "Me encanta el senderismo de montaña"),
            sample_db_message(MessageRole::Assistant, "¡Qué bien, cuéntame más alguna vez!"),
            sample_db_message(MessageRole::User, "Cuéntame más sobre eso"),
        ];

        let query = retrieval_query(&history);

        // A vague follow-up alone ("Cuéntame más sobre eso") shares nothing
        // with a memory about hiking — the topic word has to survive from
        // the earlier message in the window, or retrieval finds nothing and
        // the model has no real memory to draw from (exactly the bug
        // reported: it invented an answer instead).
        assert!(query.contains("senderismo"));
    }

    #[test]
    fn retrieval_query_ignores_what_the_assistant_said() {
        let history = vec![
            sample_db_message(MessageRole::User, "¿cómo se llamaba mi perro?"),
            sample_db_message(MessageRole::Assistant, "Creo que se llamaba Toby"),
            sample_db_message(MessageRole::User, "sí, ese"),
        ];

        let query = retrieval_query(&history);

        assert!(query.contains("perro") && query.contains("sí, ese"));
        assert!(!query.contains("Toby"), "query was: {query}");
    }

    #[test]
    fn retrieval_query_never_looks_further_back_than_the_window() {
        let history: Vec<Message> =
            (0..10).map(|i| sample_db_message(MessageRole::User, &format!("mensaje-{i}"))).collect();

        let query = retrieval_query(&history);

        assert!(!query.contains("mensaje-0"), "query was: {query}");
        assert!(query.contains("mensaje-9"));
    }

    fn sample_db_message(role: MessageRole, content: &str) -> Message {
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
    async fn a_vague_follow_up_still_searches_using_the_earlier_topic_words() {
        let pool = test_pool().await;
        let profile = seed_profile(&pool).await;
        let event_bus = EventBus::new();
        let memory = Arc::new(MockMemoryManager::new());
        let service = ChatService::new(pool.clone(), event_bus, std::env::temp_dir(), memory.clone());
        let conversation = service.create_conversation(&profile.id).await.unwrap();

        // Turn 1: the actual topic.
        let mock1 = Arc::new(MockLLMProvider::with_response(["¡Qué interesante!"]));
        let orchestrator1 = AIOrchestrator::new(mock1, "mock-model".to_string(), None, None);
        let mut ctx1 = context_with(orchestrator1);
        ctx1.profile = profile.clone();
        service.send_message(&conversation.id, "Me encanta el senderismo de montaña", None, ctx1).await.unwrap();
        wait_for_last_message_status(&pool, &conversation.id, 2, MessageStatus::Complete).await;

        // Turn 2: a vague follow-up with no topic keywords of its own.
        // Uses `send_message_retrying` (not a direct call) because this second
        // send immediately follows the first `wait_for_last_message_status` —
        // see that helper's docs for the `GenerationInProgress` race it guards
        // against under heavy parallel test load.
        send_message_retrying(&service, &conversation.id, "Cuéntame más sobre eso", &profile).await;
        wait_for_last_message_status(&pool, &conversation.id, 4, MessageStatus::Complete).await;

        let queries = memory.search_queries();
        let last_query = queries.last().unwrap();
        assert!(last_query.contains("senderismo"), "query was: {last_query}");
    }

    #[tokio::test]
    async fn enough_new_messages_triggers_automatic_consolidation() {
        let pool = test_pool().await;
        let profile = seed_profile(&pool).await;
        let event_bus = EventBus::new();
        let memory = Arc::new(MockMemoryManager::new());
        let service = ChatService::new(pool.clone(), event_bus, std::env::temp_dir(), memory.clone());
        let conversation = service.create_conversation(&profile.id).await.unwrap();

        // CONSOLIDATION_BATCH_SIZE = 6 → 3 sends (user + assistant each).
        for (turn, content) in ["Uno", "Dos", "Tres"].into_iter().enumerate() {
            send_message_retrying(&service, &conversation.id, content, &profile).await;
            let expected_len = (turn + 1) * 2;
            wait_for_last_message_status(&pool, &conversation.id, expected_len, MessageStatus::Complete).await;
        }

        let calls = wait_for_extract_calls(&memory, 1).await;
        assert_eq!(calls[0].profile_id, profile.id);
        assert_eq!(calls[0].source_id.as_deref(), Some(conversation.id.as_str()));
        assert_eq!(calls[0].message_count, 6);

        let updated = ConversationsRepository::new(&pool).get(&conversation.id).await.unwrap().unwrap();
        assert_eq!(updated.memory_consolidated_through, 6);
    }

    #[tokio::test]
    async fn fewer_than_the_threshold_never_triggers_consolidation() {
        let pool = test_pool().await;
        let profile = seed_profile(&pool).await;
        let event_bus = EventBus::new();
        let memory = Arc::new(MockMemoryManager::new());
        let service = ChatService::new(pool.clone(), event_bus, std::env::temp_dir(), memory.clone());
        let conversation = service.create_conversation(&profile.id).await.unwrap();

        let mock = Arc::new(MockLLMProvider::with_response(["ok"]));
        let orchestrator = AIOrchestrator::new(mock, "mock-model".to_string(), None, None);
        let mut ctx = context_with(orchestrator);
        ctx.profile = profile.clone();
        service.send_message(&conversation.id, "Hola", None, ctx).await.unwrap();
        wait_for_last_message_status(&pool, &conversation.id, 2, MessageStatus::Complete).await;

        // Give any (wrongly) spawned task a moment to have run, then confirm nothing happened.
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(memory.extract_calls().is_empty());
    }

    #[tokio::test]
    async fn extract_memories_now_ignores_the_threshold() {
        let pool = test_pool().await;
        let profile = seed_profile(&pool).await;
        let event_bus = EventBus::new();
        let memory = Arc::new(MockMemoryManager::new());
        memory.set_extract_result(vec![sample_memory(&profile.id, "Detalle extraído a demanda")]);
        let service = ChatService::new(pool.clone(), event_bus, std::env::temp_dir(), memory.clone());
        let conversation = service.create_conversation(&profile.id).await.unwrap();

        let mock = Arc::new(MockLLMProvider::with_response(["ok"]));
        let orchestrator = AIOrchestrator::new(mock, "mock-model".to_string(), None, None);
        let mut ctx = context_with(orchestrator);
        ctx.profile = profile.clone();
        service.send_message(&conversation.id, "Hola", None, ctx).await.unwrap();
        wait_for_last_message_status(&pool, &conversation.id, 2, MessageStatus::Complete).await;

        let provider = Arc::new(MockLLMProvider::with_response(["[]"]));
        let created = service
            .extract_memories_now(&conversation.id, &profile.id, provider, "mock-model", "es")
            .await
            .unwrap();

        assert_eq!(created.len(), 1);
        assert_eq!(created[0].content, "Detalle extraído a demanda");
        let updated = ConversationsRepository::new(&pool).get(&conversation.id).await.unwrap().unwrap();
        assert_eq!(updated.memory_consolidated_through, 2);
    }

    #[tokio::test]
    async fn extract_memories_now_with_nothing_new_returns_empty_without_calling_the_llm() {
        let pool = test_pool().await;
        let profile = seed_profile(&pool).await;
        let event_bus = EventBus::new();
        let memory = Arc::new(MockMemoryManager::new());
        let service = ChatService::new(pool.clone(), event_bus, std::env::temp_dir(), memory);
        let conversation = service.create_conversation(&profile.id).await.unwrap();

        let provider = Arc::new(MockLLMProvider::with_error(ProviderError::ConnectionRefused));
        let created = service
            .extract_memories_now(&conversation.id, &profile.id, provider, "mock-model", "es")
            .await
            .unwrap();
        assert!(created.is_empty());
    }

    #[tokio::test]
    async fn extract_memories_now_on_a_missing_conversation_errors() {
        let pool = test_pool().await;
        let event_bus = EventBus::new();
        let memory = Arc::new(MockMemoryManager::new());
        let service = ChatService::new(pool.clone(), event_bus, std::env::temp_dir(), memory);

        let provider = Arc::new(MockLLMProvider::with_response(["[]"]));
        let result = service.extract_memories_now("missing", "p1", provider, "mock-model", "es").await;
        assert!(matches!(result, Err(ChatError::ConversationNotFound)));
    }

    fn enabled_voice() -> ProfileVoice {
        ProfileVoice { enabled: true, voice_id: "es-ES-Standard".to_string(), rate: 1.0, pitch: 1.0, volume: 1.0 }
    }

    #[tokio::test]
    async fn a_completed_response_auto_speaks_when_voice_is_enabled() {
        let pool = test_pool().await;
        let profile = seed_profile(&pool).await;
        let event_bus = EventBus::new();
        let service = ChatService::new(pool.clone(), event_bus, std::env::temp_dir(), Arc::new(MockMemoryManager::new()));
        let conversation = service.create_conversation(&profile.id).await.unwrap();

        let mock = MockLLMProvider::with_response(["Hola mundo"]);
        let orchestrator = AIOrchestrator::new(Arc::new(mock), "mock-model".to_string(), None, None);
        let mut ctx = context_with(orchestrator);
        ctx.profile = profile.clone();
        let tts = Arc::new(MockTTSProvider::new());
        ctx.tts = Some(TtsInvocation { provider: tts.clone(), backend: TtsBackend::Native, voice: enabled_voice(), save_audio: false });

        service.send_message(&conversation.id, "Hola", None, ctx).await.unwrap();
        wait_for_last_message_status(&pool, &conversation.id, 2, MessageStatus::Complete).await;

        for _ in 0..200 {
            if !tts.speak_calls().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let calls = tts.speak_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].text, "Hola mundo");
        assert_eq!(calls[0].profile_id, profile.id);
        assert!(matches!(calls[0].backend, TtsBackend::Native));
    }

    /// Waits (with a timeout, so a regression hangs the test instead of the
    /// whole suite) for the specific `ChatAudioSaved` fired for
    /// `message_id`, ignoring every other event already flowing through
    /// the bus (`AiResponseStarted`/`Token`/`Finished`, etc.).
    async fn wait_for_chat_audio_saved(rx: &mut tokio::sync::broadcast::Receiver<AppEvent>, message_id: &str) -> AppEvent {
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let event = rx.recv().await.unwrap();
                if let AppEvent::ChatAudioSaved { message_id: ref mid, .. } = event {
                    if mid == message_id {
                        return event;
                    }
                }
            }
        })
        .await
        .expect("ChatAudioSaved never arrived")
    }

    #[tokio::test]
    async fn a_completed_response_saves_captured_audio_when_enabled() {
        let pool = test_pool().await;
        let profile = seed_profile(&pool).await;
        let event_bus = EventBus::new();
        let mut rx = event_bus.subscribe();
        let data_dir = tempfile::tempdir().unwrap();
        let service = ChatService::new(pool.clone(), event_bus, data_dir.path().to_path_buf(), Arc::new(MockMemoryManager::new()));
        let conversation = service.create_conversation(&profile.id).await.unwrap();

        let mock = MockLLMProvider::with_response(["Hola mundo"]);
        let orchestrator = AIOrchestrator::new(Arc::new(mock), "mock-model".to_string(), None, None);
        let mut ctx = context_with(orchestrator);
        ctx.profile = profile.clone();
        let tts = Arc::new(MockTTSProvider::with_captured_audio(b"fake-mp3-bytes".to_vec()));
        ctx.tts = Some(TtsInvocation {
            provider: tts.clone(),
            backend: TtsBackend::Http { base_url: "http://localhost".to_string(), api_key: None },
            voice: enabled_voice(),
            save_audio: true,
        });

        let user_message = service.send_message(&conversation.id, "Hola", None, ctx).await.unwrap();
        wait_for_last_message_status(&pool, &conversation.id, 2, MessageStatus::Complete).await;

        let messages = service.get_messages(&conversation.id).await.unwrap();
        let assistant_message = messages.iter().find(|m| m.message.id != user_message.message.id).unwrap();

        // The event *is* the signal that the save completed and the frontend
        // can now find the attachment — see `AppEvent::ChatAudioSaved`'s doc
        // comment for why this can't just be a DB-polling loop: reproducing
        // the reported bug means asserting the *notification* fires, not
        // just that the row eventually exists.
        let AppEvent::ChatAudioSaved { conversation_id, message_id, attachment_id, mime_type } =
            wait_for_chat_audio_saved(&mut rx, &assistant_message.message.id).await
        else {
            unreachable!()
        };
        assert_eq!(conversation_id, conversation.id);
        assert_eq!(message_id, assistant_message.message.id);
        assert_eq!(mime_type, "audio/mpeg");

        let (bytes, mime_type) = service.get_attachment(&attachment_id).await.unwrap();
        assert_eq!(bytes, b"fake-mp3-bytes");
        assert_eq!(mime_type, "audio/mpeg");
    }

    #[tokio::test]
    async fn a_completed_response_never_speaks_when_voice_is_disabled() {
        let pool = test_pool().await;
        let profile = seed_profile(&pool).await;
        let event_bus = EventBus::new();
        let service = ChatService::new(pool.clone(), event_bus, std::env::temp_dir(), Arc::new(MockMemoryManager::new()));
        let conversation = service.create_conversation(&profile.id).await.unwrap();

        let mock = MockLLMProvider::with_response(["Hola mundo"]);
        let orchestrator = AIOrchestrator::new(Arc::new(mock), "mock-model".to_string(), None, None);
        let mut ctx = context_with(orchestrator);
        ctx.profile = profile.clone();
        let tts = Arc::new(MockTTSProvider::new());
        let mut voice = enabled_voice();
        voice.enabled = false;
        ctx.tts = Some(TtsInvocation { provider: tts.clone(), backend: TtsBackend::Native, voice, save_audio: false });

        service.send_message(&conversation.id, "Hola", None, ctx).await.unwrap();
        wait_for_last_message_status(&pool, &conversation.id, 2, MessageStatus::Complete).await;

        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(tts.speak_calls().is_empty());
    }

    #[tokio::test]
    async fn a_memory_retrieval_failure_does_not_block_the_chat_response() {
        // `MockMemoryManager::search` never errors, so simulate the
        // "retrieval is best-effort" contract at the type level: `run_generation`
        // must still finish the response even if the memories list ends up
        // empty (which it does whenever nothing was seeded to search).
        let pool = test_pool().await;
        let profile = seed_profile(&pool).await;
        let event_bus = EventBus::new();
        let memory = Arc::new(MockMemoryManager::new());
        let service = ChatService::new(pool.clone(), event_bus, std::env::temp_dir(), memory);
        let conversation = service.create_conversation(&profile.id).await.unwrap();

        let mock = Arc::new(MockLLMProvider::with_response(["ok"]));
        let orchestrator = AIOrchestrator::new(mock, "mock-model".to_string(), None, None);
        let mut ctx = context_with(orchestrator);
        ctx.profile = profile.clone();
        service.send_message(&conversation.id, "Hola", None, ctx).await.unwrap();

        wait_for_last_message_status(&pool, &conversation.id, 2, MessageStatus::Complete).await;
    }

    #[tokio::test]
    async fn send_proactive_message_creates_a_conversation_when_none_exists() {
        let pool = test_pool().await;
        let profile = seed_profile(&pool).await;
        let event_bus = EventBus::new();
        let service = ChatService::new(pool.clone(), event_bus, std::env::temp_dir(), Arc::new(MockMemoryManager::new()));

        service.send_proactive_message(&profile, "¿Cómo va todo?", "local", "mock-model", None).await.unwrap();

        let conversations = service.list_conversations(&profile.id).await.unwrap();
        assert_eq!(conversations.len(), 1);
        let messages = service.get_messages(&conversations[0].id).await.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].message.role, MessageRole::Assistant);
        assert_eq!(messages[0].message.status, MessageStatus::Complete);
        assert_eq!(messages[0].message.content, "¿Cómo va todo?");
    }

    #[tokio::test]
    async fn send_proactive_message_reuses_the_most_recent_conversation() {
        let pool = test_pool().await;
        let profile = seed_profile(&pool).await;
        let event_bus = EventBus::new();
        let service = ChatService::new(pool.clone(), event_bus, std::env::temp_dir(), Arc::new(MockMemoryManager::new()));
        let conversation = service.create_conversation(&profile.id).await.unwrap();

        service.send_proactive_message(&profile, "¿Seguís ahí?", "local", "mock-model", None).await.unwrap();

        let conversations = service.list_conversations(&profile.id).await.unwrap();
        assert_eq!(conversations.len(), 1, "no debería crear una segunda conversación habiendo una ya");
        assert_eq!(conversations[0].id, conversation.id);
    }

    #[tokio::test]
    async fn send_proactive_message_publishes_the_same_events_as_a_normal_response() {
        let pool = test_pool().await;
        let profile = seed_profile(&pool).await;
        let event_bus = EventBus::new();
        let mut rx = event_bus.subscribe();
        let service = ChatService::new(pool.clone(), event_bus, std::env::temp_dir(), Arc::new(MockMemoryManager::new()));

        service.send_proactive_message(&profile, "Hola de nuevo", "local", "mock-model", None).await.unwrap();

        let mut events = Vec::new();
        for _ in 0..4 {
            events.push(rx.recv().await.unwrap());
        }
        assert!(matches!(events[0], AppEvent::AiResponseStarted { .. }));
        assert!(matches!(events[1], AppEvent::AiResponseToken { .. }));
        assert!(matches!(events[2], AppEvent::AiResponseFinished { .. }));
        assert!(matches!(events[3], AppEvent::ProactiveMessageSent { .. }));
    }

    #[tokio::test]
    async fn send_proactive_message_speaks_when_voice_is_enabled() {
        let pool = test_pool().await;
        let profile = seed_profile(&pool).await;
        let event_bus = EventBus::new();
        let service = ChatService::new(pool.clone(), event_bus, std::env::temp_dir(), Arc::new(MockMemoryManager::new()));
        let tts = Arc::new(MockTTSProvider::new());
        let invocation = TtsInvocation { provider: tts.clone(), backend: TtsBackend::Native, voice: enabled_voice(), save_audio: false };

        service.send_proactive_message(&profile, "Hola mundo", "local", "mock-model", Some(invocation)).await.unwrap();

        for _ in 0..200 {
            if !tts.speak_calls().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let calls = tts.speak_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].text, "Hola mundo");
    }

    #[tokio::test]
    async fn import_call_transcript_creates_a_call_sourced_conversation_with_messages_in_order() {
        let pool = test_pool().await;
        let profile = seed_profile(&pool).await;
        let event_bus = EventBus::new();
        let service = ChatService::new(pool.clone(), event_bus, std::env::temp_dir(), Arc::new(MockMemoryManager::new()));

        let entries = vec![
            (MessageRole::User, "Hola, ¿qué tal?".to_string()),
            (MessageRole::Assistant, "¡Muy bien! ¿Y vos?".to_string()),
            (MessageRole::User, "También bien, gracias".to_string()),
        ];
        let conversation = service.import_call_transcript(&profile.id, entries).await.unwrap();

        assert_eq!(conversation.source, "call");
        assert_eq!(conversation.title, "Hola, ¿qué tal?", "titula igual que una conversación normal, con el primer mensaje de usuario");

        let messages = service.get_messages(&conversation.id).await.unwrap();
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].message.role, MessageRole::User);
        assert_eq!(messages[0].message.content, "Hola, ¿qué tal?");
        assert_eq!(messages[1].message.role, MessageRole::Assistant);
        assert_eq!(messages[1].message.content, "¡Muy bien! ¿Y vos?");
        assert_eq!(messages[2].message.role, MessageRole::User);
        assert_eq!(messages[2].message.content, "También bien, gracias");
        assert!(messages.iter().all(|m| m.message.status == MessageStatus::Complete));
    }

    #[tokio::test]
    async fn import_call_transcript_with_no_user_message_falls_back_to_a_generic_title() {
        let pool = test_pool().await;
        let profile = seed_profile(&pool).await;
        let event_bus = EventBus::new();
        let service = ChatService::new(pool.clone(), event_bus, std::env::temp_dir(), Arc::new(MockMemoryManager::new()));

        let entries = vec![(MessageRole::Assistant, "¿Hola? ¿Hay alguien ahí?".to_string())];
        let conversation = service.import_call_transcript(&profile.id, entries).await.unwrap();

        assert_eq!(conversation.title, "Nueva conversación");
    }

    #[tokio::test]
    async fn set_conversation_pinned_moves_it_above_the_rest() {
        let pool = test_pool().await;
        let profile = seed_profile(&pool).await;
        let event_bus = EventBus::new();
        let service = ChatService::new(pool.clone(), event_bus, std::env::temp_dir(), Arc::new(MockMemoryManager::new()));

        let older = service.create_conversation(&profile.id).await.unwrap();
        let _newer = service.create_conversation(&profile.id).await.unwrap();

        service.set_conversation_pinned(&older.id, true).await.unwrap();

        let list = service.list_conversations(&profile.id).await.unwrap();
        assert_eq!(list[0].id, older.id);
        assert!(list[0].is_pinned);
    }

    #[tokio::test]
    async fn reorder_pinned_conversations_applies_the_given_order() {
        let pool = test_pool().await;
        let profile = seed_profile(&pool).await;
        let event_bus = EventBus::new();
        let service = ChatService::new(pool.clone(), event_bus, std::env::temp_dir(), Arc::new(MockMemoryManager::new()));

        let a = service.create_conversation(&profile.id).await.unwrap();
        let b = service.create_conversation(&profile.id).await.unwrap();
        service.set_conversation_pinned(&a.id, true).await.unwrap();
        service.set_conversation_pinned(&b.id, true).await.unwrap();

        service.reorder_pinned_conversations(&[a.id.clone(), b.id.clone()]).await.unwrap();

        let list = service.list_conversations(&profile.id).await.unwrap();
        assert_eq!(list[0].id, a.id);
        assert_eq!(list[1].id, b.id);
    }
}
