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
  listLlmProviders: vi.fn(),
  updateLlmProvider: vi.fn(),
  addLlmProvider: vi.fn(),
  removeLlmProvider: vi.fn(),
  setLlmProviderEnabled: vi.fn(),
  setLlmProviderPinned: vi.fn(),
  setCurrentLlmProvider: vi.fn(),
  testLlmProvider: vi.fn(),
  getLlmUsage: vi.fn().mockResolvedValue([]),
  resetLlmUsage: vi.fn(),
}));

import * as settingsIpc from "@/lib/ipc/settings";
import type { LlmProviderEntry } from "@/lib/ipc/settings";

import { AiProvidersSettings } from "../AiProvidersSettings";

function entry(overrides: Partial<LlmProviderEntry> = {}): LlmProviderEntry {
  return {
    id: "a",
    kind: "generic",
    label: "LM Studio",
    baseUrl: "http://localhost:1234/v1",
    model: "qwen",
    temperature: 0.7,
    maxTokens: 1024,
    hasApiKey: false,
    visionEnabled: false,
    enabled: true,
    pinned: false,
    privacyMode: "full",
    isLocal: true,
    ...overrides,
  };
}

function listed(...entries: LlmProviderEntry[]) {
  vi.mocked(settingsIpc.listLlmProviders).mockResolvedValue({
    providers: entries,
    currentProviderId: entries[0]?.id ?? null,
    preferredProviderId: entries[0]?.id ?? null,
  });
}

// El slider de temperatura (Radix) mide su pista con ResizeObserver, que jsdom no trae.
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
vi.stubGlobal("ResizeObserver", ResizeObserverStub);

describe("AiProvidersSettings — privacidad por proveedor", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("marks each provider as local or cloud, and flags a restricted mode", async () => {
    listed(
      entry(),
      entry({ id: "b", kind: "openai", label: "OpenAI", baseUrl: null, isLocal: false, privacyMode: "public_only" }),
      entry({ id: "c", kind: "anthropic", label: "Claude", baseUrl: null, isLocal: false, privacyMode: "minimal" }),
    );
    render(<AiProvidersSettings />);

    expect(await screen.findByText("LM Studio")).toBeInTheDocument();
    expect(screen.getByText("Local")).toBeInTheDocument();
    expect(screen.getAllByText("Nube")).toHaveLength(2);
    expect(screen.getByText("Solo recuerdos públicos")).toBeInTheDocument();
    expect(screen.getByText("Privacidad mínima")).toBeInTheDocument();
    // El modo completo no necesita aviso.
    expect(screen.queryByText("Privacidad completa")).not.toBeInTheDocument();
  });

  it("shows the privacy choice when editing, with the explanation of the current mode", async () => {
    const user = userEvent.setup();
    listed(entry({ id: "b", kind: "openai", label: "OpenAI", baseUrl: null, isLocal: false, privacyMode: "public_only" }));
    render(<AiProvidersSettings />);

    await user.click(await screen.findByRole("button", { name: "Editar" }));

    expect(screen.getByText("Privacidad de este proveedor")).toBeInTheDocument();
    expect(screen.getByText("Es un servicio en la nube: elige qué datos tuyos puede recibir.")).toBeInTheDocument();
    expect(screen.getByText(/de tus recuerdos solo los marcados como públicos/)).toBeInTheDocument();
  });

  it("in minimal mode the vision switch is off and locked, and saving keeps the mode", async () => {
    const user = userEvent.setup();
    listed(entry({ id: "c", kind: "anthropic", label: "Claude", baseUrl: null, isLocal: false, privacyMode: "minimal", visionEnabled: true }));
    vi.mocked(settingsIpc.updateLlmProvider).mockResolvedValue(entry());
    render(<AiProvidersSettings />);

    await user.click(await screen.findByRole("button", { name: "Editar" }));

    const vision = screen.getByRole("switch", { name: "Visión (imágenes)" });
    expect(vision).toBeDisabled();
    expect(vision).toHaveAttribute("aria-checked", "false");
    expect(screen.getByText("Desactivado: en el modo mínimo no se mandan imágenes.")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Guardar" }));

    await vi.waitFor(() => expect(settingsIpc.updateLlmProvider).toHaveBeenCalled());
    const [, input] = vi.mocked(settingsIpc.updateLlmProvider).mock.calls[0]!;
    expect(input.privacyMode).toBe("minimal");
    expect(input.visionEnabled).toBe(false);
  });
});
