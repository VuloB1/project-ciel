import { create } from "zustand";

import { clearActivity, listRecentActivity, type ActivityEvent } from "@/lib/ipc/activity";

interface ActivityState {
  events: ActivityEvent[];
  isLoading: boolean;
  error: string | null;
  load: () => Promise<void>;
  clear: () => Promise<void>;
}

export const useActivityStore = create<ActivityState>((set, get) => ({
  events: [],
  isLoading: false,
  error: null,

  load: async () => {
    // Only show the spinner on the very first load — the page polls this
    // in the background afterward, and flashing a spinner on every tick
    // would be worse than just leaving the previous list up while it
    // refreshes.
    set({ isLoading: get().events.length === 0, error: null });
    try {
      const events = await listRecentActivity();
      set({ events, isLoading: false });
    } catch (err) {
      set({ isLoading: false, error: err instanceof Error ? err.message : String(err) });
    }
  },

  // No try/catch here — a failure surfaces to whoever called `clear()`
  // (the confirm dialog) to report, same as `IdentityEditor`'s delete.
  clear: async () => {
    await clearActivity();
    set({ events: [] });
  },
}));
