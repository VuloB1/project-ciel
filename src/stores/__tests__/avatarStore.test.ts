import { beforeEach, describe, expect, it } from "vitest";

import { useAvatarStore } from "../avatarStore";

describe("useAvatarStore", () => {
  beforeEach(() => {
    useAvatarStore.setState({ visualState: "idle", trackedProfileId: null, activeRequestId: null });
  });

  it("events are ignored until a profile is being tracked", () => {
    useAvatarStore.getState().handleAppEvent({ type: "ai_response_started", requestId: "r1", profileId: "p1", conversationId: "c1", messageId: "m1" });
    expect(useAvatarStore.getState().visualState).toBe("idle");
  });

  it("a response starting for the tracked profile goes to thinking", () => {
    useAvatarStore.getState().setTrackedProfileId("p1");
    useAvatarStore.getState().handleAppEvent({ type: "ai_response_started", requestId: "r1", profileId: "p1", conversationId: "c1", messageId: "m1" });
    expect(useAvatarStore.getState().visualState).toBe("thinking");
  });

  it("a response starting for a different profile is ignored", () => {
    useAvatarStore.getState().setTrackedProfileId("p1");
    useAvatarStore.getState().handleAppEvent({ type: "ai_response_started", requestId: "r1", profileId: "p2", conversationId: "c1", messageId: "m1" });
    expect(useAvatarStore.getState().visualState).toBe("idle");
  });

  it("finishing the correlated request goes back to idle", () => {
    useAvatarStore.getState().setTrackedProfileId("p1");
    useAvatarStore.getState().handleAppEvent({ type: "ai_response_started", requestId: "r1", profileId: "p1", conversationId: "c1", messageId: "m1" });
    useAvatarStore.getState().handleAppEvent({ type: "ai_response_finished", requestId: "r1" });
    expect(useAvatarStore.getState().visualState).toBe("idle");
  });

  it("a finished event for an unrelated request is ignored", () => {
    // A different profile's generation wrapping up must not clobber this
    // profile's own "thinking" state (events don't carry profileId here).
    useAvatarStore.getState().setTrackedProfileId("p1");
    useAvatarStore.getState().handleAppEvent({ type: "ai_response_started", requestId: "r1", profileId: "p1", conversationId: "c1", messageId: "m1" });
    useAvatarStore.getState().handleAppEvent({ type: "ai_response_finished", requestId: "r2" });
    expect(useAvatarStore.getState().visualState).toBe("thinking");
  });

  it("voice playback starting/finishing for the tracked profile toggles talking", () => {
    useAvatarStore.getState().setTrackedProfileId("p1");
    useAvatarStore.getState().handleAppEvent({ type: "voice_playback_started", profileId: "p1", messageId: "m1" });
    expect(useAvatarStore.getState().visualState).toBe("talking");
    useAvatarStore.getState().handleAppEvent({ type: "voice_playback_finished", profileId: "p1", messageId: "m1" });
    expect(useAvatarStore.getState().visualState).toBe("idle");
  });

  it("switching tracked profile resets to idle", () => {
    useAvatarStore.getState().setTrackedProfileId("p1");
    useAvatarStore.getState().handleAppEvent({ type: "voice_playback_started", profileId: "p1", messageId: "m1" });
    expect(useAvatarStore.getState().visualState).toBe("talking");

    useAvatarStore.getState().setTrackedProfileId("p2");
    expect(useAvatarStore.getState().visualState).toBe("idle");
  });
});
