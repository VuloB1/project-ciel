import "@/lib/i18n";

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/ipc/events", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/lib/ipc/events")>()),
  useAppEvent: vi.fn(),
}));
vi.mock("@/lib/ipc/settings", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/lib/ipc/settings")>()),
  getLlmUsage: vi.fn(),
  resetLlmUsage: vi.fn(),
}));

import * as settingsIpc from "@/lib/ipc/settings";
import type { LlmProviderEntry, LlmUsageEntry } from "@/lib/ipc/settings";

import { ProviderUsagePanel } from "../ProviderUsagePanel";

function provider(id: string, label: string): LlmProviderEntry {
  return {
    id,
    kind: "generic",
    label,
    baseUrl: "http://localhost:1234/v1",
    model: "m",
    temperature: 0.7,
    maxTokens: 1024,
    hasApiKey: false,
    visionEnabled: false,
    enabled: true,
    pinned: false,
    privacyMode: "full",
    isLocal: true,
  };
}

function usage(providerId: string, overrides: Partial<LlmUsageEntry> = {}): LlmUsageEntry {
  return { providerId, requests: 0, errors: 0, inputTokensEstimate: 0, outputTokensEstimate: 0, lastUsedAt: null, lastError: null, ...overrides };
}

describe("ProviderUsagePanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows requests, estimated tokens, failures and the last error per provider, hiding unused ones", async () => {
    vi.mocked(settingsIpc.getLlmUsage).mockResolvedValue([
      usage("a", { requests: 12, errors: 2, inputTokensEstimate: 1500, outputTokensEstimate: 300, lastUsedAt: "2026-10-02T10:00:00Z", lastError: "se agotó el crédito de esta cuenta" }),
      usage("b"),
    ]);
    render(<ProviderUsagePanel providers={[provider("a", "OpenAI"), provider("b", "Claude")]} />);

    expect(await screen.findByText("OpenAI")).toBeInTheDocument();
    expect(screen.getByText("12 peticiones")).toBeInTheDocument();
    expect(screen.getByText(/≈ 1,5 mil tokens enviados · ≈ 300 recibidos/)).toBeInTheDocument();
    expect(screen.getByText(/2 fallos/)).toBeInTheDocument();
    expect(screen.getByText("Último error: se agotó el crédito de esta cuenta")).toBeInTheDocument();
    expect(screen.queryByText("Claude")).not.toBeInTheDocument();
    expect(screen.getByText(/son una estimación a partir del texto/)).toBeInTheDocument();
  });

  it("says so when nothing was used in the period", async () => {
    vi.mocked(settingsIpc.getLlmUsage).mockResolvedValue([usage("a")]);
    render(<ProviderUsagePanel providers={[provider("a", "OpenAI")]} />);

    expect(await screen.findByText("Todavía no hay uso en este periodo.")).toBeInTheDocument();
    expect(settingsIpc.getLlmUsage).toHaveBeenCalledWith("last7_days");
  });

  it("resets the numbers only after confirming", async () => {
    const user = userEvent.setup();
    vi.mocked(settingsIpc.getLlmUsage).mockResolvedValue([usage("a", { requests: 3 })]);
    vi.mocked(settingsIpc.resetLlmUsage).mockResolvedValue();
    render(<ProviderUsagePanel providers={[provider("a", "OpenAI")]} />);

    await user.click(await screen.findByRole("button", { name: "Reiniciar" }));
    expect(settingsIpc.resetLlmUsage).not.toHaveBeenCalled();

    const buttons = await screen.findAllByRole("button", { name: "Reiniciar" });
    await user.click(buttons[buttons.length - 1]!);

    await vi.waitFor(() => expect(settingsIpc.resetLlmUsage).toHaveBeenCalledTimes(1));
  });
});
