import { create } from "zustand";

import {
  connectDiscordBot,
  disconnectDiscordAccount,
  getDiscordConnection,
  getDiscordPolicy,
  getDiscordStatus,
  sendDiscordTestMessage,
  setDiscordPolicy,
  startDiscord,
  stopDiscord,
  type DiscordConnectionSettings,
  type DiscordPolicy,
} from "@/lib/ipc/discord";
import type { AppEvent, ServiceStatus } from "@/lib/ipc/events";

/** Bounded so a very active server doesn't grow this without limit — only
 * the live feed shown in the Discord tab reads this, nothing persists it. */
const MAX_FEED_MESSAGES = 200;

export interface DiscordFeedMessage {
  id: string;
  from: "chat" | "character";
  displayName: string;
  text: string;
  at: string;
}

interface DiscordState {
  connection: DiscordConnectionSettings | null;
  policy: DiscordPolicy | null;
  status: ServiceStatus | null;
  feed: DiscordFeedMessage[];
  isLoading: boolean;
  isConnecting: boolean;
  isSendingTestMessage: boolean;

  load: () => Promise<void>;
  connectBot: (token: string) => Promise<void>;
  savePolicy: (policy: DiscordPolicy) => Promise<void>;
  disconnectAccount: () => Promise<void>;
  start: () => Promise<void>;
  stop: () => Promise<void>;
  sendTestMessage: (text: string) => Promise<void>;
  handleAppEvent: (event: AppEvent) => void;
}

export const useDiscordStore = create<DiscordState>((set) => ({
  connection: null,
  policy: null,
  status: null,
  feed: [],
  isLoading: false,
  isConnecting: false,
  isSendingTestMessage: false,

  load: async () => {
    set({ isLoading: true });
    try {
      const [connection, policy, status] = await Promise.all([
        getDiscordConnection(),
        getDiscordPolicy(),
        getDiscordStatus(),
      ]);
      set({ connection, policy, status, isLoading: false });
    } catch {
      set({ isLoading: false });
    }
  },

  connectBot: async (token) => {
    set({ isConnecting: true });
    try {
      const connection = await connectDiscordBot(token);
      set({ connection, isConnecting: false });
    } catch (err) {
      set({ isConnecting: false });
      throw err;
    }
  },

  savePolicy: async (policy) => {
    const saved = await setDiscordPolicy(policy);
    set({ policy: saved });
  },

  disconnectAccount: async () => {
    await disconnectDiscordAccount();
    const connection = await getDiscordConnection();
    set({ connection, status: "stopped", feed: [] });
  },

  start: async () => {
    await startDiscord();
    set({ status: "running" });
  },

  stop: async () => {
    await stopDiscord();
    set({ status: "stopped" });
  },

  sendTestMessage: async (text) => {
    set({ isSendingTestMessage: true });
    try {
      await sendDiscordTestMessage(text);
    } finally {
      set({ isSendingTestMessage: false });
    }
  },

  handleAppEvent: (event) => {
    switch (event.type) {
      case "service_status_changed":
        if (event.service === "discord") set({ status: event.status });
        return;
      case "discord_message_received": {
        const entry: DiscordFeedMessage = {
          id: crypto.randomUUID(),
          from: "chat",
          displayName: event.authorDisplayName,
          text: event.text,
          at: new Date().toISOString(),
        };
        set((state) => ({ feed: [...state.feed, entry].slice(-MAX_FEED_MESSAGES) }));
        return;
      }
      case "discord_message_sent": {
        const entry: DiscordFeedMessage = { id: crypto.randomUUID(), from: "character", displayName: event.characterName, text: event.text, at: new Date().toISOString() };
        set((state) => ({ feed: [...state.feed, entry].slice(-MAX_FEED_MESSAGES) }));
        return;
      }
      default:
        return;
    }
  },
}));
