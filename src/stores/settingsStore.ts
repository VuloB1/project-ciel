import { create } from "zustand";

import i18n from "@/lib/i18n";
import {
  getAppearanceSettings,
  getBackgroundSettings,
  getLanguage,
  getOnboardingCompleted,
  setAppearanceSettings as setAppearanceSettingsIpc,
  setBackgroundSettings as setBackgroundSettingsIpc,
  setLanguage as setLanguageIpc,
  setOnboardingCompleted as setOnboardingCompletedIpc,
  type AppearanceSettings,
  type BackgroundSettings,
} from "@/lib/ipc/settings";

interface SettingsState {
  language: string;
  /** `null` hasta que `load()` resuelve — igual que `voiceStore`'s `muted`,
   * así `App.tsx`'s handler de `proactive_message_sent` sabe distinguir
   * "todavía no cargó" de "el usuario lo desactivó" antes de decidir si
   * notifica. */
  background: BackgroundSettings | null;
  /** `null` hasta que `load()` resuelve — `useActiveProfileTheme`/
   * `useAppearanceSettings` esperan a que esto exista antes de escribir las
   * variables CSS, para no pisarlas con un default de "modo oscuro, 100%"
   * que después salte al valor real guardado. */
  appearance: AppearanceSettings | null;
  /** `null` hasta que `load()` resuelve — `App.tsx` espera a que esto deje
   * de ser `null` antes de decidir si muestra el `WelcomeScreen` o el
   * layout normal, para no parpadear entre ambos en cada arranque. */
  onboardingCompleted: boolean | null;
  /** Bumped after a successful avatar upload — the only signal
   * `useUserAvatar` needs to know it's time to re-fetch (the actual bytes
   * always come fresh from the backend, same pattern as `useProfileAvatar`
   * keying off a profile's `avatarPath`). */
  userAvatarVersion: number;
  isLoading: boolean;
  error: string | null;
  load: () => Promise<void>;
  setLanguage: (language: string) => Promise<void>;
  setBackground: (next: BackgroundSettings) => Promise<void>;
  setAppearance: (next: AppearanceSettings) => Promise<void>;
  completeOnboarding: () => Promise<void>;
  bumpUserAvatarVersion: () => void;
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  language: "es",
  background: null,
  appearance: null,
  onboardingCompleted: null,
  userAvatarVersion: 0,
  isLoading: true,
  error: null,

  load: async () => {
    set({ isLoading: true, error: null });
    try {
      const [language, background, appearance, onboardingCompleted] = await Promise.all([
        getLanguage(),
        getBackgroundSettings(),
        getAppearanceSettings(),
        getOnboardingCompleted(),
      ]);
      await i18n.changeLanguage(language);
      set({ language, background, appearance, onboardingCompleted, isLoading: false });
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err), isLoading: false });
    }
  },

  setLanguage: async (language: string) => {
    const previous = get().language;
    set({ language });
    await i18n.changeLanguage(language);
    try {
      await setLanguageIpc(language);
    } catch (err) {
      // Roll back the optimistic update if the write failed.
      set({ language: previous });
      await i18n.changeLanguage(previous);
      throw err;
    }
  },

  setBackground: async (next: BackgroundSettings) => {
    const previous = get().background;
    set({ background: next });
    try {
      const saved = await setBackgroundSettingsIpc(next);
      set({ background: saved });
    } catch (err) {
      set({ background: previous });
      throw err;
    }
  },

  setAppearance: async (next: AppearanceSettings) => {
    const previous = get().appearance;
    set({ appearance: next });
    try {
      const saved = await setAppearanceSettingsIpc(next);
      set({ appearance: saved });
    } catch (err) {
      set({ appearance: previous });
      throw err;
    }
  },

  completeOnboarding: async () => {
    set({ onboardingCompleted: true });
    // Sin rollback en error: re-mostrar el wizard en medio de la misma
    // sesión sería más confuso que útil — si de verdad no se guardó,
    // `load()` lo va a notar en el próximo arranque.
    await setOnboardingCompletedIpc(true);
  },

  bumpUserAvatarVersion: () => set((state) => ({ userAvatarVersion: state.userAvatarVersion + 1 })),
}));
