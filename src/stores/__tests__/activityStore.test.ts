import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/ipc/activity", () => ({
  listRecentActivity: vi.fn(),
  clearActivity: vi.fn(),
}));

import * as activityIpc from "@/lib/ipc/activity";
import type { ActivityEvent } from "@/lib/ipc/activity";
import { useActivityStore } from "../activityStore";

function makeEvent(overrides: Partial<ActivityEvent> = {}): ActivityEvent {
  return {
    id: "e1",
    timestamp: "2026-01-01T00:00:00Z",
    level: "INFO",
    category: "SYSTEM",
    module: "bootstrap",
    action: "startup",
    profileId: null,
    conversationId: null,
    requestId: null,
    durationMs: null,
    status: "success",
    summary: "AIVT Companion se inició correctamente",
    detailsJson: null,
    ...overrides,
  };
}

describe("useActivityStore", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useActivityStore.setState({ events: [], isLoading: false, error: null });
  });

  it("load() populates events", async () => {
    const event = makeEvent();
    vi.mocked(activityIpc.listRecentActivity).mockResolvedValue([event]);

    await useActivityStore.getState().load();

    const state = useActivityStore.getState();
    expect(state.isLoading).toBe(false);
    expect(state.events).toEqual([event]);
  });

  it("load() sets an error message on failure without throwing", async () => {
    vi.mocked(activityIpc.listRecentActivity).mockRejectedValue(new Error("boom"));

    await expect(useActivityStore.getState().load()).resolves.toBeUndefined();

    expect(useActivityStore.getState().error).toBe("boom");
    expect(useActivityStore.getState().isLoading).toBe(false);
  });

  it("clear() empties the events list", async () => {
    useActivityStore.setState({ events: [makeEvent()] });
    vi.mocked(activityIpc.clearActivity).mockResolvedValue(undefined);

    await useActivityStore.getState().clear();

    expect(useActivityStore.getState().events).toEqual([]);
  });

  it("clear() propagates a failure instead of swallowing it", async () => {
    useActivityStore.setState({ events: [makeEvent()] });
    vi.mocked(activityIpc.clearActivity).mockRejectedValue(new Error("boom"));

    await expect(useActivityStore.getState().clear()).rejects.toThrow("boom");
    // Unlike `load()`, `clear()` has no own error handling — the caller
    // (the confirm dialog) is responsible for reporting the failure.
    expect(useActivityStore.getState().events).toHaveLength(1);
  });
});
