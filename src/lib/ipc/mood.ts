import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";

// Mirrors `aivt_database::ProfileMood` (Rust) — the character's evolving
// emotional state, distinct from `ProfilePersonality` (static config).
// See docs/ADR/006-mood.md.

const profileMoodSchema = z.object({
  profileId: z.string(),
  valence: z.number(),
  energy: z.number(),
  updatedAt: z.string(),
});
export type ProfileMood = z.infer<typeof profileMoodSchema>;

export async function getMood(profileId: string): Promise<ProfileMood> {
  const result = await invoke("mood_get", { profileId });
  return profileMoodSchema.parse(result);
}

/** Manual "reset" — back to the neutral 50/50 baseline. */
export async function resetMood(profileId: string): Promise<ProfileMood> {
  const result = await invoke("mood_reset", { profileId });
  return profileMoodSchema.parse(result);
}
