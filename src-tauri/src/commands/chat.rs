use std::sync::Arc;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;

use aivt_core::AppError;
use aivt_database::{Conversation, Memory, MessageRole, Profile};
use aivt_orchestrator::AIOrchestrator;
use aivt_provider_llm::LLMProvider;
use aivt_provider_tools::{BraveSearchTool, CurrentTimeTool, Tool};
use aivt_service_chat::{ChatError, GenerationContext, MessageView, NewAttachmentInput, TtsInvocation};
use aivt_service_settings::ChatAudioSettings;

use crate::state::AppState;

const PROVIDER_LABEL: &str = "openai_compatible";

fn map_err(err: ChatError) -> AppError {
    let code = match &err {
        ChatError::ConversationNotFound => "CHAT_CONVERSATION_NOT_FOUND",
        ChatError::GenerationInProgress => "CHAT_GENERATION_IN_PROGRESS",
        ChatError::Database(_) => "CHAT_DATABASE_ERROR",
        ChatError::Io(_) => "CHAT_ATTACHMENT_IO_ERROR",
        ChatError::AttachmentNotFound => "CHAT_ATTACHMENT_NOT_FOUND",
        ChatError::Memory(_) => "CHAT_MEMORY_ERROR",
    };
    AppError::from_display("chat", code, err)
}

/// What every command that needs to actually talk to the LLM (sending a
/// message, extracting memories, proposing a memory fusion) resolves from
/// Configuración > Proveedores IA — pulled into one place so
/// `chat_send_message`/`chat_extract_memories` don't each repeat the same
/// settings/provider construction. `pub(crate)` so `commands::memory`'s
/// fusion commands can reuse it too — same reasoning as
/// `commands::memory::map_err` being `pub(crate)` for `commands::mood`.
pub(crate) struct ResolvedConnection {
    pub(crate) provider: Arc<dyn LLMProvider>,
    pub(crate) model: String,
    pub(crate) temperature: f32,
    pub(crate) max_tokens: u32,
    pub(crate) vision_enabled: bool,
}

pub(crate) async fn resolve_llm_connection(state: &AppState) -> Result<ResolvedConnection, AppError> {
    let connection = state
        .settings
        .llm_connection()
        .await
        .map_err(|err| AppError::from_display("chat", "CHAT_SETTINGS_READ_FAILED", err))?
        .ok_or_else(|| {
            AppError::new(
                "chat",
                "CHAT_NO_LLM_CONNECTION",
                "No hay ningún proveedor de IA configurado. Ve a Configuración > Proveedores IA.",
            )
        })?;

    let provider = state
        .settings
        .build_llm_provider(&state.event_bus)
        .await
        .map_err(|err| AppError::from_display("chat", "CHAT_SETTINGS_READ_FAILED", err))?;

    Ok(ResolvedConnection {
        provider,
        model: connection.model,
        temperature: connection.temperature,
        max_tokens: connection.max_tokens,
        vision_enabled: connection.vision_enabled,
    })
}

/// Best-effort: a settings read failure here means "auto-play skipped this
/// turn", never a failed chat send — same stance as memory retrieval
/// failures inside `run_generation`. Almost always resolves to *something*
/// (the `Native` singleton always exists), unlike `resolve_llm_connection`.
async fn resolve_tts_invocation(state: &AppState, profile: &Profile) -> Option<TtsInvocation> {
    let backend = state.settings.build_tts_backend().await.ok()?;
    let save_audio = state.settings.chat_audio_settings().await.map(|s| s.save_enabled).unwrap_or(false);
    Some(TtsInvocation { provider: state.tts.clone(), backend, voice: profile.voice.clone(), save_audio })
}

#[tauri::command]
pub async fn chat_list_conversations(
    state: tauri::State<'_, AppState>,
    profile_id: String,
) -> Result<Vec<Conversation>, AppError> {
    state.chat.list_conversations(&profile_id).await.map_err(map_err)
}

#[tauri::command]
pub async fn chat_create_conversation(
    state: tauri::State<'_, AppState>,
    profile_id: String,
) -> Result<Conversation, AppError> {
    state.chat.create_conversation(&profile_id).await.map_err(map_err)
}

