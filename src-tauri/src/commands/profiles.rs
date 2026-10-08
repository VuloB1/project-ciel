use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;

use aivt_core::AppError;
use aivt_database::{NewProfile, Profile, ProfileUpdate};
use aivt_orchestrator::PromptComposer;
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
    AppError::from_display("profiles", code, err)
}

#[tauri::command]
pub async fn profiles_list(state: tauri::State<'_, AppState>) -> Result<Vec<Profile>, AppError> {
    state.profiles.list().await.map_err(map_err)
}

/// Creates a profile from just a name — everything else starts at its
/// default and gets filled in on the Identidad page afterwards.
#[tauri::command]
pub async fn profiles_create(state: tauri::State<'_, AppState>, name: String) -> Result<Profile, AppError> {
    state
        .profiles
        .create(NewProfile { name, nickname: None, description: None })
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn profiles_update(
    state: tauri::State<'_, AppState>,
    id: String,
    input: ProfileUpdate,
) -> Result<Profile, AppError> {
    state.profiles.update(&id, input).await.map_err(map_err)
}

#[tauri::command]
pub async fn profiles_duplicate(
    state: tauri::State<'_, AppState>,
    id: String,
    new_name: String,
) -> Result<Profile, AppError> {
    state.profiles.duplicate(&id, &new_name).await.map_err(map_err)
}

#[tauri::command]
pub async fn profiles_delete(state: tauri::State<'_, AppState>, id: String) -> Result<(), AppError> {
    state.profiles.delete(&id).await.map_err(map_err)
}

/// `bytes_base64` comes from the frontend's `<input type="file">` read as a
/// data URL with the `data:...;base64,` prefix already stripped. Returns
/// the updated profile (with its new `avatar_path`).
#[tauri::command]
pub async fn profiles_set_avatar(
    state: tauri::State<'_, AppState>,
    id: String,
    bytes_base64: String,
    extension: String,
) -> Result<Profile, AppError> {
    let bytes = BASE64
        .decode(bytes_base64)
        .map_err(|err| AppError::from_display("profiles", "PROFILE_AVATAR_DECODE_FAILED", err))?;

    state.profiles.set_avatar(&id, &bytes, &extension).await.map_err(map_err)?;

    state
        .profiles
        .get(&id)
        .await
        .map_err(map_err)?
        .ok_or_else(|| map_err(ProfileError::NotFound))
}

/// Returns the profile's avatar as a base64 string (no data URL prefix —
/// the frontend adds `data:<mime>;base64,`), or `None` if it has none set.
#[tauri::command]
pub async fn profiles_get_avatar(state: tauri::State<'_, AppState>, id: String) -> Result<Option<String>, AppError> {
    let Some(profile) = state.profiles.get(&id).await.map_err(map_err)? else {
        return Ok(None);
    };
    let Some(avatar_path) = profile.avatar_path else {
        return Ok(None);
    };
    let bytes = state.profiles.read_avatar(&avatar_path).await.map_err(map_err)?;
    Ok(Some(BASE64.encode(bytes)))
}

/// Fase 11 Polish — la foto propia del usuario (no de un personaje),
/// mostrada junto a sus mensajes en el chat. Vive acá (no en
/// `commands/settings.rs`) porque el trabajo real es
/// `ProfileService::set_user_avatar` (dueño de la lógica de archivos de
/// avatar); solo el *path* resultante se guarda en `SettingsService`, ya
/// que no hay ninguna fila de `profiles` a la que asociarlo.
#[tauri::command]
pub async fn user_avatar_set(
    state: tauri::State<'_, AppState>,
    bytes_base64: String,
    extension: String,
) -> Result<(), AppError> {
    let bytes = BASE64
        .decode(bytes_base64)
        .map_err(|err| AppError::from_display("profiles", "USER_AVATAR_DECODE_FAILED", err))?;

    let relative_path = state.profiles.set_user_avatar(&bytes, &extension).await.map_err(map_err)?;
    state
        .settings
        .set_user_avatar_path(Some(&relative_path))
        .await
        .map_err(|err| AppError::from_display("settings", "USER_AVATAR_PATH_WRITE_FAILED", err))?;
    Ok(())
}

/// Same base64-no-prefix shape as `profiles_get_avatar` — `None` when the
/// user hasn't set one yet.
#[tauri::command]
pub async fn user_avatar_get(state: tauri::State<'_, AppState>) -> Result<Option<String>, AppError> {
    let Some(path) = state
        .settings
        .user_avatar_path()
        .await
        .map_err(|err| AppError::from_display("settings", "USER_AVATAR_PATH_READ_FAILED", err))?
    else {
        return Ok(None);
    };
    let bytes = state.profiles.read_avatar(&path).await.map_err(map_err)?;
    Ok(Some(BASE64.encode(bytes)))
}

/// `include_chats` — pedido del usuario: tildado por defecto en el diálogo
/// de exportar del frontend, pero se puede excluir el historial de chats
/// (ej. al compartir el personaje con otra persona). El archivo resultante
/// es un ZIP comprimido (`ProfileService::export`), no JSON plano.
#[tauri::command]
pub async fn profiles_export_to_path(
    state: tauri::State<'_, AppState>,
    id: String,
    path: String,
    include_chats: bool,
) -> Result<(), AppError> {
    let zip_bytes = state.profiles.export(&id, include_chats).await.map_err(map_err)?;
    tokio::fs::write(&path, zip_bytes)
        .await
        .map_err(|err| AppError::from_display("profiles", "PROFILE_EXPORT_WRITE_FAILED", err))
}

/// Consumida una sola vez al montar `App.tsx` — ver el doc comment de
/// `AppState::pending_import_path`. Devuelve `None` en un arranque normal,
/// o la ruta del `.aivtprofile` con el que se lanzó la app en frío
/// (dejando el estado en `None` después, para que un refresco de la
/// página no vuelva a disparar el diálogo de importar).
#[tauri::command]
pub fn profiles_take_pending_import_path(state: tauri::State<'_, AppState>) -> Option<String> {
    state.pending_import_path.lock().unwrap().take()
}

#[tauri::command]
pub async fn profiles_import_from_path(state: tauri::State<'_, AppState>, path: String) -> Result<Profile, AppError> {
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|err| AppError::from_display("profiles", "PROFILE_IMPORT_READ_FAILED", err))?;
    state.profiles.import(&bytes).await.map_err(map_err)
}

/// Live preview of the system prompt for the *draft* form state — doesn't
/// touch the database, so the Identidad page can call it on every change
/// without needing to save first (sección 12 del brief: "debe poder
/// inspeccionarse"). Renders in the app's current language (see
/// `AIOrchestrator::stream_respond`'s `language` doc comment) so the preview
/// matches what's actually sent to the model.
#[tauri::command]
pub async fn profiles_preview_prompt(state: tauri::State<'_, AppState>, input: ProfileUpdate) -> Result<String, AppError> {
    let draft = Profile {
        id: String::new(),
        name: input.name,
        nickname: input.nickname,
        avatar_path: None,
        description: input.description,
        age_fictional: input.age_fictional,
        pronouns: input.pronouns,
        occupation: input.occupation,
        species: input.species,
        accent_color: input.accent_color,
        theme: input.theme,
        is_archived: false,
        created_at: String::new(),
        updated_at: String::new(),
        identity: input.identity,
        personality: input.personality,
        voice: Default::default(),
        avatar_idle_path: None,
        avatar_thinking_path: None,
        avatar_talking_path: None,
    };
    let language = state.settings.language().await.unwrap_or_else(|_| "es".to_string());
    Ok(PromptComposer::compose(&draft, &language))
}
