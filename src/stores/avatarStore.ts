import { create } from "zustand";

import type { AvatarVisualState } from "@/lib/ipc/avatar";
import type { AppEvent } from "@/lib/ipc/events";

interface AvatarStoreState {
  visualState: AvatarVisualState;
  /** Which profile this store instance reacts to — set to the active
   * profile so the Avatar tab's "en vivo" badge tracks it. The OBS Browser
   * Source itself no longer runs any of this in-process (it's a plain
   * browser tab talking to `aivt_service_avatar`'s own, server-side copy
   * of this same state machine — see that crate's `reducer.rs`); this
   * store now exists purely for the in-app preview. */
  trackedProfileId: string | null;
  /** Correlates a completion/error/cancel event back to the `ai_response_started`
   * that set "thinking" — those events only carry `requestId`, not
   * `profileId`, so this is how we avoid reacting to a different profile's
   * concurrent generation finishing. */
  activeRequestId: string | null;

  setTrackedProfileId: (profileId: string | null) => void;
  handleAppEvent: (event: AppEvent) => void;
}

export const useAvatarStore = create<AvatarStoreState>((set, get) => ({
  visualState: "idle",
  trackedProfileId: null,
  activeRequestId: null,

  setTrackedProfileId: (profileId) => set({ trackedProfileId: profileId, visualState: "idle", activeRequestId: null }),

  handleAppEvent: (event) => {
    const trackedId = get().trackedProfileId;
    if (!trackedId) return;

    switch (event.type) {
      case "ai_response_started":
        if (event.profileId === trackedId) {
          set({ visualState: "thinking", activeRequestId: event.requestId });
        }
        return;
      case "ai_response_finished":
      case "ai_response_cancelled":
      case "ai_response_error":
        if (event.requestId === get().activeRequestId) {
          set({ visualState: "idle", activeRequestId: null });
        }
        return;
      case "voice_playback_started":
        if (event.profileId === trackedId) set({ visualState: "talking" });
        return;
      case "voice_playback_finished":
      case "voice_playback_error":
        if (event.profileId === trackedId) set({ visualState: "idle" });
        return;
      // Call events (Fase 6) carry no `profileId` — unlike Chat/TTS,
      // there's only ever one active call and one tracked profile at a time
      // (`CallService`'s session is a singleton), so there's no
      // cross-profile mismatch to guard against here the way
      // `activeRequestId` guards Chat's concurrent generations. Without
      // this, the avatar never left "idle" during a call — nothing else
      // sets "thinking" outside Chat's own `ai_response_started` (pedido
      // del usuario: "el avatar no enseña que esta pensando" en Llamada).
      case "call_state_changed":
        if (event.state === "transcribing" || event.state === "thinking") {
          set({ visualState: "thinking" });
        } else if (event.state === "speaking") {
          set({ visualState: "talking" });
        } else {
          set({ visualState: "idle" });
        }
        return;
      default:
        return;
    }
  },
}));