#[tauri::command]
pub async fn chat_get_messages(state: tauri::State<'_, AppState>, conversation_id: String) -> Result<Vec<MessageView>, AppError> {
    state.chat.get_messages(&conversation_id).await.map_err(map_err)
}

/// Returns a stored attachment as base64 (no data URL prefix — the
/// frontend adds `data:<mime>;base64,`), mirroring
/// `profiles_get_avatar`/`ProfileService::read_avatar`.
#[tauri::command]
pub async fn chat_get_attachment_base64(
    state: tauri::State<'_, AppState>,
    attachment_id: String,
) -> Result<String, AppError> {
    let (bytes, _mime_type) = state.chat.get_attachment(&attachment_id).await.map_err(map_err)?;
    Ok(BASE64.encode(bytes))
}

#[tauri::command]
pub async fn chat_rename_conversation(state: tauri::State<'_, AppState>, id: String, title: String) -> Result<(), AppError> {
    state.chat.rename_conversation(&id, &title).await.map_err(map_err)
}

#[tauri::command]
pub async fn chat_archive_conversation(state: tauri::State<'_, AppState>, id: String) -> Result<(), AppError> {
    state.chat.archive_conversation(&id).await.map_err(map_err)
}

#[tauri::command]
pub async fn chat_delete_conversation(state: tauri::State<'_, AppState>, id: String) -> Result<(), AppError> {
    state.chat.delete_conversation(&id).await.map_err(map_err)
}

#[tauri::command]
pub async fn chat_cancel_generation(state: tauri::State<'_, AppState>, conversation_id: String) -> Result<(), AppError> {
    state.chat.cancel_generation(&conversation_id).await;
    Ok(())
}

/// An image the frontend read via `<input type="file">` — `base64` has
/// already had the `data:...;base64,` prefix stripped, matching
/// `profiles_set_avatar`'s convention.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatImageInput {
    pub base64: String,
    pub mime_type: String,
    pub extension: String,
}

/// Persists the user's message and kicks off the assistant response in the
/// background — the frontend follows along via `AppEvent::AiResponse*`
/// (see `src/lib/ipc/events.ts`), not by awaiting this command.
#[tauri::command]
pub async fn chat_send_message(
    state: tauri::State<'_, AppState>,
    profile_id: String,
    conversation_id: String,
    content: String,
    image: Option<ChatImageInput>,
) -> Result<MessageView, AppError> {
    let profile = state
        .profiles
        .get(&profile_id)
        .await
        .map_err(|err| AppError::from_display("chat", "CHAT_PROFILE_LOOKUP_FAILED", err))?
        .ok_or_else(|| AppError::new("chat", "CHAT_PROFILE_NOT_FOUND", "El perfil activo ya no existe"))?;

    let resolved = resolve_llm_connection(&state).await?;

    // Defense in depth: the composer already hides the attach button when
    // vision isn't enabled, but a stale UI state or a direct IPC call
    // shouldn't silently drop an image the user thinks they're sending.
    if image.is_some() && !resolved.vision_enabled {
        return Err(AppError::new(
            "chat",
            "CHAT_VISION_NOT_ENABLED",
            "El modelo activo no tiene la visión habilitada. Actívala en Configuración > Proveedores IA.",
        ));
    }

    let attachment = match image {
        Some(image) => {
            let bytes = BASE64
                .decode(&image.base64)
                .map_err(|err| AppError::from_display("chat", "CHAT_ATTACHMENT_DECODE_FAILED", err))?;
            Some(NewAttachmentInput {
                bytes,
                mime_type: image.mime_type.clone(),
                extension: image.extension,
                data_url: format!("data:{};base64,{}", image.mime_type, image.base64),
            })
        }
        None => None,
    };

    let language = state.settings.language().await.unwrap_or_else(|_| "es".to_string());
    let mut orchestrator = AIOrchestrator::new(
        resolved.provider.clone(),
        resolved.model.clone(),
        Some(resolved.temperature),
        Some(resolved.max_tokens),
    );
    // `get_current_time` is always available — pure local calculation,
    // no network/API key, so it doesn't need the same opt-in as
    // `web_search` (Fase 10b). `web_search` itself stays behind
    // `WebSearchSettings.enabled` — see its doc comment.
    let mut tools: Vec<Arc<dyn Tool>> = vec![Arc::new(CurrentTimeTool::new(language.as_str()))];
    if let Ok(web_search) = state.settings.web_search_settings().await {
        if web_search.enabled {
            if let Ok(Some(key)) = state.settings.web_search_api_key().await {
                tools.push(Arc::new(BraveSearchTool::new(key, language.as_str())));
            }
        }
    }
    orchestrator = orchestrator.with_tools(tools);
    let tts = resolve_tts_invocation(&state, &profile).await;

    let ctx = GenerationContext {
        profile,
        orchestrator: Arc::new(orchestrator),
        model: resolved.model,
        provider_label: PROVIDER_LABEL.to_string(),
        llm_provider: resolved.provider,
        tts,
        language,
    };

    state.chat.send_message(&conversation_id, &content, attachment, ctx).await.map_err(map_err)
}

