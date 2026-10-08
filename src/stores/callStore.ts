import { create } from "zustand";

import { getCallPolicy, getCallStatus, setCallMuted, setCallPolicy, startCall, stopCall, type CallPolicy } from "@/lib/ipc/call";
import type { AppEvent, CallState, ServiceStatus } from "@/lib/ipc/events";

/** Bounded so a long call doesn't grow this without limit — only the
 * transcript shown in the Llamada tab reads this, nothing persists it. */
const MAX_FEED_MESSAGES = 200;

export interface CallFeedMessage {
  id: string;
  from: "user" | "character" | "error";
  text: string;
  at: string;
}

interface CallStoreState {
  status: ServiceStatus | null;
  callState: CallState | null;
  policy: CallPolicy | null;
  feed: CallFeedMessage[];
  isLoading: boolean;
  isToggling: boolean;
  isMuted: boolean;
  /** Volumen del micrófono, 0 a 1 (evento `call_mic_level`). */
  micLevel: number;

  load: () => Promise<void>;
  savePolicy: (policy: CallPolicy) => Promise<void>;
  start: () => Promise<void>;
  stop: () => Promise<void>;
  toggleMute: () => Promise<void>;
  handleAppEvent: (event: AppEvent) => void;
}

export const useCallStore = create<CallStoreState>((set, get) => ({
  status: null,
  callState: null,
  policy: null,
  feed: [],
  isLoading: false,
  isToggling: false,
  isMuted: false,
  micLevel: 0,

  load: async () => {
    set({ isLoading: true });
    try {
      const [policy, status] = await Promise.all([getCallPolicy(), getCallStatus()]);
      set({ policy, status, isLoading: false });
    } catch {
      set({ isLoading: false });
    }
  },

  savePolicy: async (policy) => {
    const saved = await setCallPolicy(policy);
    set({ policy: saved });
  },

  start: async () => {
    set({ isToggling: true });
    try {
      await startCall();
      // The backend always starts a new session unmuted (see
      // `CallService::start`'s doc comment).
      set({ status: "running", isMuted: false });
    } finally {
      set({ isToggling: false });
    }
  },

  stop: async () => {
    set({ isToggling: true });
    try {
      await stopCall();
      set({ status: "stopped", callState: null, isMuted: false, micLevel: 0 });
    } finally {
      set({ isToggling: false });
    }
  },

  toggleMute: async () => {
    const next = !get().isMuted;
    set({ isMuted: next });
    try {
      await setCallMuted(next);
    } catch {
      // Revert on failure — the backend's mute state (whether or not a
      // call is even active) is the source of truth.
      set({ isMuted: !next });
    }
  },

  handleAppEvent: (event) => {
    switch (event.type) {
      case "service_status_changed":
        if (event.service === "call") set({ status: event.status });
        return;
      case "call_state_changed":
        set({ callState: event.state });
        return;
      case "call_mic_level":
        set({ micLevel: Math.min(1, Math.max(0, event.level)) });
        return;
      case "call_user_utterance": {
        const entry: CallFeedMessage = { id: crypto.randomUUID(), from: "user", text: event.text, at: new Date().toISOString() };
        set((state) => ({ feed: [...state.feed, entry].slice(-MAX_FEED_MESSAGES) }));
        return;
      }
      case "call_character_reply": {
        const entry: CallFeedMessage = { id: crypto.randomUUID(), from: "character", text: event.text, at: new Date().toISOString() };
        set((state) => ({ feed: [...state.feed, entry].slice(-MAX_FEED_MESSAGES) }));
        return;
      }
      case "call_error": {
        const entry: CallFeedMessage = { id: crypto.randomUUID(), from: "error", text: event.message, at: new Date().toISOString() };
        set((state) => ({ feed: [...state.feed, entry].slice(-MAX_FEED_MESSAGES) }));
        return;
      }
      default:
        return;
    }
  },
}));
