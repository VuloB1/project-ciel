import { create } from "zustand";

import { getMood, resetMood, type ProfileMood } from "@/lib/ipc/mood";
import type { AppEvent } from "@/lib/ipc/events";

interface MoodState {
  moodByProfile: Record<string, ProfileMood>;

  load: (profileId: string) => Promise<void>;
  reset: (profileId: string) => Promise<void>;
  handleAppEvent: (event: AppEvent) => void;
}

export const useMoodStore = create<MoodState>((set) => ({
  moodByProfile: {},

  load: async (profileId) => {
    const mood = await getMood(profileId);
    set((state) => ({ moodByProfile: { ...state.moodByProfile, [profileId]: mood } }));
  },

  reset: async (profileId) => {
    const mood = await resetMood(profileId);
    set((state) => ({ moodByProfile: { ...state.moodByProfile, [profileId]: mood } }));
  },

  handleAppEvent: (event) => {
    if (event.type !== "mood_updated") return;
    set((state) => ({
      moodByProfile: {
        ...state.moodByProfile,
        [event.profileId]: { profileId: event.profileId, valence: event.valence, energy: event.energy, updatedAt: new Date().toISOString() },
      },
    }));
  },
}));
