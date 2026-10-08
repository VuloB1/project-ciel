import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";

import { profileVoiceSchema, type ProfileVoice } from "@/types/profile";

// Mirrors `aivt_provider_tts::VoiceInfo` (Rust).
const voiceInfoSchema = z.object({
  id: z.string(),
  name: z.string(),
  language: z.string().nullable(),
});
export type VoiceInfo = z.infer<typeof voiceInfoSchema>;

/** Only ever non-empty when the active backend is native — an HTTP backend
 * has no standard "list voices" endpoint, see `VoiceInfo`'s Rust doc comment. */
export async function listVoices(): Promise<VoiceInfo[]> {
  const result = await invoke("voice_list_voices");
  return z.array(voiceInfoSchema).parse(result);
}

/** Búsqueda por nombre en el catálogo de voces del servicio en uso (hoy solo
 * Fish Audio; el resto devuelve lista vacía). */
export async function searchVoices(query: string): Promise<VoiceInfo[]> {
  const result = await invoke("voice_search_voices", { query });
  return z.array(voiceInfoSchema).parse(result);
}

export async function getVoiceSettings(profileId: string): Promise<ProfileVoice> {
  const result = await invoke("voice_get_settings", { profileId });
  return profileVoiceSchema.parse(result);
}

export async function setVoiceSettings(profileId: string, input: ProfileVoice): Promise<ProfileVoice> {
  const result = await invoke("voice_set_settings", { profileId, input });
  return profileVoiceSchema.parse(result);
}

/** Manual playback — replaying a chat message (`messageId` set, always the
 * profile's *saved* voice) or the Voz tab's "Probar voz" preview
 * (`messageId` omitted, `voiceOverride` set to the unsaved draft so
 * previewing a slider tweak doesn't require saving first). Playback state
 * comes back via `AppEvent::VoicePlayback*`, not this call's return value. */
export async function speak(
  profileId: string,
  text: string,
  messageId?: string,
  voiceOverride?: ProfileVoice,
): Promise<void> {
  await invoke("voice_speak", { profileId, text, messageId: messageId ?? null, voiceOverride: voiceOverride ?? null });
}

export async function stopSpeaking(): Promise<void> {
  await invoke("voice_stop");
}

/** Global mute covering every voice-producing channel (Chat, Twitch,
 * Discord, Llamada) at once — see `commands::voice::voice_set_muted`
 * (Rust) for why muting also stops whatever's playing right now. */
export async function getVoiceMuted(): Promise<boolean> {
  const result = await invoke("voice_get_muted");
  return z.boolean().parse(result);
}

export async function setVoiceMuted(muted: boolean): Promise<boolean> {
  const result = await invoke("voice_set_muted", { muted });
  return z.boolean().parse(result);
}

/** Salidas de audio donde se pueden reproducir las voces (todas menos la voz del sistema). */
export async function listOutputDevices(): Promise<string[]> {
  const result = await invoke("voice_list_output_devices");
  return z.array(z.string()).parse(result);
}

/** `null` = la salida predeterminada del sistema. */
export async function getOutputDevice(): Promise<string | null> {
  const result = await invoke("voice_get_output_device");
  return z.string().nullable().parse(result);
}

export async function setOutputDevice(name: string | null): Promise<string | null> {
  const result = await invoke("voice_set_output_device", { name });
  return z.string().nullable().parse(result);
}
