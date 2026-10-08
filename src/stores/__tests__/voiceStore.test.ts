import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/ipc/voice", () => ({
  speak: vi.fn(),
  stopSpeaking: vi.fn(),
}));

import { useVoiceStore } from "../voiceStore";

describe("useVoiceStore", () => {
  beforeEach(() => {
    useVoiceStore.setState({ speaking: null, lastError: null });
  });

  it("started sets which message is speaking", () => {
    useVoiceStore.getState().handleAppEvent({ type: "voice_playback_started", profileId: "p1", messageId: "m1" });
    expect(useVoiceStore.getState().speaking).toEqual({ profileId: "p1", messageId: "m1" });
  });

  it("finished for the currently-speaking message clears it", () => {
    useVoiceStore.setState({ speaking: { profileId: "p1", messageId: "m1" } });
    useVoiceStore.getState().handleAppEvent({ type: "voice_playback_finished", profileId: "p1", messageId: "m1" });
    expect(useVoiceStore.getState().speaking).toBeNull();
  });

  it("finished for a different message than the one currently speaking is ignored", () => {
    // A stale event arriving after a newer `speak` call already started —
    // must not clobber the *current* speaking state.
    useVoiceStore.setState({ speaking: { profileId: "p1", messageId: "m2" } });
    useVoiceStore.getState().handleAppEvent({ type: "voice_playback_finished", profileId: "p1", messageId: "m1" });
    expect(useVoiceStore.getState().speaking).toEqual({ profileId: "p1", messageId: "m2" });
  });

  it("a preview (messageId null) starting and finishing round-trips", () => {
    useVoiceStore.getState().handleAppEvent({ type: "voice_playback_started", profileId: "p1", messageId: null });
    expect(useVoiceStore.getState().speaking).toEqual({ profileId: "p1", messageId: null });
    useVoiceStore.getState().handleAppEvent({ type: "voice_playback_finished", profileId: "p1", messageId: null });
    expect(useVoiceStore.getState().speaking).toBeNull();
  });

  it("error for the currently-speaking message clears it and records the error", () => {
    useVoiceStore.setState({ speaking: { profileId: "p1", messageId: "m1" } });
    useVoiceStore.getState().handleAppEvent({ type: "voice_playback_error", profileId: "p1", messageId: "m1", message: "boom" });
    expect(useVoiceStore.getState().speaking).toBeNull();
    expect(useVoiceStore.getState().lastError).toBe("boom");
  });

  it("error unrelated to the currently-speaking message still records the error but leaves speaking alone", () => {
    useVoiceStore.setState({ speaking: { profileId: "p1", messageId: "m2" } });
    useVoiceStore.getState().handleAppEvent({ type: "voice_playback_error", profileId: "p1", messageId: "m1", message: "boom" });
    expect(useVoiceStore.getState().speaking).toEqual({ profileId: "p1", messageId: "m2" });
    expect(useVoiceStore.getState().lastError).toBe("boom");
  });
});
