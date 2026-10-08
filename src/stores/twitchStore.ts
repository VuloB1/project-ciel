import { create } from "zustand";

import {
  connectTwitchAccount,
  disconnectTwitchAccount,
  getTwitchConnection,
  getTwitchEmoteSettings,
  getTwitchEmotes,
  getTwitchPolicy,
  getTwitchStatus,
  sendTwitchTestMessage,
  setTwitchPolicy,
  setTwitchTargetChannel,
  startTwitch,
  stopTwitch,
  type TwitchConnectionSettings,
  type TwitchPolicy,
} from "@/lib/ipc/twitch";
import type { AppEvent, ServiceStatus } from "@/lib/ipc/events";

/** Bounded so a very active chat doesn't grow this without limit — only
 * the live feed shown in the Twitch tab reads this, nothing persists it. */
const MAX_FEED_MESSAGES = 200;

export interface TwitchFeedMessage {
  id: string;
  from: "chat" | "character";
  displayName: string;
  text: string;
  at: string;
}

interface TwitchState {
  connection: TwitchConnectionSettings | null;
  policy: TwitchPolicy | null;
  status: ServiceStatus | null;
  feed: TwitchFeedMessage[];
  /** Emote name → image URL, for the feed to render known emotes inline
   * instead of as plain text — empty when the emotes feature is off or
   * nothing's cached yet. See `loadEmoteMap`. */
  emoteMap: Record<string, string>;
  isLoading: boolean;
  isConnecting: boolean;
  isSendingTestMessage: boolean;

  load: () => Promise<void>;
  loadEmoteMap: () => Promise<void>;
  saveTargetChannel: (channelLogin: string) => Promise<void>;
  savePolicy: (policy: TwitchPolicy) => Promise<void>;
  connectAccount: () => Promise<void>;
  disconnectAccount: () => Promise<void>;
  start: () => Promise<void>;
  stop: () => Promise<void>;
  sendTestMessage: (text: string) => Promise<void>;
  handleAppEvent: (event: AppEvent) => void;
}

export const useTwitchStore = create<TwitchState>((set, get) => ({
  connection: null,
  policy: null,
  status: null,
  feed: [],
  emoteMap: {},
  isLoading: false,
  isConnecting: false,
  isSendingTestMessage: false,

  load: async () => {
    set({ isLoading: true });
    try {
      const [connection, policy, status] = await Promise.all([getTwitchConnection(), getTwitchPolicy(), getTwitchStatus()]);
      set({ connection, policy, status, isLoading: false });
    } catch {
      set({ isLoading: false });
    }
    void get().loadEmoteMap();
  },

  /** Re-reads the emote toggle + cached set — called on load and again
   * whenever the settings dialog changes either (toggle, refresh list) so
   * the feed doesn't need a full reload to pick them up. Silently leaves
   * `emoteMap` empty on any failure — rendering emotes is a nice-to-have,
   * never worth blocking or erroring the feed over. */
  loadEmoteMap: async () => {
    try {
      const settings = await getTwitchEmoteSettings();
      if (!settings.enabled) {
        set({ emoteMap: {} });
        return;
      }
      const emotes = await getTwitchEmotes();
      set({ emoteMap: Object.fromEntries(emotes.map((emote) => [emote.name, emote.imageUrl])) });
    } catch {
      set({ emoteMap: {} });
    }
  },

  saveTargetChannel: async (channelLogin) => {
    const connection = await setTwitchTargetChannel(channelLogin);
    set({ connection });
  },

  savePolicy: async (policy) => {
    const saved = await setTwitchPolicy(policy);
    set({ policy: saved });
  },

  connectAccount: async () => {
    set({ isConnecting: true });
    try {
      const connection = await connectTwitchAccount();
      set({ connection, isConnecting: false });
    } catch (err) {
      set({ isConnecting: false });
      throw err;
    }
  },

  disconnectAccount: async () => {
    await disconnectTwitchAccount();
    const connection = await getTwitchConnection();
    set({ connection, status: "stopped", feed: [] });
  },

  start: async () => {
    await startTwitch();
    set({ status: "running" });
  },

  stop: async () => {
    await stopTwitch();
    set({ status: "stopped" });
  },

  sendTestMessage: async (text) => {
    set({ isSendingTestMessage: true });
    try {
      await sendTwitchTestMessage(text);
    } finally {
      set({ isSendingTestMessage: false });
    }
  },

  handleAppEvent: (event) => {
    switch (event.type) {
      case "service_status_changed":
        if (event.service === "twitch") set({ status: event.status });
        return;
      case "twitch_message_received": {
        const entry: TwitchFeedMessage = {
          id: crypto.randomUUID(),
          from: "chat",
          displayName: event.chatterDisplayName,
          text: event.text,
          at: new Date().toISOString(),
        };
        set((state) => ({ feed: [...state.feed, entry].slice(-MAX_FEED_MESSAGES) }));
        return;
      }
      case "twitch_message_sent": {
        const entry: TwitchFeedMessage = { id: crypto.randomUUID(), from: "character", displayName: event.characterName, text: event.text, at: new Date().toISOString() };
        set((state) => ({ feed: [...state.feed, entry].slice(-MAX_FEED_MESSAGES) }));
        return;
      }
      default:
        return;
    }
  },
}));
