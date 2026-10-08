import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/ipc/mood", () => ({
  getMood: vi.fn(),
  resetMood: vi.fn(),
}));

import * as moodIpc from "@/lib/ipc/mood";
import type { ProfileMood } from "@/lib/ipc/mood";
import { useMoodStore } from "../moodStore";

const PROFILE_ID = "p1";

function makeMood(overrides: Partial<ProfileMood> = {}): ProfileMood {
  return { profileId: PROFILE_ID, valence: 50, energy: 50, updatedAt: "2026-01-01T00:00:00Z", ...overrides };
}

describe("useMoodStore", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useMoodStore.setState({ moodByProfile: {} });
  });

  it("load() populates the mood for the profile", async () => {
    const mood = makeMood({ valence: 80 });
    vi.mocked(moodIpc.getMood).mockResolvedValue(mood);

    await useMoodStore.getState().load(PROFILE_ID);

    expect(useMoodStore.getState().moodByProfile[PROFILE_ID]).toEqual(mood);
  });

  it("reset() replaces the stored mood with the server's baseline response", async () => {
    useMoodStore.setState({ moodByProfile: { [PROFILE_ID]: makeMood({ valence: 90, energy: 10 }) } });
    vi.mocked(moodIpc.resetMood).mockResolvedValue(makeMood());

    await useMoodStore.getState().reset(PROFILE_ID);

    expect(useMoodStore.getState().moodByProfile[PROFILE_ID]).toEqual(makeMood());
  });

  it("handleAppEvent updates the profile's mood in place", () => {
    useMoodStore.setState({ moodByProfile: { [PROFILE_ID]: makeMood() } });

    useMoodStore.getState().handleAppEvent({ type: "mood_updated", profileId: PROFILE_ID, valence: 70, energy: 30 });

    const mood = useMoodStore.getState().moodByProfile[PROFILE_ID];
    expect(mood?.valence).toBe(70);
    expect(mood?.energy).toBe(30);
  });

  it("handleAppEvent ignores unrelated events", () => {
    expect(() => useMoodStore.getState().handleAppEvent({ type: "heartbeat", timestamp: "now" })).not.toThrow();
    expect(useMoodStore.getState().moodByProfile[PROFILE_ID]).toBeUndefined();
  });
});
