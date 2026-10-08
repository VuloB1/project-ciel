import { create } from "zustand";

import {
  createPerson,
  deletePerson,
  getPeoplePolicy,
  listPeople,
  setPeoplePolicy,
  type NewPersonInput,
} from "@/lib/ipc/people";
import type { Person } from "@/types/person";

interface PeopleState {
  people: Person[];
  twitchAutoLearnEnabled: boolean;
  discordAutoLearnEnabled: boolean;
  isLoading: boolean;
  error: string | null;

  load: () => Promise<void>;
  create: (input: NewPersonInput) => Promise<void>;
  remove: (id: string) => Promise<void>;
  setTwitchAutoLearn: (enabled: boolean) => Promise<void>;
  setDiscordAutoLearn: (enabled: boolean) => Promise<void>;
}

export const usePeopleStore = create<PeopleState>((set, get) => ({
  people: [],
  twitchAutoLearnEnabled: true,
  discordAutoLearnEnabled: true,
  isLoading: false,
  error: null,

  load: async () => {
    set({ isLoading: true, error: null });
    try {
      const [people, policy] = await Promise.all([listPeople(), getPeoplePolicy()]);
      set({ people, twitchAutoLearnEnabled: policy.twitchAutoLearnEnabled, discordAutoLearnEnabled: policy.discordAutoLearnEnabled, isLoading: false });
    } catch (err) {
      set({ isLoading: false, error: err instanceof Error ? err.message : String(err) });
    }
  },

  create: async (input) => {
    const created = await createPerson(input);
    set((state) => ({ people: [created, ...state.people] }));
  },

  remove: async (id) => {
    await deletePerson(id);
    set((state) => ({ people: state.people.filter((p) => p.id !== id) }));
  },

  setTwitchAutoLearn: async (enabled) => {
    const previous = get().twitchAutoLearnEnabled;
    set({ twitchAutoLearnEnabled: enabled });
    try {
      await setPeoplePolicy({ twitchAutoLearnEnabled: enabled, discordAutoLearnEnabled: get().discordAutoLearnEnabled });
    } catch (err) {
      set({ twitchAutoLearnEnabled: previous });
      throw err;
    }
  },

  setDiscordAutoLearn: async (enabled) => {
    const previous = get().discordAutoLearnEnabled;
    set({ discordAutoLearnEnabled: enabled });
    try {
      await setPeoplePolicy({ twitchAutoLearnEnabled: get().twitchAutoLearnEnabled, discordAutoLearnEnabled: enabled });
    } catch (err) {
      set({ discordAutoLearnEnabled: previous });
      throw err;
    }
  },
}));
