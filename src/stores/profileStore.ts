import { create } from "zustand";
import { persist } from "zustand/middleware";

import {
  createProfile,
  deleteProfile,
  duplicateProfile,
  listProfiles,
  updateProfile,
} from "@/lib/ipc/profiles";
import type { Profile, ProfileUpdateInput } from "@/types/profile";

interface ProfileState {
  profiles: Profile[];
  activeProfileId: string | null;
  isLoading: boolean;
  error: string | null;

  load: () => Promise<void>;
  setActiveProfileId: (id: string) => void;
  create: (name: string) => Promise<Profile>;
  update: (id: string, input: ProfileUpdateInput) => Promise<Profile>;
  duplicate: (id: string, newName: string) => Promise<Profile>;
  remove: (id: string) => Promise<void>;
}

/**
 * `activeProfileId` is the single global "who am I talking to" selection
 * (sección 105 del brief) — shared by every channel once Chat/Call/Twitch/
 * Discord exist. Only that one field is persisted (via `localStorage`,
 * which survives across launches in the Tauri WebView); the profile list
 * itself is always re-fetched from the backend on load.
 */
export const useProfileStore = create<ProfileState>()(
  persist(
    (set, get) => ({
      profiles: [],
      activeProfileId: null,
      isLoading: true,
      error: null,

      load: async () => {
        set({ isLoading: true, error: null });
        try {
          const profiles = await listProfiles();
          const currentActive = get().activeProfileId;
          const stillExists = currentActive && profiles.some((p) => p.id === currentActive);
          set({
            profiles,
            activeProfileId: stillExists ? currentActive : (profiles[0]?.id ?? null),
            isLoading: false,
          });
        } catch (err) {
          set({ error: err instanceof Error ? err.message : String(err), isLoading: false });
        }
      },

      setActiveProfileId: (id) => set({ activeProfileId: id }),

      create: async (name: string) => {
        const profile = await createProfile(name);
        set((state) => ({ profiles: [profile, ...state.profiles], activeProfileId: profile.id }));
        return profile;
      },

      update: async (id: string, input: ProfileUpdateInput) => {
        const updated = await updateProfile(id, input);
        set((state) => ({ profiles: state.profiles.map((p) => (p.id === id ? updated : p)) }));
        return updated;
      },

      duplicate: async (id: string, newName: string) => {
        const copy = await duplicateProfile(id, newName);
        set((state) => ({ profiles: [copy, ...state.profiles], activeProfileId: copy.id }));
        return copy;
      },

      remove: async (id: string) => {
        await deleteProfile(id);
        set((state) => {
          const profiles = state.profiles.filter((p) => p.id !== id);
          const activeProfileId = state.activeProfileId === id ? (profiles[0]?.id ?? null) : state.activeProfileId;
          return { profiles, activeProfileId };
        });
      },
    }),
    {
      name: "aivt-active-profile",
      partialize: (state) => ({ activeProfileId: state.activeProfileId }),
    },
  ),
);
