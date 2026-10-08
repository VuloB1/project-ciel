import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";

import type { ServiceStatus } from "@/lib/ipc/events";

// Mirrors `aivt_service_settings::{CallPolicy, VadMode}` (Rust).

export const vadModeSchema = z.enum(["quality", "low_bitrate", "aggressive", "very_aggressive"]);
export type VadMode = z.infer<typeof vadModeSchema>;

const callPolicySchema = z.object({
  vadMode: vadModeSchema,
  silenceTimeoutMs: z.number(),
  microphone: z.string().nullable(),
  respondToAll: z.boolean(),
  wakePhrase: z.string().nullable(),
  echoGuard: z.boolean(),
});
export type CallPolicy = z.infer<typeof callPolicySchema>;

const serviceStatusSchema = z.enum(["stopped", "starting", "running", "error", "stopping"]);

export async function getCallPolicy(): Promise<CallPolicy> {
  const result = await invoke("call_get_policy");
  return callPolicySchema.parse(result);
}

export async function setCallPolicy(policy: CallPolicy): Promise<CallPolicy> {
  const result = await invoke("call_set_policy", { policy });
  return callPolicySchema.parse(result);
}

export async function startCall(): Promise<void> {
  await invoke("call_start");
}

export async function stopCall(): Promise<void> {
  await invoke("call_stop");
}

export async function getCallStatus(): Promise<ServiceStatus | null> {
  const result = await invoke("call_get_status");
  return serviceStatusSchema.nullable().parse(result);
}

export async function setCallActiveProfile(profileId: string | null): Promise<void> {
  await invoke("call_set_active_profile", { profileId });
}

export async function listMicrophones(): Promise<string[]> {
  const result = await invoke("call_list_microphones");
  return z.array(z.string()).parse(result);
}

export async function setCallMuted(muted: boolean): Promise<void> {
  await invoke("call_set_muted", { muted });
}

export async function getCallMuted(): Promise<boolean> {
  const result = await invoke("call_is_muted");
  return z.boolean().parse(result);
}

/** Abre el micrófono solo para mostrar su nivel (evento `call_mic_level`), sin
 * transcribir. No hace nada si ya hay una llamada en curso. */
export async function startMicTest(): Promise<void> {
  await invoke("call_mic_test_start");
}

export async function stopMicTest(): Promise<void> {
  await invoke("call_mic_test_stop");
}
