import "@/lib/i18n";

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/ipc/memory", () => ({
  listMemories: vi.fn(),
  searchMemories: vi.fn(),
  createMemory: vi.fn(),
  setMemoryPinned: vi.fn(),
  setMemoryScope: vi.fn(),
  deleteMemory: vi.fn(),
}));

import * as memoryIpc from "@/lib/ipc/memory";
import { useMemoryStore } from "@/stores/memoryStore";
import { useProfileStore } from "@/stores/profileStore";
import type { Memory } from "@/types/memory";
import { DEFAULT_IDENTITY, DEFAULT_PERSONALITY, DEFAULT_VOICE, type Profile } from "@/types/profile";

import { MemoryPage } from "../MemoryPage";

const PROFILE_ID = "p1";

function makeMemory(overrides: Partial<Memory> = {}): Memory {
  return {
    id: "mem-1",
    profileId: PROFILE_ID,
    title: "Le gusta el café",
    content: "Al usuario le gusta el café por la mañana.",
    summary: null,
    category: "preference",
    importance: 50,
    confidence: 1,
    source: "manual",
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

describe("MemoryPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useMemoryStore.setState({ memoriesByProfile: {}, isLoading: false, error: null });
    useProfileStore.setState({ profiles: [], activeProfileId: null, isLoading: false, error: null });
  });

  it("renders the no-profile empty state without a profile selected", () => {
    expect(() => render(<MemoryPage />)).not.toThrow();
  });

  it("renders the empty state without looping when the profile has no memories entry yet", async () => {
    // Same regression class as ConversationList's: the `memories` selector
    // must not fabricate a new array reference every render when there's no
    // store entry for the active profile yet.
    vi.mocked(memoryIpc.listMemories).mockResolvedValue([]);
    const profile = makeProfile();
    useProfileStore.setState({ profiles: [profile], activeProfileId: profile.id });

    expect(() => render(<MemoryPage />)).not.toThrow();
    expect(await screen.findByText("Todavía no hay memorias")).toBeInTheDocument();
  });

  it("does not delete immediately — clicking the trash icon opens a confirmation first", async () => {
    const user = userEvent.setup();
    const memory = makeMemory();
    const profile = makeProfile();
    vi.mocked(memoryIpc.listMemories).mockResolvedValue([memory]);
    useProfileStore.setState({ profiles: [profile], activeProfileId: profile.id });
    render(<MemoryPage />);

    await user.click(await screen.findByRole("button", { name: "Eliminar" }));

    expect(await screen.findByText(`¿Eliminar "${memory.title}"?`)).toBeInTheDocument();
    expect(memoryIpc.deleteMemory).not.toHaveBeenCalled();
  });

  it("confirming the dialog deletes the memory", async () => {
    const user = userEvent.setup();
    const memory = makeMemory();
    const profile = makeProfile();
    vi.mocked(memoryIpc.listMemories).mockResolvedValue([memory]);
    vi.mocked(memoryIpc.deleteMemory).mockResolvedValue(undefined);
    useProfileStore.setState({ profiles: [profile], activeProfileId: profile.id });
    render(<MemoryPage />);

    await user.click(await screen.findByRole("button", { name: "Eliminar" }));
    await screen.findByText(`¿Eliminar "${memory.title}"?`);

    // The trash icon button and the dialog's confirm button share the same
    // accessible name — the confirm button is whichever renders last.
    const deleteButtons = screen.getAllByRole("button", { name: "Eliminar" });
    await user.click(deleteButtons.at(-1)!);

    await vi.waitFor(() => expect(memoryIpc.deleteMemory).toHaveBeenCalledWith(memory.id));
  });

  it("making a memory public asks for confirmation first — nothing is exposed by a single click", async () => {
    const user = userEvent.setup();
    const memory = makeMemory({ scope: "private" });
    const profile = makeProfile();
    vi.mocked(memoryIpc.listMemories).mockResolvedValue([memory]);
    useProfileStore.setState({ profiles: [profile], activeProfileId: profile.id });
    render(<MemoryPage />);

    await user.click(await screen.findByRole("button", { name: "Hacer pública" }));

    expect(await screen.findByText(`¿Hacer pública «${memory.title}»?`)).toBeInTheDocument();
    expect(memoryIpc.setMemoryScope).not.toHaveBeenCalled();
  });

  it("confirming makes the memory public and shows the public badge", async () => {
    const user = userEvent.setup();
    const memory = makeMemory({ scope: "private" });
    const profile = makeProfile();
    vi.mocked(memoryIpc.listMemories).mockResolvedValue([memory]);
    vi.mocked(memoryIpc.setMemoryScope).mockResolvedValue(undefined);
    useProfileStore.setState({ profiles: [profile], activeProfileId: profile.id });
    render(<MemoryPage />);

    await user.click(await screen.findByRole("button", { name: "Hacer pública" }));
    await screen.findByText(`¿Hacer pública «${memory.title}»?`);
    // El botón del ícono y el del diálogo comparten nombre: el de confirmar es el último.
    await user.click(screen.getAllByRole("button", { name: "Hacer pública" }).at(-1)!);

    await vi.waitFor(() => expect(memoryIpc.setMemoryScope).toHaveBeenCalledWith(memory.id, "public"));
    expect(await screen.findByText("Pública")).toBeInTheDocument();
  });

  it("making a public memory private again is immediate, with no confirmation", async () => {
    const user = userEvent.setup();
    const memory = makeMemory({ scope: "public" });
    const profile = makeProfile();
    vi.mocked(memoryIpc.listMemories).mockResolvedValue([memory]);
    vi.mocked(memoryIpc.setMemoryScope).mockResolvedValue(undefined);
    useProfileStore.setState({ profiles: [profile], activeProfileId: profile.id });
    render(<MemoryPage />);

    await user.click(await screen.findByRole("button", { name: "Hacer privada" }));

    await vi.waitFor(() => expect(memoryIpc.setMemoryScope).toHaveBeenCalledWith(memory.id, "private"));
  });
});
