import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";

// Mirrors `aivt_service_settings::{SttModelSize, SttModelSettings, SttModelInput}`
// and `aivt_provider_stt::WhisperModelSize` (Rust) — STT now runs embedded
// (whisper.cpp) rather than talking to a server, so there's no connection/
// API-key shape here, just which model size is selected/downloaded.

export const sttModelSizeSchema = z.enum(["tiny", "base", "small", "medium"]);
export type SttModelSize = z.infer<typeof sttModelSizeSchema>;

const sttModelSettingsSchema = z.object({ size: sttModelSizeSchema, gpuEnabled: z.boolean() });
export type SttModelSettings = z.infer<typeof sttModelSettingsSchema>;

const sttModelStatusSchema = z.object({
  size: sttModelSizeSchema,
  downloaded: z.boolean(),
  approxSizeMb: z.number(),
});
export type SttModelStatus = z.infer<typeof sttModelStatusSchema>;

export async function getSttModel(): Promise<SttModelSettings> {
  const result = await invoke("stt_get_model");
  return sttModelSettingsSchema.parse(result);
}

export async function setSttModel(size: SttModelSize, gpuEnabled: boolean): Promise<SttModelSettings> {
  const result = await invoke("stt_set_model", { input: { size, gpuEnabled } });
  return sttModelSettingsSchema.parse(result);
}

export async function listSttModelStatus(): Promise<SttModelStatus[]> {
  const result = await invoke("stt_list_model_status");
  return z.array(sttModelStatusSchema).parse(result);
}

/** Returns once the download has *started* — actual progress arrives via
 * `AppEvent` (`stt_model_download_progress`/`_finished`/`_error`), not the
 * resolution of this promise. See `commands::stt::stt_download_model`. */
export async function downloadSttModel(size: SttModelSize): Promise<void> {
  await invoke("stt_download_model", { size });
}

export async function deleteSttModel(size: SttModelSize): Promise<void> {
  await invoke("stt_delete_model", { size });
}

// ---- Dictado por voz (botón de micrófono del chat) -----------------------
// Mirrors `aivt_service_settings::DictationSettings` and the `dictation_*`
// commands in `src-tauri/src/commands/stt.rs`.

const dictationSettingsSchema = z.object({ autoSend: z.boolean() });
export type DictationSettings = z.infer<typeof dictationSettingsSchema>;

export async function getDictationSettings(): Promise<DictationSettings> {
  const result = await invoke("dictation_get_settings");
  return dictationSettingsSchema.parse(result);
}

export async function setDictationSettings(settings: DictationSettings): Promise<DictationSettings> {
  const result = await invoke("dictation_set_settings", { settings });
  return dictationSettingsSchema.parse(result);
}

/** Empieza a grabar el micrófono. Rechaza con un mensaje claro si hay una
 * llamada en curso o no hay un modelo de transcripción descargado. */
export async function startDictation(): Promise<void> {
  await invoke("dictation_start");
}

/** Termina la grabación y devuelve el texto transcrito ("" si no se entendió). */
export async function stopDictation(profileId: string): Promise<string> {
  const result = await invoke("dictation_stop", { profileId });
  return z.string().parse(result);
}

export async function cancelDictation(): Promise<void> {
  await invoke("dictation_cancel");
}
