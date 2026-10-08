import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/ipc/discord", () => ({
  getDiscordConnection: vi.fn(),
  getDiscordPolicy: vi.fn(),
  getDiscordStatus: vi.fn(),
  connectDiscordBot: vi.fn(),
  setDiscordPolicy: vi.fn(),
  disconnectDiscordAccount: vi.fn(),
  startDiscord: vi.fn(),
  stopDiscord: vi.fn(),
  sendDiscordTestMessage: vi.fn(),
}));

import { sendDiscordTestMessage } from "@/lib/ipc/discord";

import { useDiscordStore } from "../discordStore";

describe("useDiscordStore", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useDiscordStore.setState({
      connection: null,
      policy: null,
      status: null,
      feed: [],
      isLoading: false,
      isConnecting: false,
      isSendingTestMessage: false,
    });
  });

  it("service_status_changed for discord updates status", () => {
    useDiscordStore.getState().handleAppEvent({ type: "service_status_changed", service: "discord", status: "running" });
    expect(useDiscordStore.getState().status).toBe("running");
  });

  it("service_status_changed for a different service is ignored", () => {
    useDiscordStore.getState().handleAppEvent({ type: "service_status_changed", service: "twitch", status: "running" });
    expect(useDiscordStore.getState().status).toBeNull();
  });

  it("a received message is appended to the feed as 'chat'", () => {
    useDiscordStore.getState().handleAppEvent({ type: "discord_message_received", authorDisplayName: "Vulito", text: "hola!" });
    const feed = useDiscordStore.getState().feed;
    expect(feed).toHaveLength(1);
    expect(feed[0]).toMatchObject({ from: "chat", displayName: "Vulito", text: "hola!" });
  });

  it("a sent reply is appended to the feed as 'character', labeled with the active character's name", () => {
    useDiscordStore.getState().handleAppEvent({ type: "discord_message_sent", characterName: "Ciel", text: "¡hola a todos!" });
    const feed = useDiscordStore.getState().feed;
    expect(feed).toHaveLength(1);
    expect(feed[0]).toMatchObject({ from: "character", displayName: "Ciel", text: "¡hola a todos!" });
  });

  it("the feed keeps chronological order across both message kinds", () => {
    useDiscordStore.getState().handleAppEvent({ type: "discord_message_received", authorDisplayName: "Vulito", text: "hola" });
    useDiscordStore.getState().handleAppEvent({ type: "discord_message_sent", characterName: "Ciel", text: "¡hola!" });
    const feed = useDiscordStore.getState().feed;
    expect(feed.map((m) => m.from)).toEqual(["chat", "character"]);
  });

  it("the feed is capped so a very active server doesn't grow it without bound", () => {
    for (let i = 0; i < 250; i++) {
      useDiscordStore.getState().handleAppEvent({ type: "discord_message_received", authorDisplayName: "Vulito", text: `mensaje-${i}` });
    }
    const feed = useDiscordStore.getState().feed;
    expect(feed.length).toBeLessThanOrEqual(200);
    expect(feed.at(-1)?.text).toBe("mensaje-249");
  });

  it("sendTestMessage calls the IPC function and toggles isSendingTestMessage", async () => {
    vi.mocked(sendDiscordTestMessage).mockResolvedValue(undefined);

    const promise = useDiscordStore.getState().sendTestMessage("hola personaje");
    expect(useDiscordStore.getState().isSendingTestMessage).toBe(true);

    await promise;

    expect(sendDiscordTestMessage).toHaveBeenCalledWith("hola personaje");
    expect(useDiscordStore.getState().isSendingTestMessage).toBe(false);
  });

  it("sendTestMessage clears isSendingTestMessage even when the call fails", async () => {
    vi.mocked(sendDiscordTestMessage).mockRejectedValue(new Error("no active profile"));

    await expect(useDiscordStore.getState().sendTestMessage("hola")).rejects.toThrow();
    expect(useDiscordStore.getState().isSendingTestMessage).toBe(false);
  });
});