/// Runs memory extraction immediately over whatever's new since the last
/// consolidation pass, ignoring the usual message-count threshold — lets
/// the user (or this session's live verification) see it work without
/// having to send several messages first.
#[tauri::command]
pub async fn chat_extract_memories(
    state: tauri::State<'_, AppState>,
    conversation_id: String,
    profile_id: String,
) -> Result<Vec<Memory>, AppError> {
    let resolved = resolve_llm_connection(&state).await?;
    let language = state.settings.language().await.unwrap_or_else(|_| "es".to_string());
    state
        .chat
        .extract_memories_now(&conversation_id, &profile_id, resolved.provider, &resolved.model, &language)
        .await
        .map_err(map_err)
}

/// One line of a Llamada transcript, as the frontend's `CallFeedMessage`
/// shape (`"user" | "character" | "error"`) — the command layer maps
/// `"character"` to `MessageRole::Assistant` and drops `"error"` entries
/// (they're diagnostic, not something either party actually said) before
/// this ever reaches `ChatService`.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallTranscriptEntryInput {
    pub from: String,
    pub text: String,
}

/// "Exportar a Chat" desde la pestaña Llamada — ver
/// `ChatService::import_call_transcript`'s doc comment para el porqué.
#[tauri::command]
pub async fn chat_import_call_transcript(
    state: tauri::State<'_, AppState>,
    profile_id: String,
    entries: Vec<CallTranscriptEntryInput>,
) -> Result<Conversation, AppError> {
    let mapped = entries
        .into_iter()
        .filter(|entry| entry.from != "error" && !entry.text.trim().is_empty())
        .map(|entry| {
            let role = if entry.from == "user" { MessageRole::User } else { MessageRole::Assistant };
            (role, entry.text)
        })
        .collect();

    state.chat.import_call_transcript(&profile_id, mapped).await.map_err(map_err)
}

#[tauri::command]
pub async fn chat_set_conversation_pinned(state: tauri::State<'_, AppState>, id: String, pinned: bool) -> Result<(), AppError> {
    state.chat.set_conversation_pinned(&id, pinned).await.map_err(map_err)
}

#[tauri::command]
pub async fn chat_reorder_pinned_conversations(
    state: tauri::State<'_, AppState>,
    ordered_ids: Vec<String>,
) -> Result<(), AppError> {
    state.chat.reorder_pinned_conversations(&ordered_ids).await.map_err(map_err)
}

#[tauri::command]
pub async fn chat_get_audio_settings(state: tauri::State<'_, AppState>) -> Result<ChatAudioSettings, AppError> {
    state
        .settings
        .chat_audio_settings()
        .await
        .map_err(|err| AppError::from_display("chat", "CHAT_AUDIO_SETTINGS_READ_FAILED", err))
}

#[tauri::command]
pub async fn chat_set_audio_settings(
    state: tauri::State<'_, AppState>,
    settings: ChatAudioSettings,
) -> Result<ChatAudioSettings, AppError> {
    state
        .settings
        .set_chat_audio_settings(settings)
        .await
        .map_err(|err| AppError::from_display("chat", "CHAT_AUDIO_SETTINGS_WRITE_FAILED", err))
}
