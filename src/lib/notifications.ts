import { invoke } from "@tauri-apps/api/core";
import { isPermissionGranted, requestPermission } from "@tauri-apps/plugin-notification";

const PREVIEW_MAX_LENGTH = 140;

function truncate(text: string): string {
  const trimmed = text.trim();
  return trimmed.length > PREVIEW_MAX_LENGTH ? `${trimmed.slice(0, PREVIEW_MAX_LENGTH - 1)}…` : trimmed;
}

/** Aviso de escritorio cuando el personaje escribe por su cuenta (modo
 * proactivo) — el permiso se pide recién la primera vez que hace falta
 * (nunca al arrancar la app), y si el usuario lo niega no se vuelve a
 * insistir en esta sesión: la próxima llamada simplemente no hace nada.
 * El sonido lo pone el propio sistema operativo al mostrar la
 * notificación.
 *
 * Se manda por `notifications_send_proactive` (comando de Rust) en vez de
 * `sendNotification()` del plugin de JS, porque este último ignora
 * cualquier ícono en Windows — el comando de Rust arma el toast directo
 * con el avatar del personaje como ícono circular, estilo WhatsApp. */
export async function notifyProactiveMessage(profileId: string, title: string, body: string): Promise<void> {
  let granted = await isPermissionGranted();
  if (!granted) {
    const permission = await requestPermission();
    granted = permission === "granted";
  }
  if (!granted) return;

  await invoke("notifications_send_proactive", { profileId, title, body: truncate(body) });
}
