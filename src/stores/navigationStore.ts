import { create } from "zustand";
import { persist } from "zustand/middleware";

import type { SettingsSection } from "@/features/settings/SettingsDialog";

export const SIDEBAR_TABS = ["chat", "identity", "memory", "voice", "call", "avatar", "twitch", "discord"] as const;

export type SidebarTab = (typeof SIDEBAR_TABS)[number];

interface NavigationState {
  activeTab: SidebarTab;
  setActiveTab: (tab: SidebarTab) => void;
  /** Preferencia estable del usuario — persistida (igual que
   * `profileStore`'s `activeProfileId`), sobrevive a un reinicio. */
  sidebarCollapsed: boolean;
  toggleSidebarCollapsed: () => void;
  /** A diferencia de `sidebarCollapsed`, nunca se persiste — abrir la app
   * y encontrarse sin sidebar sin saber por qué sería confuso. Siempre
   * arranca en `false`. */
  immersiveMode: boolean;
  toggleImmersiveMode: () => void;
  /** Configuración ya no es una pestaña propia — vive en un diálogo
   * flotante (`SettingsDialog.tsx`, mismo espíritu que `CallSettingsDialog`)
   * montado una sola vez en `App.tsx`, así que su visibilidad se controla
   * acá en vez de con `activeTab`. */
  isSettingsOpen: boolean;
  openSettings: () => void;
  closeSettings: () => void;
  /** Set by the command palette (`CommandPalette.tsx`) when the user picks
   * a Configuración sub-section directly — `SettingsDialog` (whose own
   * `activeSection` is otherwise purely local state, unreachable from
   * outside) watches this and jumps to it once, then clears it back to
   * `null` so a later, ordinary visit to Configuración doesn't stay stuck
   * on that section. */
  pendingSettingsSection: SettingsSection | null;
  openSettingsSection: (section: SettingsSection) => void;
  clearPendingSettingsSection: () => void;
}

export const useNavigationStore = create<NavigationState>()(
  persist(
    (set) => ({
      activeTab: "chat",
      setActiveTab: (tab) => set({ activeTab: tab }),
      sidebarCollapsed: false,
      toggleSidebarCollapsed: () => set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed })),
      immersiveMode: false,
      toggleImmersiveMode: () => set((state) => ({ immersiveMode: !state.immersiveMode })),
      isSettingsOpen: false,
      openSettings: () => set({ isSettingsOpen: true }),
      closeSettings: () => set({ isSettingsOpen: false }),
      pendingSettingsSection: null,
      openSettingsSection: (section) => set({ isSettingsOpen: true, pendingSettingsSection: section }),
      clearPendingSettingsSection: () => set({ pendingSettingsSection: null }),
    }),
    {
      name: "aivt-navigation",
      partialize: (state) => ({ sidebarCollapsed: state.sidebarCollapsed }),
    },
  ),
);
