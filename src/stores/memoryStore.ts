import { create } from "zustand";

import {
  createMemory,
  deleteMemory,
  listMemories,
  searchMemories,
  setMemoryPinned,
  setMemoryScope,
  updateMemory,
  type MemoryUpdateInput,
  type NewMemoryInput,
} from "@/lib/ipc/memory";
import type { AppEvent } from "@/lib/ipc/events";
import type { Memory, MemoryScope } from "@/types/memory";

interface MemoryState {
  memoriesByProfile: Record<string, Memory[]>;
  isLoading: boolean;
  error: string | null;

  load: (profileId: string) => Promise<void>;
  search: (profileId: string, query: string) => Promise<void>;
  create: (input: NewMemoryInput) => Promise<void>;
  update: (id: string, profileId: string, input: MemoryUpdateInput) => Promise<void>;
  setPinned: (id: string, profileId: string, pinned: boolean) => Promise<void>;
  setScope: (id: string, profileId: string, scope: MemoryScope) => Promise<void>;
  remove: (id: string, profileId: string) => Promise<void>;
  handleAppEvent: (event: AppEvent) => void;
}

export const useMemoryStore = create<MemoryState>((set, get) => ({
  memoriesByProfile: {},
  isLoading: false,
  error: null,

  load: async (profileId) => {
    set({ isLoading: true, error: null });
    try {
      const memories = await listMemories(profileId);
      set((state) => ({ memoriesByProfile: { ...state.memoriesByProfile, [profileId]: memories }, isLoading: false }));
    } catch (err) {
      set({ isLoading: false, error: err instanceof Error ? err.message : String(err) });
    }
  },

  search: async (profileId, query) => {
    set({ isLoading: true, error: null });
    try {
      // An empty query means "show everything" rather than "search for
      // nothing" — matches how the FTS backend treats a blank query.
      const memories = query.trim() ? await searchMemories(profileId, query) : await listMemories(profileId);
      set((state) => ({ memoriesByProfile: { ...state.memoriesByProfile, [profileId]: memories }, isLoading: false }));
    } catch (err) {
      set({ isLoading: false, error: err instanceof Error ? err.message : String(err) });
    }
  },

  create: async (input) => {
    const created = await createMemory(input);
    set((state) => ({
      memoriesByProfile: {
        ...state.memoriesByProfile,
        [input.profileId]: [created, ...(state.memoriesByProfile[input.profileId] ?? [])],
      },
    }));
  },

  update: async (id, profileId, input) => {
    const updated = await updateMemory(id, input);
    set((state) => ({
      memoriesByProfile: {
        ...state.memoriesByProfile,
        [profileId]: (state.memoriesByProfile[profileId] ?? []).map((m) => (m.id === id ? updated : m)),
      },
    }));
  },

  setPinned: async (id, profileId, pinned) => {
    await setMemoryPinned(id, pinned);
    set((state) => ({
      memoriesByProfile: {
        ...state.memoriesByProfile,
        [profileId]: (state.memoriesByProfile[profileId] ?? []).map((m) => (m.id === id ? { ...m, isPinned: pinned } : m)),
      },
    }));
  },

  setScope: async (id, profileId, scope) => {
    await setMemoryScope(id, scope);
    set((state) => ({
      memoriesByProfile: {
        ...state.memoriesByProfile,
        [profileId]: (state.memoriesByProfile[profileId] ?? []).map((m) => (m.id === id ? { ...m, scope } : m)),
      },
    }));
  },

  remove: async (id, profileId) => {
    await deleteMemory(id);
    set((state) => ({
      memoriesByProfile: {
        ...state.memoriesByProfile,
        [profileId]: (state.memoriesByProfile[profileId] ?? []).filter((m) => m.id !== id),
      },
    }));
  },

  handleAppEvent: (event) => {
    if (event.type !== "memories_updated") return;
    // Only refetch if the Memoria tab has actually been visited for this
    // profile before (i.e. there's already an entry) — no point eagerly
    // loading data for a profile nobody's looking at.
    if (get().memoriesByProfile[event.profileId] !== undefined) {
      void get().load(event.profileId);
    }
  },
}));
