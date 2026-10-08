import "@/lib/i18n";

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/ipc/memory", () => ({
  proposeMemoryFusion: vi.fn(),
  applyMemoryFusion: vi.fn(),
}));

import { Dialog } from "@/design-system/Dialog";
import * as memoryIpc from "@/lib/ipc/memory";
import { useMemoryStore } from "@/stores/memoryStore";
import type { Memory } from "@/types/memory";

import { MemoryFusionDialog } from "../MemoryFusionDialog";

// `MemoryFusionDialog` renders only `DialogContent` (the caller supplies
// the `Dialog` root — same shape as `MemoryPage.tsx`'s `MemoryFormDialog`),
// so every render here needs that wrapper too.
function renderDialog(profileId: string, onClose: () => void) {
  return render(
    <Dialog open onOpenChange={() => {}}>
      <MemoryFusionDialog profileId={profileId} onClose={onClose} />
    </Dialog>,
  );
}

const PROFILE_ID = "p1";

function makeMemory(overrides: Partial<Memory> = {}): Memory {
  return {
    id: "mem-1",
    profileId: PROFILE_ID,
    title: "Le gusta el café solo",
    content: "Le gusta el café solo",
    summary: null,
    category: "preference",
    importance: 50,
    confidence: 100,
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

function makeProposal(id: string) {
  return {
    originals: [makeMemory({ id: `${id}-a`, title: "Café solo" }), makeMemory({ id: `${id}-b`, title: "Café con leche" })],
    title: "Café",
    summary: "Le gusta el café",
    content: "Le gusta el café, solo y con leche",
    category: "preference",
    importance: 60,
  };
}

describe("MemoryFusionDialog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useMemoryStore.setState({ memoriesByProfile: {}, isLoading: false, error: null });
  });

  it("shows an empty state when there is nothing to merge", async () => {
    vi.mocked(memoryIpc.proposeMemoryFusion).mockResolvedValue([]);
    const onClose = vi.fn();

    renderDialog(PROFILE_ID, onClose);

    expect(await screen.findByText("No se encontraron memorias para fusionar")).toBeInTheDocument();
  });

  it("lists each proposed group with its originals and the merged result", async () => {
    vi.mocked(memoryIpc.proposeMemoryFusion).mockResolvedValue([makeProposal("g1")]);

    renderDialog(PROFILE_ID, vi.fn());

    expect(await screen.findByText("Café solo")).toBeInTheDocument();
    expect(screen.getByText("Café con leche")).toBeInTheDocument();
    expect(screen.getByText("→ Café")).toBeInTheDocument();
  });

  it("does not apply anything until the user confirms", async () => {
    vi.mocked(memoryIpc.proposeMemoryFusion).mockResolvedValue([makeProposal("g1")]);

    renderDialog(PROFILE_ID, vi.fn());
    await screen.findByText("→ Café");

    expect(memoryIpc.applyMemoryFusion).not.toHaveBeenCalled();
  });

  it("unchecking the only group's switch disables confirming", async () => {
    const user = userEvent.setup();
    vi.mocked(memoryIpc.proposeMemoryFusion).mockResolvedValue([makeProposal("g1")]);
    renderDialog(PROFILE_ID, vi.fn());
    await screen.findByText("→ Café");
    expect(screen.getByText("Fusionar 1")).toBeEnabled();

    await user.click(screen.getByRole("switch"));

    expect(screen.getByText("Fusionar 0")).toBeDisabled();
  });

  it("confirming applies only the accepted groups and closes", async () => {
    const user = userEvent.setup();
    vi.mocked(memoryIpc.proposeMemoryFusion).mockResolvedValue([makeProposal("g1")]);
    vi.mocked(memoryIpc.applyMemoryFusion).mockResolvedValue([]);
    const onClose = vi.fn();

    renderDialog(PROFILE_ID, onClose);
    await screen.findByText("→ Café");

    await user.click(screen.getByText("Fusionar 1"));

    expect(memoryIpc.applyMemoryFusion).toHaveBeenCalledWith(
      PROFILE_ID,
      expect.arrayContaining([expect.objectContaining({ originalIds: ["g1-a", "g1-b"], title: "Café" })]),
    );
    await vi.waitFor(() => expect(onClose).toHaveBeenCalled());
  });
});
