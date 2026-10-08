use tauri_plugin_opener::OpenerExt;

use aivt_core::AppError;
use aivt_database::CachedEmote;
use aivt_eventbus::ServiceStatus;
use aivt_service_settings::{TwitchConnectionSettings, TwitchEmoteSettings, TwitchPolicy};
use aivt_service_twitch::TwitchError;

use crate::state::AppState;

fn map_err(err: TwitchError) -> AppError {
    let code = match &err {
        TwitchError::NotConnected => "TWITCH_NOT_CONNECTED",
        TwitchError::BrowserOpenFailed(_) => "TWITCH_BROWSER_OPEN_FAILED",
        TwitchError::AuthorizationTimedOut => "TWITCH_AUTH_TIMED_OUT",
        TwitchError::AuthorizationDenied(_) => "TWITCH_AUTH_DENIED",
        TwitchError::Network(_) => "TWITCH_NETWORK_ERROR",
        TwitchError::Api { .. } => "TWITCH_API_ERROR",
        TwitchError::ChannelNotFound(_) => "TWITCH_CHANNEL_NOT_FOUND",
        TwitchError::EventSubConnect(_) => "TWITCH_EVENTSUB_CONNECT_FAILED",
        TwitchError::EventSubClosed => "TWITCH_EVENTSUB_CLOSED",
        TwitchError::Database(_) => "TWITCH_DATABASE_ERROR",
        TwitchError::Settings(_) => "TWITCH_SETTINGS_ERROR",
        TwitchError::Secret(_) => "TWITCH_SECRET_ERROR",
        TwitchError::NoActiveProfile => "TWITCH_NO_ACTIVE_PROFILE",
        TwitchError::NoLlmConnection => "TWITCH_NO_LLM_CONNECTION",
        TwitchError::Generation(_) => "TWITCH_GENERATION_FAILED",
        TwitchError::EmptyResponse => "TWITCH_EMPTY_RESPONSE",
    };
    AppError::from_display("twitch", code, err)
}

#[tauri::command]
pub async fn twitch_get_connection(state: tauri::State<'_, AppState>) -> Result<TwitchConnectionSettings, AppError> {
    state.settings.twitch_connection().await.map_err(|err| AppError::from_display("twitch", "TWITCH_SETTINGS_READ_FAILED", err))
}

#[tauri::command]
pub async fn twitch_get_policy(state: tauri::State<'_, AppState>) -> Result<TwitchPolicy, AppError> {
    state.settings.twitch_policy().await.map_err(|err| AppError::from_display("twitch", "TWITCH_SETTINGS_READ_FAILED", err))
}

#[tauri::command]
pub async fn twitch_set_policy(state: tauri::State<'_, AppState>, policy: TwitchPolicy) -> Result<TwitchPolicy, AppError> {
    state.settings.set_twitch_policy(policy).await.map_err(|err| AppError::from_display("twitch", "TWITCH_SETTINGS_WRITE_FAILED", err))
}

