use tauri::AppHandle;

use aivt_core::AppError;
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
    AppError::from_display("notifications", code, err)
}

/// Aviso de escritorio para un mensaje proactivo, con el avatar del
/// personaje como ícono circular (estilo WhatsApp) cuando tiene uno — el
/// plugin de JS de `tauri-plugin-notification` no expone esa opción en
/// Windows, así que este comando construye el toast directo con
/// `tauri-winrt-notification` en vez de pasar por `sendNotification()`.
#[tauri::command]
pub async fn notifications_send_proactive(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    profile_id: String,
    title: String,
    body: String,
) -> Result<(), AppError> {
    let icon_path = state.profiles.notification_icon_path(&profile_id).await.map_err(map_err)?;

    show_toast(&app, &title, &body, icon_path.as_deref())
        .map_err(|err| AppError::from_display("notifications", "TOAST_SHOW_FAILED", err))
}

#[cfg(windows)]
fn show_toast(
    app: &AppHandle,
    title: &str,
    body: &str,
    icon_path: Option<&std::path::Path>,
) -> tauri_winrt_notification::Result<()> {
    use tauri_winrt_notification::{IconCrop, Toast};

    let app_id = app.config().identifier.clone();
    let mut toast = Toast::new(&app_id).title(title).text1(body);

    // `exists()` evita mandarle a Windows una ruta rota si el personaje no
    // tiene avatar propio todavía — el toast simplemente sale sin ícono.
    if let Some(path) = icon_path.filter(|path| path.exists()) {
        toast = toast.icon(path, IconCrop::Circular, title);
    }

    toast.show()
}

#[cfg(not(windows))]
fn show_toast(
    app: &AppHandle,
    title: &str,
    body: &str,
    _icon_path: Option<&std::path::Path>,
) -> tauri_plugin_notification::Result<()> {
    use tauri_plugin_notification::NotificationExt;

    app.notification().builder().title(title).body(body).show()
}
