import { create } from "zustand";

import { getVoiceMuted, setVoiceMuted, speak, stopSpeaking } from "@/lib/ipc/voice";
import type { AppEvent } from "@/lib/ipc/events";
import type { ProfileVoice } from "@/types/profile";

interface VoiceState {
  /** `null` when nothing is speaking. `messageId` is `null` for a Voz tab
   * "Probar voz" preview — see `voice_speak`'s Rust doc comment. */
  speaking: { profileId: string; messageId: string | null } | null;
  lastError: string | null;
  /** Pedido hecho pero todavía sin sonar (el motor remoto está generando el
   * audio). Permite mostrar "stop" ya en ese tramo, que dura segundos. */
  pending: { profileId: string; messageId: string | null } | null;
  /** `null` until `loadMuted` resolves — the Sidebar button stays
   * disabled until then, same pattern as `discordPolicy` in
   * `discordStore`. Global mute covering Chat/Twitch/Discord/Llamada at
   * once, see `commands::voice::voice_set_muted` (Rust). */
  muted: boolean | null;

  /** Fire-and-forget: playback state is reflected back via
   * `AppEvent::VoicePlayback*`, not this call's return value. */
  speak: (profileId: string, text: string, messageId?: string, voiceOverride?: ProfileVoice) => Promise<void>;
  stop: () => Promise<void>;
  loadMuted: () => Promise<void>;
  setMuted: (muted: boolean) => Promise<void>;
  handleAppEvent: (event: AppEvent) => void;
}

export const useVoiceStore = create<VoiceState>((set) => ({
  speaking: null,
  lastError: null,
  pending: null,
  muted: null,

  speak: async (profileId, text, messageId, voiceOverride) => {
    const mine = { profileId, messageId: messageId ?? null };
    set({ lastError: null, pending: mine });
    try {
      await speak(profileId, text, messageId, voiceOverride);
    } finally {
      set((state) => (state.pending === mine ? { pending: null } : state));
    }
  },

  stop: async () => {
    set({ pending: null });
    await stopSpeaking();
  },

  loadMuted: async () => {
    try {
      const muted = await getVoiceMuted();
      set({ muted });
    } catch {
      // Sidebar's mute button just stays disabled — same fallback as
      // `discordStore.load`'s catch.
    }
  },

  setMuted: async (muted) => {
    const saved = await setVoiceMuted(muted);
    set({ muted: saved });
  },

  handleAppEvent: (event) => {
    switch (event.type) {
      case "voice_playback_started":
        set({ speaking: { profileId: event.profileId, messageId: event.messageId }, lastError: null });
        return;
      case "voice_playback_finished":
        set((state) =>
          state.speaking?.profileId === event.profileId && state.speaking.messageId === event.messageId
            ? { speaking: null }
            : state,
        );
        return;
      case "voice_playback_error":
        set((state) =>
          state.speaking?.profileId === event.profileId && state.speaking.messageId === event.messageId
            ? { speaking: null, lastError: event.message }
            : { lastError: event.message },
        );
        return;
      default:
        return;
    }
  },
}));
