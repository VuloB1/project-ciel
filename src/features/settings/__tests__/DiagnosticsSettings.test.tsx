import "@/lib/i18n";

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));
vi.mock("@/lib/ipc/diagnostics", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/lib/ipc/diagnostics")>()),
  runDiagnostics: vi.fn(),
  getDiagnosticsText: vi.fn(),
  saveDiagnostics: vi.fn(),
}));

import { save } from "@tauri-apps/plugin-dialog";

import * as diagnosticsIpc from "@/lib/ipc/diagnostics";
import type { DiagnosticsReport } from "@/lib/ipc/diagnostics";

import { DiagnosticsSettings } from "../DiagnosticsSettings";

function makeReport(overrides: Partial<DiagnosticsReport> = {}): DiagnosticsReport {
  return {
    overall: "ok",
    generatedAt: "2026-10-02T00:00:00Z",
    system: { appVersion: "0.1.0", os: "windows", arch: "x86_64", dataDir: "C:/datos" },
    database: {
      schemaVersion: 19,
      latestSchemaVersion: 19,
      sizeBytes: 2048,
      integrityOk: true,
      integrityMessage: null,
      counts: [{ table: "profiles", count: 2 }],
    },
    storage: { attachmentsBytes: 0, avatarsBytes: 0, modelsBytes: 0, logsBytes: 0, preRestoreBytes: 0 },
    services: [{ name: "twitch", status: "running" }],
    ai: {
      llm: [
        { label: "LM Studio", kind: "generic", enabled: true, pinned: true, hasApiKey: false, ready: true, current: true, preferred: true, cooldownSecs: null },
      ],
      tts: [{ label: "Nativo", kind: "native", enabled: true, hasApiKey: false, current: true }],
      sttModels: [{ size: "base", downloaded: true, selected: true }],
      embeddingDownloaded: true,
    },
    problems: [],
    checks: [
      { id: "database_integrity", severity: "ok", params: {} },
      { id: "llm_ready", severity: "ok", params: {} },
    ],
    ...overrides,
  };
}

describe("DiagnosticsSettings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows the overall state and each check in plain language", async () => {
    vi.mocked(diagnosticsIpc.runDiagnostics).mockResolvedValue(makeReport());
    render(<DiagnosticsSettings />);

    expect(await screen.findByText("Todo en orden")).toBeInTheDocument();
    expect(screen.getByText("La base de datos está íntegra.")).toBeInTheDocument();
    expect(screen.getByText("El proveedor de IA en uso está listo.")).toBeInTheDocument();
    expect(screen.getByText("Twitch")).toBeInTheDocument();
    expect(screen.getByText("En marcha")).toBeInTheDocument();
  });

  it("explains an error check with its parameters", async () => {
    vi.mocked(diagnosticsIpc.runDiagnostics).mockResolvedValue(
      makeReport({
        overall: "error",
        checks: [
          { id: "database_integrity", severity: "error", params: { message: "fila 3 dañada" } },
          { id: "llm_cooling_down", severity: "warning", params: { label: "LM Studio", seconds: "42" } },
        ],
      }),
    );
    render(<DiagnosticsSettings />);

    expect(await screen.findByText("Hay problemas")).toBeInTheDocument();
    expect(screen.getByText(/fila 3 dañada/)).toBeInTheDocument();
    expect(screen.getByText(/LM Studio falló hace poco y está en pausa unos 42 s/)).toBeInTheDocument();
  });

  it("formats the reclaimable pre-restore size", async () => {
    vi.mocked(diagnosticsIpc.runDiagnostics).mockResolvedValue(
      makeReport({ checks: [{ id: "pre_restore_leftover", severity: "info", params: { bytes: String(5 * 1024 * 1024) } }] }),
    );
    render(<DiagnosticsSettings />);

    expect(await screen.findByText(/5\.0 MB de datos de antes de una restauración/)).toBeInTheDocument();
  });

  it("an unknown check id still renders something instead of a raw key", async () => {
    vi.mocked(diagnosticsIpc.runDiagnostics).mockResolvedValue(
      makeReport({ checks: [{ id: "una_nueva", severity: "warning", params: {} }] }),
    );
    render(<DiagnosticsSettings />);

    expect(await screen.findByText("Comprobación «una_nueva».")).toBeInTheDocument();
  });

  it("copies the redacted text report to the clipboard", async () => {
    const user = userEvent.setup();
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    vi.mocked(diagnosticsIpc.runDiagnostics).mockResolvedValue(makeReport());
    vi.mocked(diagnosticsIpc.getDiagnosticsText).mockResolvedValue("== Diagnóstico ==");
    render(<DiagnosticsSettings />);

    await user.click(await screen.findByRole("button", { name: "Copiar informe" }));

    await vi.waitFor(() => expect(writeText).toHaveBeenCalledWith("== Diagnóstico =="));
  });

  it("saves the report where the user chooses, and does nothing if they cancel", async () => {
    const user = userEvent.setup();
    vi.mocked(diagnosticsIpc.runDiagnostics).mockResolvedValue(makeReport());
    vi.mocked(save).mockResolvedValueOnce(null).mockResolvedValueOnce("C:/x/diag.txt");
    render(<DiagnosticsSettings />);

    await user.click(await screen.findByRole("button", { name: "Guardar informe…" }));
    expect(diagnosticsIpc.saveDiagnostics).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Guardar informe…" }));
    await vi.waitFor(() => expect(diagnosticsIpc.saveDiagnostics).toHaveBeenCalledWith("C:/x/diag.txt"));
  });

  it("checks again when asked", async () => {
    const user = userEvent.setup();
    vi.mocked(diagnosticsIpc.runDiagnostics).mockResolvedValue(makeReport());
    render(<DiagnosticsSettings />);

    await user.click(await screen.findByRole("button", { name: "Volver a comprobar" }));

    await vi.waitFor(() => expect(diagnosticsIpc.runDiagnostics).toHaveBeenCalledTimes(2));
  });
});
