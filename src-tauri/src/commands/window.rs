//! Comandos para la ventanita propia del clic derecho en el ícono de la
//! bandeja (`TrayMenuWindow.tsx`), que reemplaza el menú nativo del SO para
//! poder seguir el tema de la app (colores, redondez, densidad) — ver el
//! `TrayIconBuilder` en `lib.rs`. Ese mismo `lib.rs` también llama
//! `tray_show_main_window` directamente (no solo vía `invoke`) para el clic
//! izquierdo del ícono, que hace lo mismo que el botón "Mostrar" de la
//! ventanita.

use tauri::Manager;

#[tauri::command]
pub fn tray_show_main_window(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[tauri::command]
pub fn tray_quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

/// Recarga la interfaz de la ventana principal desde la bandeja — reemplaza
/// al ítem "Recargar interfaz" que antes vivía en el menú contextual normal
/// (`AppContextMenu.tsx`); se movió acá para que no quede siempre a la
/// vista como si el programa necesitara reiniciarse seguido, y esté
/// disponible igual para el caso de emergencia (interfaz colgada).
#[tauri::command]
pub fn tray_reload_main_window(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.eval("window.location.reload()");
    }
}

/// Ajusta el alto real de `tray-menu` al contenido (`ResizeObserver` en
/// `TrayMenuWindow.tsx`) en vez de depender de un alto fijo en
/// `tauri.conf.json` — así agregar/sacar ítems del menú, o un cambio de
/// densidad/escala de fuente en Apariencia, nunca vuelve a dejar el último
/// ítem recortado. El ancho se mantiene fijo (208, igual que `tauri.conf.json`
/// y `MENU_WIDTH` en `AppContextMenu.tsx`) porque el posicionamiento en
/// `lib.rs` centra la ventana en torno al clic asumiendo ese ancho.
#[tauri::command]
pub fn tray_resize_menu_window(window: tauri::WebviewWindow, height: f64) {
    let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize { width: 208.0, height }));
}

/// Esconde la ventana que invoca este comando — lo usa `TrayMenuWindow.tsx`
/// para Escape en vez de los comandos `core:window:*` (que necesitarían su
/// propio permiso ACL habilitado solo para esa ventana; perder el foco ya
/// la esconde sola vía `WindowEvent::Focused(false)` en `lib.rs`, pero
/// Escape no dispara ese evento por sí solo).
#[tauri::command]
pub fn hide_window(window: tauri::WebviewWindow) {
    let _ = window.hide();
}
