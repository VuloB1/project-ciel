import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";

import type { ServiceStatus } from "@/lib/ipc/events";

// Mirrors `aivt_service_settings::{DiscordConnectionSettings, DiscordPolicy}` (Rust).

const discordConnectionSettingsSchema = z.object({
  botUsername: z.string(),
  botUserId: z.string(),
  hasToken: z.boolean(),
  avatarUrl: z.string(),
});
export type DiscordConnectionSettings = z.infer<typeof discordConnectionSettingsSchema>;

const discordPolicySchema = z.object({
  enabled: z.boolean(),
  respondToAll: z.boolean(),
  mentionedOnly: z.boolean(),
  cooldownSeconds: z.number(),
});
export type DiscordPolicy = z.infer<typeof discordPolicySchema>;

const serviceStatusSchema = z.enum(["stopped", "starting", "running", "error", "stopping"]);

export async function getDiscordConnection(): Promise<DiscordConnectionSettings> {
  const result = await invoke("discord_get_connection");
  return discordConnectionSettingsSchema.parse(result);
}

export async function getDiscordPolicy(): Promise<DiscordPolicy> {
  const result = await invoke("discord_get_policy");
  return discordPolicySchema.parse(result);
}

export async function setDiscordPolicy(policy: DiscordPolicy): Promise<DiscordPolicy> {
  const result = await invoke("discord_set_policy", { policy });
  return discordPolicySchema.parse(result);
}

/** Validates and saves the bot token in one step — no browser, no OAuth
 * round trip. See `DiscordService::connect_bot` (Rust). */
export async function connectDiscordBot(token: string): Promise<DiscordConnectionSettings> {
  const result = await invoke("discord_connect_bot", { token });
  return discordConnectionSettingsSchema.parse(result);
}

export async function disconnectDiscordAccount(): Promise<void> {
  await invoke("discord_disconnect_account");
}

export async function startDiscord(): Promise<void> {
  await invoke("discord_start");
}

export async function stopDiscord(): Promise<void> {
  await invoke("discord_stop");
}

export async function getDiscordStatus(): Promise<ServiceStatus | null> {
  const result = await invoke("discord_get_status");
  return serviceStatusSchema.nullable().parse(result);
}

export async function setDiscordActiveProfile(profileId: string | null): Promise<void> {
  await invoke("discord_set_active_profile", { profileId });
}

/** Runs the LLM + TTS reply pipeline against a locally-typed message,
 * without touching the real Discord API. The result arrives as the usual
 * `discord_message_received` / `discord_message_sent` events, same as a
 * real message exchange. */
export async function sendDiscordTestMessage(text: string): Promise<void> {
  await invoke("discord_send_test_message", { text });
}
