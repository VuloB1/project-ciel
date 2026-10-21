import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";

// Mirrors `aivt_service_settings::{WebSearchSettings, WebSearchInput}`
// (Rust) — Fase 10b. Opt-in web search tool for the character, backed by
// the Brave Search API (the only backend implemented so far).

const webSearchSettingsSchema = z.object({ enabled: z.boolean(), hasApiKey: z.boolean(), readPages: z.boolean() });
export type WebSearchSettings = z.infer<typeof webSearchSettingsSchema>;

export async function getWebSearchSettings(): Promise<WebSearchSettings> {
  const result = await invoke("web_search_get_settings");
  return webSearchSettingsSchema.parse(result);
}

/** `apiKey: undefined` keeps whatever key is already saved (same contract
 * as the LLM/TTS connection inputs); pass `""` to clear it. */
export async function setWebSearchSettings(enabled: boolean, apiKey?: string, readPages?: boolean): Promise<WebSearchSettings> {
  const result = await invoke("web_search_set_settings", { input: { enabled, apiKey: apiKey ?? null, readPages: readPages ?? null } });
  return webSearchSettingsSchema.parse(result);
}

// Obscura: the optional headless browser behind the `read_url` tool, downloaded on demand.
const obscuraStatusSchema = z.object({ supported: z.boolean(), installed: z.boolean(), approxSizeMb: z.number() });
export type ObscuraStatus = z.infer<typeof obscuraStatusSchema>;

export async function getObscuraStatus(): Promise<ObscuraStatus> {
  return obscuraStatusSchema.parse(await invoke("obscura_get_status"));
}

/** Starts the download in the background; progress arrives as `obscura_download_*` events. */
export async function downloadObscura(): Promise<void> {
  await invoke("obscura_download");
}

export async function deleteObscura(): Promise<void> {
  await invoke("obscura_delete");
}
