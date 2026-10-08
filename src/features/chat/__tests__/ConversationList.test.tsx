import "@/lib/i18n";

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/ipc/chat", () => ({
  deleteConversation: vi.fn(),
  extractMemories: vi.fn(),
}));

vi.mock("@/lib/ipc/memory", () => ({
  listMemories: vi.fn().mockResolvedValue([]),
}));

import * as chatIpc from "@/lib/ipc/chat";
import { useChatStore } from "@/stores/chatStore";
import { useMemoryStore } from "@/stores/memoryStore";
import type { Conversation } from "@/types/chat";

import { ConversationList } from "../ConversationList";

const PROFILE_ID = "profile-1";

function makeConversation(overrides: Partial<Conversation> = {}): Conversation {
  return {
    id: "conv-1",
    profileId: PROFILE_ID,
    title: "Charla sobre el clima",
    source: "chat",
    model: null,
    isArchived: false,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    isPinned: false,
    pinOrder: 0,
    ...overrides,
  };
}

describe("ConversationList", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useChatStore.setState({
      conversationsByProfile: {},
      activeConversationId: null,
      messagesByConversation: {},
      generatingConversations: {},
      requestIndex: {},
      isLoadingConversations: false,
    });
    useMemoryStore.setState({ memoriesByProfile: {}, isLoading: false, error: null });
  });

  it("renders the empty state without looping when the profile has no entry in the store yet", () => {
    // Regression test: a `useChatStore((s) => s.conversationsByProfile[id] ?? [])`
    // selector used to allocate a fresh array every render whenever `id` had
    // no entry yet, which Zustand's reference-equality check reads as a
    // change every render — an infinite "Maximum update depth exceeded" loop
    // that crashed the real app on first launch (no conversations created yet).
    expect(() => render(<ConversationList profileId="profile-without-conversations" />)).not.toThrow();
    expect(screen.getByText("Aún no hay conversaciones")).toBeInTheDocument();
  });

  // Plain `fireEvent.click`/`pointerDown` never opens this menu — Radix's
  // `DropdownMenuTrigger` opens on a real pointerdown-then-up sequence with
  // proper pointer capture, which `userEvent` reproduces and bare
  // `fireEvent` does not.
  async function openDeleteDialog() {
    const user = userEvent.setup();
    const conversation = makeConversation();
    useChatStore.setState({ conversationsByProfile: { [PROFILE_ID]: [conversation] } });
    render(<ConversationList profileId={PROFILE_ID} />);

    await user.click(screen.getByRole("button", { name: "Opciones" }));
    await user.click(await screen.findByText("Eliminar"));

    return { user, conversation };
  }

  it("does not delete immediately — clicking Eliminar opens a confirmation first", async () => {
    const { conversation } = await openDeleteDialog();

    expect(await screen.findByText(`¿Eliminar "${conversation.title}"?`)).toBeInTheDocument();
    expect(chatIpc.deleteConversation).not.toHaveBeenCalled();
  });

  it("confirming without the toggle deletes without extracting memories", async () => {
    const { user, conversation } = await openDeleteDialog();
    await screen.findByText(`¿Eliminar "${conversation.title}"?`);
    vi.mocked(chatIpc.deleteConversation).mockResolvedValue(undefined);

    // The dropdown's "Eliminar" item and the dialog's confirm button share
    // the same label — the confirm button is whichever renders last.
    const deleteButtons = screen.getAllByText("Eliminar");
    await user.click(deleteButtons.at(-1)!);

    await vi.waitFor(() => expect(chatIpc.deleteConversation).toHaveBeenCalledWith(conversation.id));
    expect(chatIpc.extractMemories).not.toHaveBeenCalled();
  });

  it("confirming with the toggle extracts memories before deleting", async () => {
    const { user, conversation } = await openDeleteDialog();
    await screen.findByText(`¿Eliminar "${conversation.title}"?`);
    vi.mocked(chatIpc.extractMemories).mockResolvedValue([]);
    vi.mocked(chatIpc.deleteConversation).mockResolvedValue(undefined);

    await user.click(screen.getByRole("switch", { name: "Extraer memorias antes de eliminar" }));
    const deleteButtons = screen.getAllByText("Eliminar");
    await user.click(deleteButtons.at(-1)!);

    await vi.waitFor(() => expect(chatIpc.deleteConversation).toHaveBeenCalledWith(conversation.id));
    expect(chatIpc.extractMemories).toHaveBeenCalledWith(conversation.id, PROFILE_ID);
  });
});
