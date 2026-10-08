import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/ipc/proactive", () => ({
  getProactiveSettings: vi.fn(),
  setProactiveEnabled: vi.fn(),
}));

import * as proactiveIpc from "@/lib/ipc/proactive";
import type { ProactiveSettings } from "@/lib/ipc/proactive";
import { useProactiveStore } from "../proactiveStore";

const PROFILE_ID = "p1";

function makeSettings(overrides: Partial<ProactiveSettings> = {}): ProactiveSettings {
  return { profileId: PROFILE_ID, enabled: false, lastTriggeredAt: null, ...overrides };
}

describe("useProactiveStore", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useProactiveStore.setState({ settingsByProfile: {}, unreadConversationByProfile: {} });
  });

  it("load() populates the settings for the profile", async () => {
    const settings = makeSettings({ enabled: true });
    vi.mocked(proactiveIpc.getProactiveSettings).mockResolvedValue(settings);

    await useProactiveStore.getState().load(PROFILE_ID);

    expect(useProactiveStore.getState().settingsByProfile[PROFILE_ID]).toEqual(settings);
  });

  it("setEnabled() replaces the stored settings with the server's response", async () => {
    useProactiveStore.setState({ settingsByProfile: { [PROFILE_ID]: makeSettings() } });
    vi.mocked(proactiveIpc.setProactiveEnabled).mockResolvedValue(makeSettings({ enabled: true }));

    await useProactiveStore.getState().setEnabled(PROFILE_ID, true);

    expect(useProactiveStore.getState().settingsByProfile[PROFILE_ID]?.enabled).toBe(true);
  });

  it("handleAppEvent marks the profile's conversation as unread", () => {
    useProactiveStore
      .getState()
      .handleAppEvent({ type: "proactive_message_sent", profileId: PROFILE_ID, conversationId: "c1", content: "hola" });

    expect(useProactiveStore.getState().unreadConversationByProfile[PROFILE_ID]).toBe("c1");
  });

  it("handleAppEvent ignores unrelated events", () => {
    expect(() => useProactiveStore.getState().handleAppEvent({ type: "heartbeat", timestamp: "now" })).not.toThrow();
    expect(useProactiveStore.getState().unreadConversationByProfile[PROFILE_ID]).toBeUndefined();
  });

  it("clearUnread removes the flag when it matches the given conversation", () => {
    useProactiveStore.setState({ unreadConversationByProfile: { [PROFILE_ID]: "c1" } });

    useProactiveStore.getState().clearUnread(PROFILE_ID, "c1");

    expect(useProactiveStore.getState().unreadConversationByProfile[PROFILE_ID]).toBeUndefined();
  });

  it("clearUnread does nothing when the conversation id doesn't match", () => {
    useProactiveStore.setState({ unreadConversationByProfile: { [PROFILE_ID]: "c1" } });

    useProactiveStore.getState().clearUnread(PROFILE_ID, "c2");

    expect(useProactiveStore.getState().unreadConversationByProfile[PROFILE_ID]).toBe("c1");
  });
});
