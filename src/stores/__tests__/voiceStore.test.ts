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

  it("playing a second saved clip pauses the first, and stop pauses the current one", async () => {
    const made: { pause: ReturnType<typeof vi.fn> }[] = [];
    vi.stubGlobal(
      "Audio",
      class {
        pause = vi.fn();
        play = vi.fn().mockResolvedValue(undefined);
        addEventListener = vi.fn();
        constructor() {
          made.push(this);
        }
      },
    );
    await useVoiceStore.getState().playStored("m1", "data:audio/mpeg;base64,AA==");
    await useVoiceStore.getState().playStored("m2", "data:audio/mpeg;base64,AA==");
    expect(made[0]?.pause).toHaveBeenCalled();
    expect(useVoiceStore.getState().storedPlayingId).toBe("m2");
    await useVoiceStore.getState().stop();
    expect(made[1]?.pause).toHaveBeenCalled();
    expect(useVoiceStore.getState().storedPlayingId).toBeNull();
    vi.unstubAllGlobals();
  });
});