/// Runs the whole Device Code flow and only returns once it's done (or
/// failed/timed out) — the frontend awaits this with a spinner rather than
/// polling connection state. `verification_uri` already has the device
/// code baked in as a query param (Twitch's own convention), so opening it
/// pre-fills the code — the user just needs to approve. See
/// `TwitchService::begin_oauth`/`complete_oauth`.
#[tauri::command]
pub async fn twitch_connect_account(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> Result<TwitchConnectionSettings, AppError> {
    let flow = state.twitch.begin_oauth().await.map_err(map_err)?;

    app.opener()
        .open_url(&flow.verification_uri, None::<&str>)
        .map_err(|err| AppError::from_display("twitch", "TWITCH_BROWSER_OPEN_FAILED", err))?;

    state.twitch.complete_oauth(flow).await.map_err(map_err)?;

    state.settings.twitch_connection().await.map_err(|err| AppError::from_display("twitch", "TWITCH_SETTINGS_READ_FAILED", err))
}

#[tauri::command]
pub async fn twitch_disconnect_account(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    let _ = state.service_manager.stop("twitch").await;
    state.twitch.disconnect().await.map_err(map_err)
}

#[tauri::command]
pub async fn twitch_start(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    state.service_manager.start("twitch").await.map_err(|err| AppError::from_display("twitch", "TWITCH_START_FAILED", err))
}

#[tauri::command]
pub async fn twitch_stop(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    state.service_manager.stop("twitch").await.map_err(|err| AppError::from_display("twitch", "TWITCH_STOP_FAILED", err))
}

#[tauri::command]
pub async fn twitch_get_status(state: tauri::State<'_, AppState>) -> Result<Option<ServiceStatus>, AppError> {
    Ok(state.service_manager.status("twitch").await)
}

/// The frontend calls this whenever the active profile changes — see
/// `TwitchService`'s doc comment for why the backend has no other way to
/// know which character should answer in chat.
#[tauri::command]
pub async fn twitch_set_active_profile(state: tauri::State<'_, AppState>, profile_id: Option<String>) -> Result<(), AppError> {
    state.twitch.set_active_profile(profile_id).await;
    Ok(())
}

/// Runs the reply pipeline (LLM + TTS) against a locally-typed message
/// without touching real Twitch chat — see `TwitchService::send_test_message`
/// for why this exists: the connected account is also the one the
/// character posts as, so a solo streamer has no other way to trigger a
/// reply and see/hear it.
#[tauri::command]
pub async fn twitch_send_test_message(state: tauri::State<'_, AppState>, text: String) -> Result<(), AppError> {
    state.twitch.send_test_message(text).await.map_err(map_err)
}

/// Resolves and saves which channel the connected account joins/posts in
/// — see `TwitchService::set_target_channel`. Empty string clears it
/// (revert to "posts in its own channel").
#[tauri::command]
pub async fn twitch_set_target_channel(state: tauri::State<'_, AppState>, channel_login: String) -> Result<TwitchConnectionSettings, AppError> {
    state.twitch.set_target_channel(channel_login).await.map_err(map_err)
}

#[tauri::command]
pub async fn twitch_get_emote_settings(state: tauri::State<'_, AppState>) -> Result<TwitchEmoteSettings, AppError> {
    state.settings.twitch_emote_settings().await.map_err(|err| AppError::from_display("twitch", "TWITCH_SETTINGS_READ_FAILED", err))
}

#[tauri::command]
pub async fn twitch_set_emote_settings(state: tauri::State<'_, AppState>, settings: TwitchEmoteSettings) -> Result<TwitchEmoteSettings, AppError> {
    state.settings.set_twitch_emote_settings(settings).await.map_err(|err| AppError::from_display("twitch", "TWITCH_SETTINGS_WRITE_FAILED", err))
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmoteCacheStatus {
    pub total: i64,
    pub described: i64,
}

/// Fetches 7TV/BTTV/FFZ for the connected channel and caches the result —
/// awaited directly (a couple of HTTP round trips, not long enough to
/// warrant progress events) — see `TwitchService::refresh_emotes`.
#[tauri::command]
pub async fn twitch_refresh_emotes(state: tauri::State<'_, AppState>) -> Result<EmoteCacheStatus, AppError> {
    let (total, described) = state.twitch.refresh_emotes().await.map_err(map_err)?;
    Ok(EmoteCacheStatus { total, described })
}

#[tauri::command]
pub async fn twitch_get_emote_status(state: tauri::State<'_, AppState>) -> Result<EmoteCacheStatus, AppError> {
    let (total, described) = state.twitch.emote_status().await.map_err(map_err)?;
    Ok(EmoteCacheStatus { total, described })
}

/// The cached emote set for the connected channel — the live Twitch feed
/// (`TwitchPage`) uses this to render known emote names inline as images,
/// same cache the AI's own prompt reads from (`TwitchEmoteSettings`). See
/// `TwitchService::list_emotes`.
#[tauri::command]
pub async fn twitch_get_emotes(state: tauri::State<'_, AppState>) -> Result<Vec<CachedEmote>, AppError> {
    state.twitch.list_emotes().await.map_err(map_err)
}

/// Fire-and-forget — real progress travels through `TwitchEmoteCaption*`
/// events, not this command's return value. See
/// `TwitchService::start_emote_captioning`.
#[tauri::command]
pub async fn twitch_start_emote_captioning(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    state.twitch.start_emote_captioning().await.map_err(map_err)
}
