import { create } from "zustand";

import {
  archiveConversation,
  cancelGeneration,
  createConversation,
  deleteConversation,
  getMessages,
  importCallTranscript,
  listConversations,
  renameConversation,
  reorderPinnedConversations,
  sendMessage as sendMessageIpc,
  setConversationPinned,
  type CallTranscriptEntryInput,
  type ChatImageInput,
} from "@/lib/ipc/chat";
import type { AppEvent } from "@/lib/ipc/events";
import type { Conversation, Message } from "@/types/chat";

interface ActiveGeneration {
  conversationId: string;
  messageId: string;
}

interface ChatState {
  conversationsByProfile: Record<string, Conversation[]>;
  activeConversationId: string | null;
  messagesByConversation: Record<string, Message[]>;
  generatingConversations: Record<string, boolean>;
  requestIndex: Record<string, ActiveGeneration>;
  isLoadingConversations: boolean;

  loadConversations: (profileId: string) => Promise<void>;
  setActiveConversationId: (id: string | null) => void;
  loadMessages: (conversationId: string) => Promise<void>;
  createNewConversation: (profileId: string) => Promise<Conversation>;
  renameConversationTitle: (id: string, profileId: string, title: string) => Promise<void>;
  archiveConversationById: (id: string, profileId: string) => Promise<void>;
  removeConversation: (id: string, profileId: string) => Promise<void>;
  togglePin: (id: string, profileId: string) => Promise<void>;
  reorderPinned: (profileId: string, orderedIds: string[]) => Promise<void>;
  importCallToChat: (profileId: string, entries: CallTranscriptEntryInput[]) => Promise<Conversation>;
  send: (profileId: string, conversationId: string, content: string, image?: ChatImageInput) => Promise<void>;
  cancel: (conversationId: string) => Promise<void>;
  handleAppEvent: (event: AppEvent) => void;
}

/** Mirrors `ConversationsRepository::list_for_profile`'s `ORDER BY`
 * (`is_pinned DESC, pin_order ASC, updated_at DESC`) so a local
 * (optimistic) reorder shows the exact same order the backend would
 * return on a fresh `listConversations`. */
function sortConversations(conversations: Conversation[]): Conversation[] {
  return [...conversations].sort((a, b) => {
    if (a.isPinned !== b.isPinned) return a.isPinned ? -1 : 1;
    if (a.isPinned && b.isPinned && a.pinOrder !== b.pinOrder) return a.pinOrder - b.pinOrder;
    return b.updatedAt.localeCompare(a.updatedAt);
  });
}

function omitKey<T>(record: Record<string, T>, key: string): Record<string, T> {
  const next = { ...record };
  delete next[key];
  return next;
}

function updateMessage(
  messages: Message[] | undefined,
  messageId: string,
  updater: (message: Message) => Message,
): Message[] {
  return (messages ?? []).map((message) => (message.id === messageId ? updater(message) : message));
}

