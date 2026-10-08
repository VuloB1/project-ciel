import { create } from "zustand";

import { getProactiveSettings, setProactiveEnabled, type ProactiveSettings } from "@/lib/ipc/proactive";
import type { AppEvent } from "@/lib/ipc/events";

interface ProactiveState {
  settingsByProfile: Record<string, ProactiveSettings>;
  /** Perfil -> id de la conversación con un mensaje proactivo que el
   * usuario todavía no vio — alimenta el punto de "no leído" en el
   * Sidebar. Ausencia de entrada = nada sin leer. */
  unreadConversationByProfile: Record<string, string>;

  load: (profileId: string) => Promise<void>;
  setEnabled: (profileId: string, enabled: boolean) => Promise<void>;
  /** Se llama al abrir una conversación — si es la que tenía el mensaje
   * proactivo sin leer, lo limpia. */
  clearUnread: (profileId: string, conversationId: string) => void;
  handleAppEvent: (event: AppEvent) => void;
}

export const useProactiveStore = create<ProactiveState>((set, get) => ({
  settingsByProfile: {},
  unreadConversationByProfile: {},

  load: async (profileId) => {
    const settings = await getProactiveSettings(profileId);
    set((state) => ({ settingsByProfile: { ...state.settingsByProfile, [profileId]: settings } }));
  },

  setEnabled: async (profileId, enabled) => {
    const settings = await setProactiveEnabled(profileId, enabled);
    set((state) => ({ settingsByProfile: { ...state.settingsByProfile, [profileId]: settings } }));
  },

  clearUnread: (profileId, conversationId) => {
    if (get().unreadConversationByProfile[profileId] !== conversationId) return;
    set((state) => {
      const next = { ...state.unreadConversationByProfile };
      delete next[profileId];
      return { unreadConversationByProfile: next };
    });
  },

  handleAppEvent: (event) => {
    if (event.type !== "proactive_message_sent") return;
    set((state) => ({
      unreadConversationByProfile: { ...state.unreadConversationByProfile, [event.profileId]: event.conversationId },
    }));
  },
}));
