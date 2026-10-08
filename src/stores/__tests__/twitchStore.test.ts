import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/ipc/twitch", () => ({
  getTwitchConnection: vi.fn(),
  getTwitchPolicy: vi.fn(),
  getTwitchStatus: vi.fn(),
  setTwitchPolicy: vi.fn(),
  setTwitchTargetChannel: vi.fn(),
  connectTwitchAccount: vi.fn(),
  disconnectTwitchAccount: vi.fn(),
  startTwitch: vi.fn(),
  stopTwitch: vi.fn(),
  sendTwitchTestMessage: vi.fn(),
}));

import { sendTwitchTestMessage } from "@/lib/ipc/twitch";

import { useTwitchStore } from "../twitchStore";

describe("useTwitchStore", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useTwitchStore.setState({
      connection: null,
      policy: null,
      status: null,
      feed: [],
      isLoading: false,
      isConnecting: false,
      isSendingTestMessage: false,
    });
  });

  it("service_status_changed for twitch updates status", () => {
    useTwitchStore.getState().handleAppEvent({ type: "service_status_changed", service: "twitch", status: "running" });
    expect(useTwitchStore.getState().status).toBe("running");
  });

  it("service_status_changed for a different service is ignored", () => {
    useTwitchStore.getState().handleAppEvent({ type: "service_status_changed", service: "discord", status: "running" });
    expect(useTwitchStore.getState().status).toBeNull();
  });

  it("a received chat message is appended to the feed as 'chat'", () => {
    useTwitchStore.getState().handleAppEvent({ type: "twitch_message_received", chatterDisplayName: "Vulito", text: "hola!" });
    const feed = useTwitchStore.getState().feed;
    expect(feed).toHaveLength(1);
    expect(feed[0]).toMatchObject({ from: "chat", displayName: "Vulito", text: "hola!" });
  });

  it("a sent reply is appended to the feed as 'character', labeled with the active character's name", () => {
    useTwitchStore.getState().handleAppEvent({ type: "twitch_message_sent", characterName: "Ciel", text: "¡hola a todos!" });
    const feed = useTwitchStore.getState().feed;
    expect(feed).toHaveLength(1);
    expect(feed[0]).toMatchObject({ from: "character", displayName: "Ciel", text: "¡hola a todos!" });
  });

  it("the feed keeps chronological order across both message kinds", () => {
    useTwitchStore.getState().handleAppEvent({ type: "twitch_message_received", chatterDisplayName: "Vulito", text: "hola" });
    useTwitchStore.getState().handleAppEvent({ type: "twitch_message_sent", characterName: "Ciel", text: "¡hola!" });
    const feed = useTwitchStore.getState().feed;
    expect(feed.map((m) => m.from)).toEqual(["chat", "character"]);
  });

  it("the feed is capped so a very active chat doesn't grow it without bound", () => {
    for (let i = 0; i < 250; i++) {
      useTwitchStore.getState().handleAppEvent({ type: "twitch_message_received", chatterDisplayName: "Vulito", text: `mensaje-${i}` });
    }
    const feed = useTwitchStore.getState().feed;
    expect(feed.length).toBeLessThanOrEqual(200);
    expect(feed.at(-1)?.text).toBe("mensaje-249");
  });

  it("sendTestMessage calls the IPC function and toggles isSendingTestMessage", async () => {
    vi.mocked(sendTwitchTestMessage).mockResolvedValue(undefined);

    const promise = useTwitchStore.getState().sendTestMessage("hola personaje");
    expect(useTwitchStore.getState().isSendingTestMessage).toBe(true);

    await promise;

    expect(sendTwitchTestMessage).toHaveBeenCalledWith("hola personaje");
    expect(useTwitchStore.getState().isSendingTestMessage).toBe(false);
  });

  it("sendTestMessage clears isSendingTestMessage even when the call fails", async () => {
    vi.mocked(sendTwitchTestMessage).mockRejectedValue(new Error("no active profile"));

    await expect(useTwitchStore.getState().sendTestMessage("hola")).rejects.toThrow();
    expect(useTwitchStore.getState().isSendingTestMessage).toBe(false);
  });
});
