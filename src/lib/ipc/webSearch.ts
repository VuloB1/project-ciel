import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";

// Mirrors `aivt_service_settings::{WebSearchSettings, WebSearchInput}`
// (Rust) — Fase 10b. Opt-in web search tool for the character, backed by
// the Brave Search API (the only backend implemented so far).

const webSearchSettingsSchema = z.object({ enabled: z.boolean(), hasApiKey: z.boolean() });
export type WebSearchSettings = z.infer<typeof webSearchSettingsSchema>;

export async function getWebSearchSettings(): Promise<WebSearchSettings> {
  const result = await invoke("web_search_get_settings");
  return webSearchSettingsSchema.parse(result);
}

/** `apiKey: undefined` keeps whatever key is already saved (same contract
 * as the LLM/TTS connection inputs); pass `""` to clear it. */
export async function setWebSearchSettings(enabled: boolean, apiKey?: string): Promise<WebSearchSettings> {
  const result = await invoke("web_search_set_settings", { input: { enabled, apiKey: apiKey ?? null } });
  return webSearchSettingsSchema.parse(result);
}
