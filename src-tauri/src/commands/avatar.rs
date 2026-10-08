use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use serde::Serialize;

use aivt_core::AppError;
use aivt_database::{AvatarState, Profile};
use aivt_eventbus::ServiceStatus;
use aivt_service_profiles::ProfileError;

use crate::state::AppState;

fn map_err(err: ProfileError) -> AppError {
    let code = match &err {
        ProfileError::NotFound => "PROFILE_NOT_FOUND",
        ProfileError::Database(_) => "PROFILE_DATABASE_ERROR",
        ProfileError::Io(_) => "PROFILE_IO_ERROR",
        ProfileError::InvalidExport(_) => "PROFILE_INVALID_EXPORT",
        ProfileError::Image(_) => "PROFILE_IMAGE_ERROR",
        ProfileError::ExportFormat(_) => "PROFILE_EXPORT_FORMAT_ERROR",
    };
    AppError::from_display("avatar", code, err)
}

fn parse_state(state: &str) -> Result<AvatarState, AppError> {
    match state {
        "idle" => Ok(AvatarState::Idle),
        "thinking" => Ok(AvatarState::Thinking),
        "talking" => Ok(AvatarState::Talking),
        other => Err(AppError::new("avatar", "AVATAR_INVALID_STATE", format!("Estado de avatar desconocido: {other}"))),
    }
}

/// `bytes_base64` comes from the frontend's `<input type="file">`, same
/// convention as `profiles_set_avatar`. `state` is one of
/// `"idle" | "thinking" | "talking"`.
#[tauri::command]
pub async fn avatar_set_state_image(
    state_handle: tauri::State<'_, AppState>,
    id: String,
    state: String,
    bytes_base64: String,
    extension: String,
) -> Result<Profile, AppError> {
    let avatar_state = parse_state(&state)?;
    let bytes = BASE64
        .decode(bytes_base64)
        .map_err(|err| AppError::from_display("avatar", "AVATAR_DECODE_FAILED", err))?;

    state_handle.profiles.set_avatar_state(&id, avatar_state, &bytes, &extension).await.map_err(map_err)?;

    state_handle
        .profiles
        .get(&id)
        .await
        .map_err(map_err)?
        .ok_or_else(|| map_err(ProfileError::NotFound))
}

/// Returns one state's image as base64 (no data URL prefix), or `None` if
/// that state has no image of its own yet — the frontend falls back to the
/// profile's regular picture in that case (see `useAvatarStateImage`).
#[tauri::command]
pub async fn avatar_get_state_image(
    state_handle: tauri::State<'_, AppState>,
    id: String,
    state: String,
) -> Result<Option<String>, AppError> {
    let avatar_state = parse_state(&state)?;
    let Some(profile) = state_handle.profiles.get(&id).await.map_err(map_err)? else {
        return Ok(None);
    };
    let path = match avatar_state {
        AvatarState::Idle => profile.avatar_idle_path,
        AvatarState::Thinking => profile.avatar_thinking_path,
        AvatarState::Talking => profile.avatar_talking_path,
    };
    let Some(path) = path else {
        return Ok(None);
    };
    let bytes = state_handle.profiles.read_avatar(&path).await.map_err(map_err)?;
    Ok(Some(BASE64.encode(bytes)))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvatarServerStatus {
    pub enabled: bool,
    pub port: u16,
    pub status: Option<ServiceStatus>,
}

async fn build_status(state: &AppState) -> Result<AvatarServerStatus, AppError> {
    let settings = state
        .settings
        .avatar_server_settings()
        .await
        .map_err(|err| AppError::from_display("avatar", "AVATAR_SERVER_SETTINGS_READ_FAILED", err))?;
    Ok(AvatarServerStatus {
        enabled: settings.enabled,
        port: settings.port,
        status: state.service_manager.status("avatar_server").await,
    })
}

/// Current on/off + port + live `ServiceManager` status of the OBS Browser
/// Source server — read on mount by `AvatarEditor.tsx` to show the right
/// toggle state and URL without assuming anything about what the last
/// session left it in.
#[tauri::command]
pub async fn avatar_server_get_status(state: tauri::State<'_, AppState>) -> Result<AvatarServerStatus, AppError> {
    build_status(&state).await
}

/// Flips the persisted `enabled` flag and starts/stops the
/// `ManagedService` live in the same call — see `aivt_service_avatar` and
/// `run()::setup`'s boot-time auto-resume for the other half of this.
#[tauri::command]
pub async fn avatar_server_set_enabled(state: tauri::State<'_, AppState>, enabled: bool) -> Result<AvatarServerStatus, AppError> {
    let mut settings = state
        .settings
        .avatar_server_settings()
        .await
        .map_err(|err| AppError::from_display("avatar", "AVATAR_SERVER_SETTINGS_READ_FAILED", err))?;
    settings.enabled = enabled;
    state
        .settings
        .set_avatar_server_settings(settings)
        .await
        .map_err(|err| AppError::from_display("avatar", "AVATAR_SERVER_SETTINGS_WRITE_FAILED", err))?;

    let toggle = if enabled {
        state.service_manager.start("avatar_server").await
    } else {
        state.service_manager.stop("avatar_server").await
    };
    toggle.map_err(|err| AppError::from_display("avatar", "AVATAR_SERVER_TOGGLE_FAILED", err))?;

    build_status(&state).await
}
