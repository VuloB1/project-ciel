import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";

import { profileSchema, type Profile, type ProfileUpdateInput } from "@/types/profile";

export async function listProfiles(): Promise<Profile[]> {
  const result = await invoke("profiles_list");
  return z.array(profileSchema).parse(result);
}

export async function createProfile(name: string): Promise<Profile> {
  const result = await invoke("profiles_create", { name });
  return profileSchema.parse(result);
}

export async function updateProfile(id: string, input: ProfileUpdateInput): Promise<Profile> {
  const result = await invoke("profiles_update", { id, input });
  return profileSchema.parse(result);
}

export async function duplicateProfile(id: string, newName: string): Promise<Profile> {
  const result = await invoke("profiles_duplicate", { id, newName });
  return profileSchema.parse(result);
}

export async function deleteProfile(id: string): Promise<void> {
  await invoke("profiles_delete", { id });
}

export async function setProfileAvatar(id: string, bytesBase64: string, extension: string): Promise<Profile> {
  const result = await invoke("profiles_set_avatar", { id, bytesBase64, extension });
  return profileSchema.parse(result);
}

export async function getProfileAvatarBase64(id: string): Promise<string | null> {
  const result = await invoke("profiles_get_avatar", { id });
  return z.string().nullable().parse(result);
}

export async function exportProfileToPath(id: string, path: string, includeChats: boolean): Promise<void> {
  await invoke("profiles_export_to_path", { id, path, includeChats });
}

export async function importProfileFromPath(path: string): Promise<Profile> {
  const result = await invoke("profiles_import_from_path", { path });
  return profileSchema.parse(result);
}

/** Consumida una sola vez al montar la app — ver el doc comment de
 * `AppState::pending_import_path` (Rust). `null` en un arranque normal. */
export async function takePendingImportPath(): Promise<string | null> {
  const result = await invoke("profiles_take_pending_import_path");
  return z.string().nullable().parse(result);
}

export async function previewProfilePrompt(input: ProfileUpdateInput): Promise<string> {
  const result = await invoke("profiles_preview_prompt", { input });
  return z.string().parse(result);
}
