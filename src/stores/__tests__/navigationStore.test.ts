import { beforeEach, describe, expect, it } from "vitest";

import { useNavigationStore } from "../navigationStore";

describe("useNavigationStore", () => {
  beforeEach(() => {
    localStorage.clear();
    useNavigationStore.setState({ activeTab: "chat", sidebarCollapsed: false, immersiveMode: false });
  });

  it("defaults to the chat tab", () => {
    expect(useNavigationStore.getState().activeTab).toBe("chat");
  });

  it("updates the active tab", () => {
    useNavigationStore.getState().setActiveTab("identity");
    expect(useNavigationStore.getState().activeTab).toBe("identity");
  });

  it("openSettings opens the dialog, closeSettings closes it", () => {
    expect(useNavigationStore.getState().isSettingsOpen).toBe(false);

    useNavigationStore.getState().openSettings();
    expect(useNavigationStore.getState().isSettingsOpen).toBe(true);

    useNavigationStore.getState().closeSettings();
    expect(useNavigationStore.getState().isSettingsOpen).toBe(false);
  });

  it("openSettingsSection opens the dialog on a specific section", () => {
    useNavigationStore.getState().openSettingsSection("privacy");
    expect(useNavigationStore.getState().isSettingsOpen).toBe(true);
    expect(useNavigationStore.getState().pendingSettingsSection).toBe("privacy");
  });

  it("toggleSidebarCollapsed flips the flag", () => {
    useNavigationStore.getState().toggleSidebarCollapsed();
    expect(useNavigationStore.getState().sidebarCollapsed).toBe(true);

    useNavigationStore.getState().toggleSidebarCollapsed();
    expect(useNavigationStore.getState().sidebarCollapsed).toBe(false);
  });

  it("toggleImmersiveMode flips the flag", () => {
    useNavigationStore.getState().toggleImmersiveMode();
    expect(useNavigationStore.getState().immersiveMode).toBe(true);
  });

  it("only persists sidebarCollapsed, not activeTab or immersiveMode", () => {
    useNavigationStore.getState().setActiveTab("identity");
    useNavigationStore.getState().toggleSidebarCollapsed();
    useNavigationStore.getState().toggleImmersiveMode();

    const raw = localStorage.getItem("aivt-navigation");
    expect(raw).not.toBeNull();
    const persisted = JSON.parse(raw ?? "{}") as { state: Record<string, unknown> };
    expect(persisted.state).toEqual({ sidebarCollapsed: true });
  });
});
