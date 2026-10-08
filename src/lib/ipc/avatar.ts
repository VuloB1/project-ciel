import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";

import type { ServiceStatus } from "@/lib/ipc/events";
import { profileSchema, type Profile } from "@/types/profile";

// Mirrors `aivt_database::AvatarState` (Rust) — "reactive PNG first"
// (Fase 7), see docs/DEVELOPMENT_STATUS.md.
export type AvatarVisualState = "idle" | "thinking" | "talking";

export async function setAvatarStateImage(
  id: string,
  state: AvatarVisualState,
  bytesBase64: string,
  extension: string,
): Promise<Profile> {
  const result = await invoke("avatar_set_state_image", { id, state, bytesBase64, extension });
  return profileSchema.parse(result);
}

/** `null` when that state has no image of its own — callers fall back to
 * the profile's regular picture (see `useAvatarStateImage`). */
export async function getAvatarStateImage(id: string, state: AvatarVisualState): Promise<string | null> {
  const result = await invoke("avatar_get_state_image", { id, state });
  return z.string().nullable().parse(result);
}

const serviceStatusSchema = z.enum(["stopped", "starting", "running", "error", "stopping"]);

const avatarServerStatusSchema = z.object({
  enabled: z.boolean(),
  port: z.number(),
  status: serviceStatusSchema.nullable(),
});

export interface AvatarServerStatus {
  enabled: boolean;
  port: number;
  status: ServiceStatus | null;
}

/** Current on/off + port + live `ServiceManager` status of the OBS Browser
 * Source server (see `aivt_service_avatar`) — read on mount so the toggle
 * and URL reflect reality regardless of what the last session left it in. */
export async function getAvatarServerStatus(): Promise<AvatarServerStatus> {
  const result = await invoke("avatar_server_get_status");
  return avatarServerStatusSchema.parse(result);
}

/** Flips the persisted setting and starts/stops the local server live. */
export async function setAvatarServerEnabled(enabled: boolean): Promise<AvatarServerStatus> {
  const result = await invoke("avatar_server_set_enabled", { enabled });
  return avatarServerStatusSchema.parse(result);
}