export const useChatStore = create<ChatState>((set, get) => ({
  conversationsByProfile: {},
  activeConversationId: null,
  messagesByConversation: {},
  generatingConversations: {},
  requestIndex: {},
  isLoadingConversations: false,

  loadConversations: async (profileId) => {
    set({ isLoadingConversations: true });
    try {
      const conversations = await listConversations(profileId);
      set((state) => ({
        conversationsByProfile: { ...state.conversationsByProfile, [profileId]: conversations },
        isLoadingConversations: false,
      }));
    } catch {
      set({ isLoadingConversations: false });
    }
  },

  setActiveConversationId: (id) => set({ activeConversationId: id }),

  loadMessages: async (conversationId) => {
    const messages = await getMessages(conversationId);
    set((state) => ({ messagesByConversation: { ...state.messagesByConversation, [conversationId]: messages } }));
  },

  createNewConversation: async (profileId) => {
    const conversation = await createConversation(profileId);
    set((state) => ({
      conversationsByProfile: {
        ...state.conversationsByProfile,
        [profileId]: [conversation, ...(state.conversationsByProfile[profileId] ?? [])],
      },
      activeConversationId: conversation.id,
      messagesByConversation: { ...state.messagesByConversation, [conversation.id]: [] },
    }));
    return conversation;
  },

  renameConversationTitle: async (id, profileId, title) => {
    await renameConversation(id, title);
    set((state) => ({
      conversationsByProfile: {
        ...state.conversationsByProfile,
        [profileId]: (state.conversationsByProfile[profileId] ?? []).map((c) => (c.id === id ? { ...c, title } : c)),
      },
    }));
  },

  archiveConversationById: async (id, profileId) => {
    await archiveConversation(id);
    set((state) => ({
      conversationsByProfile: {
        ...state.conversationsByProfile,
        [profileId]: (state.conversationsByProfile[profileId] ?? []).filter((c) => c.id !== id),
      },
      activeConversationId: state.activeConversationId === id ? null : state.activeConversationId,
    }));
  },

  removeConversation: async (id, profileId) => {
    await deleteConversation(id);
    set((state) => ({
      conversationsByProfile: {
        ...state.conversationsByProfile,
        [profileId]: (state.conversationsByProfile[profileId] ?? []).filter((c) => c.id !== id),
      },
      activeConversationId: state.activeConversationId === id ? null : state.activeConversationId,
    }));
  },

  // Optimistic — same reasoning as `callStore.toggleMute`: flips the flag
  // locally right away so the UI (and the pinned/unpinned grouping) reacts
  // instantly, then confirms with the backend. `pinOrder` is left stale
  // here on purpose: `set_pinned` (backend) is the only thing that assigns
  // a *correct* one (lowest among the profile's pinned conversations), and
  // the next `loadConversations` — or simply reopening the list — picks it
  // up; this optimistic patch only needs `isPinned` right for sorting to
  // put it in the right group immediately.
  togglePin: async (id, profileId) => {
    const current = get().conversationsByProfile[profileId] ?? [];
    const target = current.find((c) => c.id === id);
    if (!target) return;
    const nextPinned = !target.isPinned;

    set((state) => ({
      conversationsByProfile: {
        ...state.conversationsByProfile,
        [profileId]: sortConversations(
          (state.conversationsByProfile[profileId] ?? []).map((c) => (c.id === id ? { ...c, isPinned: nextPinned } : c)),
        ),
      },
    }));

    try {
      await setConversationPinned(id, nextPinned);
      // Refresh from the backend so `pinOrder` (assigned server-side) is
      // accurate for the next drag-to-reorder, not just `isPinned`.
      await get().loadConversations(profileId);
    } catch (err) {
      set((state) => ({
        conversationsByProfile: {
          ...state.conversationsByProfile,
          [profileId]: sortConversations(
            (state.conversationsByProfile[profileId] ?? []).map((c) => (c.id === id ? { ...c, isPinned: target.isPinned } : c)),
          ),
        },
      }));
      throw err;
    }
  },

  reorderPinned: async (profileId, orderedIds) => {
    const previous = get().conversationsByProfile[profileId] ?? [];
    const orderIndex = new Map(orderedIds.map((id, index) => [id, index]));

    set((state) => ({
      conversationsByProfile: {
        ...state.conversationsByProfile,
        [profileId]: sortConversations(
          (state.conversationsByProfile[profileId] ?? []).map((c) =>
            orderIndex.has(c.id) ? { ...c, pinOrder: orderIndex.get(c.id)! } : c,
          ),
        ),
      },
    }));

    try {
      await reorderPinnedConversations(orderedIds);
    } catch (err) {
      set((state) => ({ conversationsByProfile: { ...state.conversationsByProfile, [profileId]: previous } }));
      throw err;
    }
  },

  importCallToChat: async (profileId, entries) => {
    const conversation = await importCallTranscript(profileId, entries);
    set((state) => ({
      conversationsByProfile: {
        ...state.conversationsByProfile,
        [profileId]: sortConversations([conversation, ...(state.conversationsByProfile[profileId] ?? [])]),
      },
      activeConversationId: conversation.id,
      messagesByConversation: { ...state.messagesByConversation, [conversation.id]: [] },
    }));
    return conversation;
  },

  send: async (profileId, conversationId, content, image) => {
    const userMessage = await sendMessageIpc(profileId, conversationId, content, image);
    set((state) => ({
      messagesByConversation: {
        ...state.messagesByConversation,
        [conversationId]: [...(state.messagesByConversation[conversationId] ?? []), userMessage],
      },
      generatingConversations: { ...state.generatingConversations, [conversationId]: true },
    }));
  },

  cancel: async (conversationId) => {
    await cancelGeneration(conversationId);
  },

  handleAppEvent: (event) => {
    switch (event.type) {
      case "ai_response_started": {
        const placeholder: Message = {
          id: event.messageId,
          conversationId: event.conversationId,
          role: "assistant",
          content: "",
          status: "streaming",
          providerId: null,
          model: null,
          inputTokens: null,
          outputTokens: null,
          createdAt: new Date().toISOString(),
        };
        set((state) => ({
          requestIndex: {
            ...state.requestIndex,
            [event.requestId]: { conversationId: event.conversationId, messageId: event.messageId },
          },
          generatingConversations: { ...state.generatingConversations, [event.conversationId]: true },
          messagesByConversation: {
            ...state.messagesByConversation,
            [event.conversationId]: [...(state.messagesByConversation[event.conversationId] ?? []), placeholder],
          },
        }));
        return;
      }
      case "ai_response_token": {
        const target = get().requestIndex[event.requestId];
        if (!target) return;
        set((state) => ({
          messagesByConversation: {
            ...state.messagesByConversation,
            [target.conversationId]: updateMessage(state.messagesByConversation[target.conversationId], target.messageId, (m) => ({
              ...m,
              content: m.content + event.token,
            })),
          },
        }));
        return;
      }
      case "ai_response_finished": {
        finishGeneration(event.requestId, "complete");
        return;
      }
      case "ai_response_cancelled": {
        finishGeneration(event.requestId, "cancelled");
        return;
      }
      case "ai_response_error": {
        finishGeneration(event.requestId, "error", event.message);
        return;
      }
      case "chat_audio_saved": {
        set((state) => ({
          messagesByConversation: {
            ...state.messagesByConversation,
            [event.conversationId]: updateMessage(state.messagesByConversation[event.conversationId], event.messageId, (m) => ({
              ...m,
              attachmentId: event.attachmentId,
              attachmentMimeType: event.mimeType,
            })),
          },
        }));
        return;
      }
      default:
        return;
    }
  },
}));

function finishGeneration(requestId: string, status: Message["status"], errorText?: string) {
  const state = useChatStore.getState();
  const target = state.requestIndex[requestId];
  if (!target) return;

  useChatStore.setState({
    requestIndex: omitKey(state.requestIndex, requestId),
    generatingConversations: omitKey(state.generatingConversations, target.conversationId),
    messagesByConversation: {
      ...state.messagesByConversation,
      [target.conversationId]: updateMessage(state.messagesByConversation[target.conversationId], target.messageId, (m) => ({
        ...m,
        status,
        content: errorText ?? m.content,
      })),
    },
  });
}
