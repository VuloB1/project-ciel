import { invoke } from "@tauri-apps/api/core";

// Usados por `TrayMenuWindow.tsx` (la ventanita del clic derecho en el
// ícono de la bandeja) — comandos propios en vez de `@tauri-apps/api/window`
// para no tener que habilitar permisos `core:window:*` en las capabilities
// solo para esa ventana; ver `commands/window.rs`.

export async function showMainWindow(): Promise<void> {
  await invoke("tray_show_main_window");
}

export async function quitApp(): Promise<void> {
  await invoke("tray_quit_app");
}

export async function reloadMainWindow(): Promise<void> {
  await invoke("tray_reload_main_window");
}

export async function resizeMenuWindow(height: number): Promise<void> {
  await invoke("tray_resize_menu_window", { height });
}

export async function hideCurrentWindow(): Promise<void> {
  await invoke("hide_window");
}
