import "@/lib/i18n";

import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/ipc/activity", () => ({
  listRecentActivity: vi.fn(),
  clearActivity: vi.fn(),
}));

import * as activityIpc from "@/lib/ipc/activity";
import type { ActivityEvent } from "@/lib/ipc/activity";
import { useActivityStore } from "@/stores/activityStore";

import { ActivityPage } from "../ActivityPage";

function makeEvent(overrides: Partial<ActivityEvent> = {}): ActivityEvent {
  return {
    id: "e1",
    timestamp: "2026-01-01T00:00:00Z",
    level: "INFO",
    category: "LLM",
    module: "chat",
    action: "generate",
    profileId: null,
    conversationId: null,
    requestId: null,
    durationMs: null,
    status: "success",
    summary: "Respuesta generada",
    detailsJson: null,
    ...overrides,
  };
}

describe("ActivityPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useActivityStore.setState({ events: [], isLoading: false, error: null });
  });

  it("renders the empty state with no events", async () => {
    vi.mocked(activityIpc.listRecentActivity).mockResolvedValue([]);

    render(<ActivityPage />);

    expect(await screen.findByText("Todavía no hay actividad")).toBeInTheDocument();
  });

  it("renders a loaded event's summary", async () => {
    vi.mocked(activityIpc.listRecentActivity).mockResolvedValue([makeEvent({ summary: "Respondió a Vulito" })]);

    render(<ActivityPage />);

    expect(await screen.findByText("Respondió a Vulito")).toBeInTheDocument();
  });

  // Plain `fireEvent.click` never opens this menu — Radix's
  // `DropdownMenuTrigger` opens on a real pointerdown-then-up sequence with
  // proper pointer capture, which `userEvent` reproduces and bare
  // `fireEvent` does not (same gotcha as ConversationList's "⋯" menu).
  async function openOptionsMenu() {
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Opciones" }));
    return user;
  }

  it("hides SYSTEM-category events by default, and reveals them via the menu", async () => {
    vi.mocked(activityIpc.listRecentActivity).mockResolvedValue([
      makeEvent({ category: "SYSTEM", summary: "AIVT Companion se inició correctamente" }),
    ]);

    render(<ActivityPage />);

    // Nothing else is loaded, so hiding the only event lands on the
    // (filtered) empty state rather than the summary text ever appearing.
    expect(await screen.findByText("Sin resultados")).toBeInTheDocument();
    expect(screen.queryByText("AIVT Companion se inició correctamente")).not.toBeInTheDocument();

    const user = await openOptionsMenu();
    await user.click(await screen.findByText("Ver eventos del sistema"));

    expect(await screen.findByText("AIVT Companion se inició correctamente")).toBeInTheDocument();
  });

  it("clears the feed only after confirming, not on the first click", async () => {
    vi.mocked(activityIpc.listRecentActivity).mockResolvedValue([makeEvent()]);
    vi.mocked(activityIpc.clearActivity).mockResolvedValue(undefined);

    render(<ActivityPage />);
    expect(await screen.findByText("Respuesta generada")).toBeInTheDocument();

    const user = await openOptionsMenu();
    await user.click(await screen.findByText("Limpiar eventos"));
    expect(await screen.findByText("¿Limpiar el registro de actividad?")).toBeInTheDocument();
    // Merely opening the confirmation must not have cleared anything yet.
    expect(activityIpc.clearActivity).not.toHaveBeenCalled();
    expect(screen.getByText("Respuesta generada")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Limpiar eventos" }));

    expect(await screen.findByText("Todavía no hay actividad")).toBeInTheDocument();
    expect(activityIpc.clearActivity).toHaveBeenCalledTimes(1);
  });
});
