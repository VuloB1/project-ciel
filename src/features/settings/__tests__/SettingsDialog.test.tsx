import "@/lib/i18n";

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";

import { AppContextMenu } from "@/app/AppContextMenu";
import i18n from "@/lib/i18n";
import { useNavigationStore } from "@/stores/navigationStore";
import { useProfileStore } from "@/stores/profileStore";
import { DEFAULT_IDENTITY, DEFAULT_PERSONALITY, DEFAULT_VOICE, type Profile } from "@/types/profile";

import { SettingsDialog } from "../SettingsDialog";

function makeProfile(overrides: Partial<Profile> = {}): Profile {
  return {
    id: "p1",
    name: "Ciel",
    nickname: null,
    avatarPath: null,
    description: null,
    ageFictional: null,
    pronouns: null,
    occupation: null,
    species: null,
    accentColor: null,
    theme: null,
    isArchived: false,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    identity: DEFAULT_IDENTITY,
    personality: DEFAULT_PERSONALITY,
    voice: DEFAULT_VOICE,
    avatarIdlePath: null,
    avatarThinkingPath: null,
    avatarTalkingPath: null,
    ...overrides,
  };
}

describe("SettingsDialog", () => {
  beforeEach(() => {
    useNavigationStore.setState({ isSettingsOpen: false, pendingSettingsSection: null });
  });

  it("stays closed until isSettingsOpen is true", () => {
    useProfileStore.setState({ profiles: [makeProfile()], activeProfileId: "p1", isLoading: false, error: null });
    render(<SettingsDialog />);

    expect(screen.queryByText(i18n.t("settings.general.avatar"))).not.toBeInTheDocument();
  });

  it("shows General's content by default and swaps sections on click", async () => {
    useProfileStore.setState({ profiles: [makeProfile()], activeProfileId: "p1", isLoading: false, error: null });
    useNavigationStore.setState({ isSettingsOpen: true });
    const user = userEvent.setup();
    render(<SettingsDialog />);

    expect(screen.getByText(i18n.t("settings.general.avatar"))).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: i18n.t("settings.tabs.privacy") }));
    expect(await screen.findByText(i18n.t("settings.privacy.title"))).toBeInTheDocument();
    expect(screen.queryByText(i18n.t("settings.general.avatar"))).not.toBeInTheDocument();
  });

  it("jumps to a pending section from the command palette and clears it", async () => {
    useProfileStore.setState({ profiles: [makeProfile()], activeProfileId: "p1", isLoading: false, error: null });
    useNavigationStore.setState({ isSettingsOpen: true, pendingSettingsSection: "privacy" });
    render(<SettingsDialog />);

    expect(await screen.findByText(i18n.t("settings.privacy.title"))).toBeInTheDocument();
    expect(useNavigationStore.getState().pendingSettingsSection).toBeNull();
  });

  it("the global right-click menu still reaches content inside this dialog (Radix Portal, not the App tree)", async () => {
    useProfileStore.setState({ profiles: [makeProfile()], activeProfileId: "p1", isLoading: false, error: null });
    useNavigationStore.setState({ isSettingsOpen: true });
    const user = userEvent.setup();
    render(
      <>
        <SettingsDialog />
        <AppContextMenu />
      </>,
    );

    // `SettingsDialog` renders via `DialogPrimitive.Portal` straight to
    // `document.body` — a React-tree-nested trigger placed in `App.tsx`
    // would never see clicks in here. `AppContextMenu` listens on `window`
    // instead, so it doesn't matter which container the click bubbled up
    // from.
    await user.pointer({ target: screen.getByText(i18n.t("sidebar.settings")), keys: "[MouseRight]" });

    // `getByRole` en vez de `getByText`: la etiqueta del ítem del menú
    // ("Configuración") coincide literalmente con el propio título del
    // diálogo que ya está abierto detrás — el rol de botón es lo único que
    // los distingue.
    const settingsItem = screen.getByRole("button", { name: i18n.t("contextMenu.settings") });
    expect(settingsItem).toBeInTheDocument();

    // Radix's `DismissableLayer` pone `document.body.style.pointerEvents =
    // "none"` mientras el diálogo modal está abierto y solo reactiva "auto"
    // en su propio Content/Overlay — un click acá debe seguir funcionando
    // (el menú debe cerrarse) en vez de quedar bloqueado por esa herencia.
    await user.click(settingsItem);
    expect(screen.queryByRole("button", { name: i18n.t("contextMenu.settings") })).not.toBeInTheDocument();
  });
});
