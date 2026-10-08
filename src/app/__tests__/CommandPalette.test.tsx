import "@/lib/i18n";

import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";

import i18n from "@/lib/i18n";
import { useNavigationStore } from "@/stores/navigationStore";

import { CommandPalette } from "../CommandPalette";

function pressCtrlK() {
  act(() => {
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "k", ctrlKey: true, bubbles: true }));
  });
}

describe("CommandPalette", () => {
  beforeEach(() => {
    useNavigationStore.setState({ activeTab: "chat", isSettingsOpen: false, pendingSettingsSection: null });
  });

  it("opens on Ctrl+K", () => {
    render(<CommandPalette />);
    expect(screen.queryByPlaceholderText(i18n.t("commandPalette.placeholder"))).not.toBeInTheDocument();

    pressCtrlK();

    expect(screen.getByPlaceholderText(i18n.t("commandPalette.placeholder"))).toBeInTheDocument();
  });

  it("closes again on a second Ctrl+K", () => {
    render(<CommandPalette />);
    pressCtrlK();
    expect(screen.getByPlaceholderText(i18n.t("commandPalette.placeholder"))).toBeInTheDocument();

    pressCtrlK();

    expect(screen.queryByPlaceholderText(i18n.t("commandPalette.placeholder"))).not.toBeInTheDocument();
  });

  it("filters the list as the user types", async () => {
    const user = userEvent.setup();
    render(<CommandPalette />);
    pressCtrlK();

    await user.type(screen.getByPlaceholderText(i18n.t("commandPalette.placeholder")), i18n.t("sidebar.voice"));

    expect(screen.getByText(i18n.t("sidebar.voice"))).toBeInTheDocument();
    expect(screen.queryByText(i18n.t("sidebar.chat"))).not.toBeInTheDocument();
  });

  it("shows a no-results row when nothing matches", async () => {
    const user = userEvent.setup();
    render(<CommandPalette />);
    pressCtrlK();

    await user.type(screen.getByPlaceholderText(i18n.t("commandPalette.placeholder")), "zzzzzz");

    expect(screen.getByText(i18n.t("commandPalette.noResults"))).toBeInTheDocument();
  });

  it("selecting a sidebar tab switches activeTab and closes the palette", async () => {
    const user = userEvent.setup();
    render(<CommandPalette />);
    pressCtrlK();

    await user.click(screen.getByText(i18n.t("sidebar.voice")));

    expect(useNavigationStore.getState().activeTab).toBe("voice");
    expect(screen.queryByPlaceholderText(i18n.t("commandPalette.placeholder"))).not.toBeInTheDocument();
  });

  it("selecting a Configuración section opens Settings on that section", async () => {
    const user = userEvent.setup();
    render(<CommandPalette />);
    pressCtrlK();

    await user.type(screen.getByPlaceholderText(i18n.t("commandPalette.placeholder")), i18n.t("settings.tabs.privacy"));
    await user.click(screen.getByText(`${i18n.t("sidebar.settings")} — ${i18n.t("settings.tabs.privacy")}`));

    expect(useNavigationStore.getState().isSettingsOpen).toBe(true);
    expect(useNavigationStore.getState().pendingSettingsSection).toBe("privacy");
  });

  it("Enter selects the highlighted command", async () => {
    const user = userEvent.setup();
    render(<CommandPalette />);
    pressCtrlK();

    await user.type(screen.getByPlaceholderText(i18n.t("commandPalette.placeholder")), i18n.t("sidebar.discord"));
    await user.keyboard("{Enter}");

    expect(useNavigationStore.getState().activeTab).toBe("discord");
  });

  it("closes on Escape", async () => {
    const user = userEvent.setup();
    render(<CommandPalette />);
    pressCtrlK();
    expect(screen.getByPlaceholderText(i18n.t("commandPalette.placeholder"))).toBeInTheDocument();

    await user.keyboard("{Escape}");

    expect(screen.queryByPlaceholderText(i18n.t("commandPalette.placeholder"))).not.toBeInTheDocument();
  });
});
