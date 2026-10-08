import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/ipc/memory", () => ({
  listMemories: vi.fn(),
  searchMemories: vi.fn(),
  createMemory: vi.fn(),
  setMemoryPinned: vi.fn(),
  setMemoryScope: vi.fn(),
  updateMemory: vi.fn(),
  deleteMemory: vi.fn(),
}));

import * as memoryIpc from "@/lib/ipc/memory";
import type { Memory } from "@/types/memory";
import { useMemoryStore } from "../memoryStore";

const PROFILE_ID = "p1";

function makeMemory(overrides: Partial<Memory> = {}): Memory {
  return {
    id: "m1",
    profileId: PROFILE_ID,
    title: "Bebida favorita",
    content: "Le encanta el café",
    summary: null,
    category: "preference",
    importance: 70,
    confidence: 90,
    source: "chat",
    sourceId: null,
    isPinned: false,
    scope: "private",
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    lastAccessedAt: null,
    accessCount: 0,
    ...overrides,
  };
}

describe("useMemoryStore", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useMemoryStore.setState({ memoriesByProfile: {}, isLoading: false, error: null });
  });

  it("load() populates memories for the profile", async () => {
    const memory = makeMemory();
    vi.mocked(memoryIpc.listMemories).mockResolvedValue([memory]);

    await useMemoryStore.getState().load(PROFILE_ID);

    const state = useMemoryStore.getState();
    expect(state.isLoading).toBe(false);
    expect(state.memoriesByProfile[PROFILE_ID]).toEqual([memory]);
  });

  it("load() sets an error message on failure", async () => {
    vi.mocked(memoryIpc.listMemories).mockRejectedValue(new Error("boom"));

    await useMemoryStore.getState().load(PROFILE_ID);

    expect(useMemoryStore.getState().error).toBe("boom");
  });

  it("search() with a non-empty query calls the search endpoint, not list", async () => {
    vi.mocked(memoryIpc.searchMemories).mockResolvedValue([makeMemory({ id: "found" })]);

    await useMemoryStore.getState().search(PROFILE_ID, "café");

    expect(memoryIpc.searchMemories).toHaveBeenCalledWith(PROFILE_ID, "café");
    expect(memoryIpc.listMemories).not.toHaveBeenCalled();
    expect(useMemoryStore.getState().memoriesByProfile[PROFILE_ID]?.[0]?.id).toBe("found");
  });

  it("search() with an empty query falls back to listing everything", async () => {
    vi.mocked(memoryIpc.listMemories).mockResolvedValue([makeMemory()]);

    await useMemoryStore.getState().search(PROFILE_ID, "   ");

    expect(memoryIpc.listMemories).toHaveBeenCalledWith(PROFILE_ID);
    expect(memoryIpc.searchMemories).not.toHaveBeenCalled();
  });

  it("create() prepends the new memory", async () => {
    const existing = makeMemory({ id: "existing" });
    const created = makeMemory({ id: "new" });
    useMemoryStore.setState({ memoriesByProfile: { [PROFILE_ID]: [existing] } });
    vi.mocked(memoryIpc.createMemory).mockResolvedValue(created);

    await useMemoryStore
      .getState()
      .create({ profileId: PROFILE_ID, title: "Algo", content: "algo", category: "fact", importance: 50 });

    expect(useMemoryStore.getState().memoriesByProfile[PROFILE_ID]?.map((m) => m.id)).toEqual(["new", "existing"]);
  });

  it("setPinned() updates the matching memory in place", async () => {
    const memory = makeMemory({ id: "m1", isPinned: false });
    useMemoryStore.setState({ memoriesByProfile: { [PROFILE_ID]: [memory] } });
    vi.mocked(memoryIpc.setMemoryPinned).mockResolvedValue(undefined);

    await useMemoryStore.getState().setPinned("m1", PROFILE_ID, true);

    expect(useMemoryStore.getState().memoriesByProfile[PROFILE_ID]?.[0]?.isPinned).toBe(true);
  });

  it("setScope() updates the matching memory's scope in place, only after the backend accepts it", async () => {
    const memory = makeMemory({ id: "m1", scope: "private" });
    useMemoryStore.setState({ memoriesByProfile: { [PROFILE_ID]: [memory] } });
    vi.mocked(memoryIpc.setMemoryScope).mockResolvedValue(undefined);

    await useMemoryStore.getState().setScope("m1", PROFILE_ID, "public");

    expect(memoryIpc.setMemoryScope).toHaveBeenCalledWith("m1", "public");
    expect(useMemoryStore.getState().memoriesByProfile[PROFILE_ID]?.[0]?.scope).toBe("public");
  });

  it("setScope() leaves the memory private when the backend rejects the change", async () => {
    const memory = makeMemory({ id: "m1", scope: "private" });
    useMemoryStore.setState({ memoriesByProfile: { [PROFILE_ID]: [memory] } });
    vi.mocked(memoryIpc.setMemoryScope).mockRejectedValue(new Error("boom"));

    await expect(useMemoryStore.getState().setScope("m1", PROFILE_ID, "public")).rejects.toThrow("boom");

    expect(useMemoryStore.getState().memoriesByProfile[PROFILE_ID]?.[0]?.scope).toBe("private");
  });

  it("remove() drops the memory from state", async () => {
    const memory = makeMemory({ id: "m1" });
    useMemoryStore.setState({ memoriesByProfile: { [PROFILE_ID]: [memory] } });
    vi.mocked(memoryIpc.deleteMemory).mockResolvedValue(undefined);

    await useMemoryStore.getState().remove("m1", PROFILE_ID);

    expect(useMemoryStore.getState().memoriesByProfile[PROFILE_ID]).toEqual([]);
  });

  it("update() replaces the matching memory with the server's response", async () => {
    const original = makeMemory({ id: "m1", content: "Texto original" });
    const updated = makeMemory({ id: "m1", content: "Texto corregido" });
    useMemoryStore.setState({ memoriesByProfile: { [PROFILE_ID]: [original] } });
    vi.mocked(memoryIpc.updateMemory).mockResolvedValue(updated);

    await useMemoryStore
      .getState()
      .update("m1", PROFILE_ID, { title: "Bebida favorita", content: "Texto corregido", category: "fact", importance: 50 });

    expect(useMemoryStore.getState().memoriesByProfile[PROFILE_ID]?.[0]?.content).toBe("Texto corregido");
  });

  it("handleAppEvent refetches only when the profile's memories were already loaded", () => {
    vi.mocked(memoryIpc.listMemories).mockResolvedValue([makeMemory()]);

    // Not loaded yet for this profile — no point fetching data nobody's viewing.
    useMemoryStore.getState().handleAppEvent({ type: "memories_updated", profileId: PROFILE_ID });
    expect(memoryIpc.listMemories).not.toHaveBeenCalled();

    // Once there's an entry (Memoria tab was visited), the event triggers a refresh.
    useMemoryStore.setState({ memoriesByProfile: { [PROFILE_ID]: [] } });
    useMemoryStore.getState().handleAppEvent({ type: "memories_updated", profileId: PROFILE_ID });
    expect(memoryIpc.listMemories).toHaveBeenCalledWith(PROFILE_ID);
  });

  it("handleAppEvent ignores unrelated events", () => {
    expect(() => useMemoryStore.getState().handleAppEvent({ type: "heartbeat", timestamp: "now" })).not.toThrow();
    expect(memoryIpc.listMemories).not.toHaveBeenCalled();
  });
});
