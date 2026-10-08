import { beforeEach, describe, expect, it, vi } from "vitest";

import { DEFAULT_IDENTITY, DEFAULT_PERSONALITY, DEFAULT_VOICE, type Profile } from "@/types/profile";

vi.mock("@/lib/ipc/profiles", () => ({
  listProfiles: vi.fn(),
  createProfile: vi.fn(),
  updateProfile: vi.fn(),
  duplicateProfile: vi.fn(),
  deleteProfile: vi.fn(),
}));

import * as profilesIpc from "@/lib/ipc/profiles";
import { useProfileStore } from "../profileStore";

function makeProfile(overrides: Partial<Profile> = {}): Profile {
  return {
    id: "p1",
    name: "Ciel",
    nickname: null,
    avatarPath: null,
    description: null,
    ageFictional: null,
    pronouns: null,
    occupation: null,
    species: null,
    accentColor: null,
    theme: null,
    isArchived: false,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    identity: DEFAULT_IDENTITY,
    personality: DEFAULT_PERSONALITY,
    voice: DEFAULT_VOICE,
    avatarIdlePath: null,
    avatarThinkingPath: null,
    avatarTalkingPath: null,
    ...overrides,
  };
}

describe("useProfileStore", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useProfileStore.setState({ profiles: [], activeProfileId: null, isLoading: true, error: null });
    localStorage.clear();
  });

  it("load() populates profiles and defaults activeProfileId to the first one", async () => {
    const profile = makeProfile();
    vi.mocked(profilesIpc.listProfiles).mockResolvedValue([profile]);

    await useProfileStore.getState().load();

    const state = useProfileStore.getState();
    expect(state.isLoading).toBe(false);
    expect(state.profiles).toEqual([profile]);
    expect(state.activeProfileId).toBe("p1");
  });

  it("load() keeps the current active profile if it still exists", async () => {
    const a = makeProfile({ id: "a" });
    const b = makeProfile({ id: "b" });
    useProfileStore.setState({ activeProfileId: "b" });
    vi.mocked(profilesIpc.listProfiles).mockResolvedValue([a, b]);

    await useProfileStore.getState().load();

    expect(useProfileStore.getState().activeProfileId).toBe("b");
  });

  it("load() sets an error message and stops loading on failure", async () => {
    vi.mocked(profilesIpc.listProfiles).mockRejectedValue(new Error("boom"));

    await useProfileStore.getState().load();

    const state = useProfileStore.getState();
    expect(state.isLoading).toBe(false);
    expect(state.error).toBe("boom");
  });

  it("create() prepends the new profile and makes it active", async () => {
    const existing = makeProfile({ id: "existing" });
    const created = makeProfile({ id: "new" });
    useProfileStore.setState({ profiles: [existing] });
    vi.mocked(profilesIpc.createProfile).mockResolvedValue(created);

    await useProfileStore.getState().create("Nuevo personaje");

    const state = useProfileStore.getState();
    expect(state.profiles.map((p) => p.id)).toEqual(["new", "existing"]);
    expect(state.activeProfileId).toBe("new");
  });

  it("remove() falls back to another profile when the active one is deleted", async () => {
    const a = makeProfile({ id: "a" });
    const b = makeProfile({ id: "b" });
    useProfileStore.setState({ profiles: [a, b], activeProfileId: "a" });
    vi.mocked(profilesIpc.deleteProfile).mockResolvedValue(undefined);

    await useProfileStore.getState().remove("a");

    const state = useProfileStore.getState();
    expect(state.profiles.map((p) => p.id)).toEqual(["b"]);
    expect(state.activeProfileId).toBe("b");
  });

  it("update() replaces the matching profile in place", async () => {
    const original = makeProfile({ id: "a", name: "Ciel" });
    const updated = makeProfile({ id: "a", name: "Ciel Updated" });
    useProfileStore.setState({ profiles: [original] });
    vi.mocked(profilesIpc.updateProfile).mockResolvedValue(updated);

    await useProfileStore.getState().update("a", {
      name: "Ciel Updated",
      nickname: null,
      description: null,
      ageFictional: null,
      pronouns: null,
      occupation: null,
      species: null,
      accentColor: null,
      theme: null,
      identity: DEFAULT_IDENTITY,
      personality: DEFAULT_PERSONALITY,
    });

    expect(useProfileStore.getState().profiles[0]?.name).toBe("Ciel Updated");
  });
});
