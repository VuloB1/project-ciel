use aivt_core::AppError;
use aivt_service_backup::{BackupError, BackupSummary, RestorePreview};

use crate::state::AppState;

fn map_err(err: BackupError) -> AppError {
    let code = match &err {
        BackupError::Database(_) => "BACKUP_DATABASE_ERROR",
        BackupError::Io(_) => "BACKUP_IO_ERROR",
        BackupError::Zip(_) | BackupError::Invalid(_) => "BACKUP_INVALID",
        BackupError::Join(_) => "BACKUP_TASK_FAILED",
    };
    AppError::from_display("backup", code, err)
}

/// Crea una copia completa de la app en `path`: base de datos, adjuntos y avatares.
/// No incluye las API keys ni los tokens (viven en el almacén seguro del sistema).
#[tauri::command]
pub async fn backup_create(state: tauri::State<'_, AppState>, path: String) -> Result<BackupSummary, AppError> {
    aivt_service_backup::create_backup(&state.pool, &state.data_dir, std::path::Path::new(&path)).await.map_err(map_err)
}

/// Valida una copia y la deja preparada. NO cambia nada todavía: la restauración se
/// aplica al reiniciar la app (`backup_restart_app`), con la base aún sin abrir.
#[tauri::command]
pub async fn backup_prepare_restore(state: tauri::State<'_, AppState>, path: String) -> Result<RestorePreview, AppError> {
    aivt_service_backup::stage_restore(std::path::Path::new(&path), &state.data_dir).await.map_err(map_err)
}

/// El usuario se arrepintió después de preparar una restauración.
#[tauri::command]
pub async fn backup_cancel_restore(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    aivt_service_backup::discard_pending_restore(&state.data_dir).await;
    Ok(())
}

/// Cierra la app y la vuelve a abrir (sin argumentos) para aplicar la restauración.
///
/// `app.restart()` no sirve acá: lanza el proceso nuevo ANTES de que el actual
/// termine, y `tauri-plugin-single-instance` lo ve como "segunda instancia" y
/// lo cierra — la app se apagaba y no volvía a abrirse. Tampoco debe reenviar
/// los argumentos originales: si la restauración vino de un doble clic en un
/// `.aivtbackup`, el proceso nuevo volvería a pedir la misma confirmación.
/// En Windows se lanza un `cmd` oculto que espera unos segundos a que este
/// proceso (y su base de datos) se cierren del todo y recién entonces abre la
/// app. Ojo con dos cosas que ya fallaron: `DETACHED_PROCESS` junto a
/// `CREATE_NO_WINDOW` hace que Windows ignore la segunda y muestre una consola,
/// y `current_exe()` devuelve una ruta con prefijo `\\?\` que `cmd` no entiende
/// (aparecía el error "Windows no encuentra el archivo").
#[tauri::command]
pub fn backup_restart_app(app: tauri::AppHandle) -> Result<(), AppError> {
    let exe = std::env::current_exe().map_err(|err| AppError::from_display("backup", "BACKUP_RESTART_FAILED", err))?;

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let exe_text = exe.to_string_lossy();
        let exe_text = exe_text.strip_prefix("\\\\?\\").unwrap_or(&exe_text).to_string();
        std::process::Command::new("cmd")
            // `raw_arg`, no `args`: Rust escapa las comillas como `\"` y `cmd` no
            // entiende ese escape (`start \"\"` buscaba un archivo llamado `\\`).
            .raw_arg("/C ping -n 4 127.0.0.1 >nul & start \"\" \"%CIEL_EXE%\"")
            .env("CIEL_EXE", exe_text)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|err| AppError::from_display("backup", "BACKUP_RESTART_FAILED", err))?;
        app.exit(0);
        Ok(())
    }

    #[cfg(not(windows))]
    {
        let _ = exe;
        app.restart()
    }
}
