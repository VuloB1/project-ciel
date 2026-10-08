import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";

// Mirrors `aivt_database::ProactiveSettings` (Rust) — whether the
// character may start a conversation on its own. See
// docs/ADR/007-proactive-mode.md.

const proactiveSettingsSchema = z.object({
  profileId: z.string(),
  enabled: z.boolean(),
  lastTriggeredAt: z.string().nullable(),
});
export type ProactiveSettings = z.infer<typeof proactiveSettingsSchema>;

export async function getProactiveSettings(profileId: string): Promise<ProactiveSettings> {
  const result = await invoke("proactive_get_settings", { profileId });
  return proactiveSettingsSchema.parse(result);
}

export async function setProactiveEnabled(profileId: string, enabled: boolean): Promise<ProactiveSettings> {
  const result = await invoke("proactive_set_enabled", { profileId, enabled });
  return proactiveSettingsSchema.parse(result);
}

/** Igual que `setTwitchActiveProfile`/`setCallActiveProfile` — el motor de
 * decisión corre en segundo plano y no tiene otra forma de saber qué
 * personaje está activo en la ventana principal. */
export async function setProactiveActiveProfile(profileId: string | null): Promise<void> {
  await invoke("proactive_set_active_profile", { profileId });
}
