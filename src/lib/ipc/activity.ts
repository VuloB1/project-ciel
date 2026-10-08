import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";

// Mirrors `aivt_database::ActivityEventRow` (Rust) — Fase 11. Read-only
// feed of everything already being recorded into `activity_events` via
// the `tracing` → `ActivityLayer` bridge (Chat/Twitch/Discord/LLM/Memoria).

export const activityLevelSchema = z.enum(["DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"]);
export type ActivityLevel = z.infer<typeof activityLevelSchema>;

export const activityEventSchema = z.object({
  id: z.string(),
  timestamp: z.string(),
  // A level string the backend didn't already know about shouldn't break
  // rendering the rest of the feed — fall back to INFO rather than throw.
  level: activityLevelSchema.catch("INFO"),
  category: z.string(),
  module: z.string(),
  action: z.string(),
  profileId: z.string().nullable(),
  conversationId: z.string().nullable(),
  requestId: z.string().nullable(),
  durationMs: z.number().nullable(),
  status: z.string(),
  summary: z.string(),
  detailsJson: z.string().nullable(),
});
export type ActivityEvent = z.infer<typeof activityEventSchema>;

export async function listRecentActivity(): Promise<ActivityEvent[]> {
  const result = await invoke("activity_recent");
  return z.array(activityEventSchema).parse(result);
}

export async function clearActivity(): Promise<void> {
  await invoke("activity_clear");
}
