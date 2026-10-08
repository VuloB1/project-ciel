use aivt_core::AppError;
use aivt_eventbus::ServiceStatus;
use aivt_service_discord::DiscordError;
use aivt_service_settings::{DiscordConnectionSettings, DiscordPolicy};

use crate::state::AppState;

fn map_err(err: DiscordError) -> AppError {
    let code = match &err {
        DiscordError::NotConnected => "DISCORD_NOT_CONNECTED",
        DiscordError::MissingToken => "DISCORD_MISSING_TOKEN",
        DiscordError::InvalidToken(_) => "DISCORD_INVALID_TOKEN",
        DiscordError::Network(_) => "DISCORD_NETWORK_ERROR",
        DiscordError::GatewayConnect(_) => "DISCORD_GATEWAY_CONNECT_FAILED",
        DiscordError::GatewayClosed => "DISCORD_GATEWAY_CLOSED",
        DiscordError::Database(_) => "DISCORD_DATABASE_ERROR",
        DiscordError::Settings(_) => "DISCORD_SETTINGS_ERROR",
        DiscordError::Secret(_) => "DISCORD_SECRET_ERROR",
        DiscordError::NoActiveProfile => "DISCORD_NO_ACTIVE_PROFILE",
        DiscordError::NoLlmConnection => "DISCORD_NO_LLM_CONNECTION",
        DiscordError::Generation(_) => "DISCORD_GENERATION_FAILED",
        DiscordError::EmptyResponse => "DISCORD_EMPTY_RESPONSE",
    };
    AppError::from_display("discord", code, err)
}

#[tauri::command]
pub async fn discord_get_connection(state: tauri::State<'_, AppState>) -> Result<DiscordConnectionSettings, AppError> {
    state.settings.discord_connection().await.map_err(|err| AppError::from_display("discord", "DISCORD_SETTINGS_READ_FAILED", err))
}

#[tauri::command]
pub async fn discord_get_policy(state: tauri::State<'_, AppState>) -> Result<DiscordPolicy, AppError> {
    state.settings.discord_policy().await.map_err(|err| AppError::from_display("discord", "DISCORD_SETTINGS_READ_FAILED", err))
}

#[tauri::command]
pub async fn discord_set_policy(state: tauri::State<'_, AppState>, policy: DiscordPolicy) -> Result<DiscordPolicy, AppError> {
    state.settings.set_discord_policy(policy).await.map_err(|err| AppError::from_display("discord", "DISCORD_SETTINGS_WRITE_FAILED", err))
}

/// Validates and saves the bot token in one step — no browser, no OAuth
/// round trip, unlike `twitch_connect_account`. See
/// `DiscordService::connect_bot`.
#[tauri::command]
pub async fn discord_connect_bot(state: tauri::State<'_, AppState>, token: String) -> Result<DiscordConnectionSettings, AppError> {
    state.discord.connect_bot(token).await.map_err(map_err)
}

#[tauri::command]
pub async fn discord_disconnect_account(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    let _ = state.service_manager.stop("discord").await;
    state.discord.disconnect().await.map_err(map_err)
}

#[tauri::command]
pub async fn discord_start(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    state.service_manager.start("discord").await.map_err(|err| AppError::from_display("discord", "DISCORD_START_FAILED", err))
}

#[tauri::command]
pub async fn discord_stop(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    state.service_manager.stop("discord").await.map_err(|err| AppError::from_display("discord", "DISCORD_STOP_FAILED", err))
}

#[tauri::command]
pub async fn discord_get_status(state: tauri::State<'_, AppState>) -> Result<Option<ServiceStatus>, AppError> {
    Ok(state.service_manager.status("discord").await)
}

/// The frontend calls this whenever the active profile changes — see
/// `twitch_set_active_profile` for why the backend has no other way to
/// know which character should answer in Discord.
#[tauri::command]
pub async fn discord_set_active_profile(state: tauri::State<'_, AppState>, profile_id: Option<String>) -> Result<(), AppError> {
    state.discord.set_active_profile(profile_id).await;
    Ok(())
}

/// Runs the reply pipeline (LLM + TTS) against a locally-typed message
/// without touching the real Discord API — see
/// `DiscordService::send_test_message`.
#[tauri::command]
pub async fn discord_send_test_message(state: tauri::State<'_, AppState>, text: String) -> Result<(), AppError> {
    state.discord.send_test_message(text).await.map_err(map_err)
}
