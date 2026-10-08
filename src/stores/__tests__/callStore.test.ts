import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/ipc/call", () => ({
  getCallPolicy: vi.fn(),
  getCallStatus: vi.fn(),
  setCallPolicy: vi.fn(),
  startCall: vi.fn(),
  stopCall: vi.fn(),
  setCallMuted: vi.fn(),
}));

import { setCallMuted } from "@/lib/ipc/call";

import { useCallStore } from "../callStore";

describe("useCallStore", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useCallStore.setState({
      status: null,
      callState: null,
      policy: null,
      feed: [],
      isLoading: false,
      isToggling: false,
      isMuted: false,
      micLevel: 0,
    });
  });

  it("service_status_changed for call updates status", () => {
    useCallStore.getState().handleAppEvent({ type: "service_status_changed", service: "call", status: "running" });
    expect(useCallStore.getState().status).toBe("running");
  });

  it("service_status_changed for a different service is ignored", () => {
    useCallStore.getState().handleAppEvent({ type: "service_status_changed", service: "twitch", status: "running" });
    expect(useCallStore.getState().status).toBeNull();
  });

  it("call_mic_level updates the level and keeps it between 0 and 1", () => {
    useCallStore.getState().handleAppEvent({ type: "call_mic_level", level: 0.4 });
    expect(useCallStore.getState().micLevel).toBe(0.4);
    useCallStore.getState().handleAppEvent({ type: "call_mic_level", level: 3 });
    expect(useCallStore.getState().micLevel).toBe(1);
    useCallStore.getState().handleAppEvent({ type: "call_mic_level", level: -1 });
    expect(useCallStore.getState().micLevel).toBe(0);
  });

  it("call_state_changed updates callState", () => {
    useCallStore.getState().handleAppEvent({ type: "call_state_changed", state: "listening" });
    expect(useCallStore.getState().callState).toBe("listening");
  });

  it("a user utterance is appended to the feed", () => {
    useCallStore.getState().handleAppEvent({ type: "call_user_utterance", text: "hola" });
    const feed = useCallStore.getState().feed;
    expect(feed).toHaveLength(1);
    expect(feed[0]).toMatchObject({ from: "user", text: "hola" });
  });

  it("a character reply is appended to the feed", () => {
    useCallStore.getState().handleAppEvent({ type: "call_character_reply", text: "¡hola!" });
    const feed = useCallStore.getState().feed;
    expect(feed).toHaveLength(1);
    expect(feed[0]).toMatchObject({ from: "character", text: "¡hola!" });
  });

  it("a call error is appended to the feed so failures are never silent", () => {
    useCallStore.getState().handleAppEvent({ type: "call_error", message: "el servidor de transcripción respondió con un error: 404" });
    const feed = useCallStore.getState().feed;
    expect(feed).toHaveLength(1);
    expect(feed[0]).toMatchObject({ from: "error", text: "el servidor de transcripción respondió con un error: 404" });
  });

  it("the feed keeps chronological order across both message kinds", () => {
    useCallStore.getState().handleAppEvent({ type: "call_user_utterance", text: "hola" });
    useCallStore.getState().handleAppEvent({ type: "call_character_reply", text: "¡hola!" });
    const feed = useCallStore.getState().feed;
    expect(feed.map((m) => m.from)).toEqual(["user", "character"]);
  });

  it("the feed is capped so a very long call doesn't grow it without bound", () => {
    for (let i = 0; i < 250; i++) {
      useCallStore.getState().handleAppEvent({ type: "call_user_utterance", text: `mensaje-${i}` });
    }
    const feed = useCallStore.getState().feed;
    expect(feed.length).toBeLessThanOrEqual(200);
    expect(feed.at(-1)?.text).toBe("mensaje-249");
  });

  it("toggleMute flips isMuted optimistically and calls the backend", async () => {
    vi.mocked(setCallMuted).mockResolvedValue(undefined);

    await useCallStore.getState().toggleMute();
    expect(useCallStore.getState().isMuted).toBe(true);
    expect(setCallMuted).toHaveBeenCalledWith(true);

    await useCallStore.getState().toggleMute();
    expect(useCallStore.getState().isMuted).toBe(false);
    expect(setCallMuted).toHaveBeenCalledWith(false);
  });

  it("toggleMute reverts the optimistic update if the backend call fails", async () => {
    vi.mocked(setCallMuted).mockRejectedValue(new Error("no active call"));

    await useCallStore.getState().toggleMute();
    expect(useCallStore.getState().isMuted).toBe(false);
  });
});
