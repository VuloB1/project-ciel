import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";

import type { ServiceStatus } from "@/lib/ipc/events";

// Mirrors `aivt_service_settings::{TwitchConnectionSettings, TwitchPolicy}` (Rust).

const twitchConnectionSettingsSchema = z.object({
  channelLogin: z.string(),
  broadcasterId: z.string(),
  hasTokens: z.boolean(),
  targetChannelLogin: z.string(),
  targetBroadcasterId: z.string(),
  profileImageUrl: z.string(),
});
export type TwitchConnectionSettings = z.infer<typeof twitchConnectionSettingsSchema>;

const twitchPolicySchema = z.object({
  enabled: z.boolean(),
  respondToAll: z.boolean(),
  mentionedOnly: z.boolean(),
  cooldownSeconds: z.number(),
});
export type TwitchPolicy = z.infer<typeof twitchPolicySchema>;

const serviceStatusSchema = z.enum(["stopped", "starting", "running", "error", "stopping"]);

// Mirrors `aivt_service_settings::TwitchEmoteSettings` (Rust).

const twitchEmoteSettingsSchema = z.object({ enabled: z.boolean() });
export type TwitchEmoteSettings = z.infer<typeof twitchEmoteSettingsSchema>;

const emoteCacheStatusSchema = z.object({ total: z.number(), described: z.number() });
export type EmoteCacheStatus = z.infer<typeof emoteCacheStatusSchema>;

// Mirrors `aivt_database::CachedEmote` (Rust).

const cachedEmoteSchema = z.object({
  source: z.string(),
  emoteId: z.string(),
  name: z.string(),
  tags: z.string().nullable(),
  imageUrl: z.string(),
  description: z.string().nullable(),
});
export type CachedEmote = z.infer<typeof cachedEmoteSchema>;

export async function getTwitchConnection(): Promise<TwitchConnectionSettings> {
  const result = await invoke("twitch_get_connection");
  return twitchConnectionSettingsSchema.parse(result);
}

export async function getTwitchPolicy(): Promise<TwitchPolicy> {
  const result = await invoke("twitch_get_policy");
  return twitchPolicySchema.parse(result);
}

export async function setTwitchPolicy(policy: TwitchPolicy): Promise<TwitchPolicy> {
  const result = await invoke("twitch_set_policy", { policy });
  return twitchPolicySchema.parse(result);
}

/** Opens the system browser for Twitch's authorize page and doesn't
 * resolve until the whole OAuth round trip finishes (or fails/times out)
 * — the caller should show a "waiting for authorization" state. */
export async function connectTwitchAccount(): Promise<TwitchConnectionSettings> {
  const result = await invoke("twitch_connect_account");
  return twitchConnectionSettingsSchema.parse(result);
}

export async function disconnectTwitchAccount(): Promise<void> {
  await invoke("twitch_disconnect_account");
}

export async function startTwitch(): Promise<void> {
  await invoke("twitch_start");
}

export async function stopTwitch(): Promise<void> {
  await invoke("twitch_stop");
}

export async function getTwitchStatus(): Promise<ServiceStatus | null> {
  const result = await invoke("twitch_get_status");
  return serviceStatusSchema.nullable().parse(result);
}

export async function setTwitchActiveProfile(profileId: string | null): Promise<void> {
  await invoke("twitch_set_active_profile", { profileId });
}

/** Runs the LLM + TTS reply pipeline against a locally-typed message,
 * without touching real Twitch chat — see `TwitchService::send_test_message`
 * (Rust) for why this exists: the connected account is also the one the
 * character posts as, so a solo streamer has no other way to trigger and
 * see/hear a reply. The result arrives as the usual `twitch_message_received`
 * / `twitch_message_sent` events, same as a real chat exchange. */
export async function sendTwitchTestMessage(text: string): Promise<void> {
  await invoke("twitch_send_test_message", { text });
}

/** Resolves and saves which channel the connected account joins/posts in
 * — see `TwitchService::set_target_channel` (Rust). Pass an empty string
 * to clear it (revert to "posts in its own channel", today's behavior). */
export async function setTwitchTargetChannel(channelLogin: string): Promise<TwitchConnectionSettings> {
  const result = await invoke("twitch_set_target_channel", { channelLogin });
  return twitchConnectionSettingsSchema.parse(result);
}

export async function getTwitchEmoteSettings(): Promise<TwitchEmoteSettings> {
  const result = await invoke("twitch_get_emote_settings");
  return twitchEmoteSettingsSchema.parse(result);
}

export async function setTwitchEmoteSettings(settings: TwitchEmoteSettings): Promise<TwitchEmoteSettings> {
  const result = await invoke("twitch_set_emote_settings", { settings });
  return twitchEmoteSettingsSchema.parse(result);
}

/** Fetches 7TV/BTTV/FFZ for the connected channel and caches the result —
 * see `TwitchService::refresh_emotes` (Rust). Awaited directly, not
 * fire-and-forget: a couple of HTTP round trips, not long enough to need
 * progress events. */
export async function refreshTwitchEmotes(): Promise<EmoteCacheStatus> {
  const result = await invoke("twitch_refresh_emotes");
  return emoteCacheStatusSchema.parse(result);
}

export async function getTwitchEmoteStatus(): Promise<EmoteCacheStatus> {
  const result = await invoke("twitch_get_emote_status");
  return emoteCacheStatusSchema.parse(result);
}

/** Fire-and-forget — real progress arrives via the `twitch_emote_caption_*`
 * events (see `lib/ipc/events.ts`), not this call's return value. */
export async function startEmoteCaptioning(): Promise<void> {
  await invoke("twitch_start_emote_captioning");
}

/** The cached emote set for the connected channel — see
 * `TwitchService::list_emotes` (Rust). Used to render known emote names as
 * images in the live feed (`TwitchPage`). */
export async function getTwitchEmotes(): Promise<CachedEmote[]> {
  const result = await invoke("twitch_get_emotes");
  return z.array(cachedEmoteSchema).parse(result);
}
