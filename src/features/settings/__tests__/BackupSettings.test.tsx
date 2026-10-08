import "@/lib/i18n";

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));
vi.mock("@/lib/ipc/backup", () => ({
  createBackup: vi.fn(),
  prepareRestore: vi.fn(),
  cancelRestore: vi.fn(),
  restartApp: vi.fn(),
}));

import { open, save } from "@tauri-apps/plugin-dialog";

import * as backupIpc from "@/lib/ipc/backup";

import { BackupSettings, formatBytes } from "../BackupSettings";

const PREVIEW = { createdAt: "2026-01-01T00:00:00Z", appVersion: "0.1.0", files: 3, sizeBytes: 5 * 1024 * 1024 };

describe("BackupSettings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("tells the user the API keys are not part of the backup", () => {
    render(<BackupSettings />);
    expect(screen.getByText(/No incluye tus API keys/)).toBeInTheDocument();
  });

  it("creates a backup at the chosen path", async () => {
    const user = userEvent.setup();
    vi.mocked(save).mockResolvedValue("C:/copias/ciel.aivtbackup");
    vi.mocked(backupIpc.createBackup).mockResolvedValue({ path: "C:/copias/ciel.aivtbackup", sizeBytes: 2048, files: 4, createdAt: "x" });
    render(<BackupSettings />);

    await user.click(screen.getByRole("button", { name: "Crear copia de seguridad" }));

    await vi.waitFor(() => expect(backupIpc.createBackup).toHaveBeenCalledWith("C:/copias/ciel.aivtbackup"));
  });

  it("does nothing when the save dialog is cancelled", async () => {
    const user = userEvent.setup();
    vi.mocked(save).mockResolvedValue(null);
    render(<BackupSettings />);

    await user.click(screen.getByRole("button", { name: "Crear copia de seguridad" }));

    expect(backupIpc.createBackup).not.toHaveBeenCalled();
  });

  it("validates a picked backup and asks for confirmation — nothing is restarted yet", async () => {
    const user = userEvent.setup();
    vi.mocked(open).mockResolvedValue("C:/copias/ciel.aivtbackup");
    vi.mocked(backupIpc.prepareRestore).mockResolvedValue(PREVIEW);
    render(<BackupSettings />);

    await user.click(screen.getByRole("button", { name: "Restaurar desde un archivo…" }));

    expect(await screen.findByText("¿Restaurar esta copia?")).toBeInTheDocument();
    expect(backupIpc.prepareRestore).toHaveBeenCalledWith("C:/copias/ciel.aivtbackup");
    expect(backupIpc.restartApp).not.toHaveBeenCalled();
  });

  it("cancelling the confirmation discards the prepared restore", async () => {
    const user = userEvent.setup();
    vi.mocked(open).mockResolvedValue("C:/copias/ciel.aivtbackup");
    vi.mocked(backupIpc.prepareRestore).mockResolvedValue(PREVIEW);
    vi.mocked(backupIpc.cancelRestore).mockResolvedValue(undefined);
    render(<BackupSettings />);

    await user.click(screen.getByRole("button", { name: "Restaurar desde un archivo…" }));
    await screen.findByText("¿Restaurar esta copia?");
    await user.click(screen.getByRole("button", { name: "Cancelar" }));

    await vi.waitFor(() => expect(backupIpc.cancelRestore).toHaveBeenCalled());
    expect(backupIpc.restartApp).not.toHaveBeenCalled();
  });

  it("confirming restarts the app and does not discard the prepared restore", async () => {
    const user = userEvent.setup();
    vi.mocked(open).mockResolvedValue("C:/copias/ciel.aivtbackup");
    vi.mocked(backupIpc.prepareRestore).mockResolvedValue(PREVIEW);
    vi.mocked(backupIpc.restartApp).mockResolvedValue(undefined);
    render(<BackupSettings />);

    await user.click(screen.getByRole("button", { name: "Restaurar desde un archivo…" }));
    await screen.findByText("¿Restaurar esta copia?");
    await user.click(screen.getByRole("button", { name: "Restaurar y reiniciar" }));

    await vi.waitFor(() => expect(backupIpc.restartApp).toHaveBeenCalled());
    expect(backupIpc.cancelRestore).not.toHaveBeenCalled();
  });

  it("an invalid backup shows an error and never reaches the confirmation", async () => {
    const user = userEvent.setup();
    vi.mocked(open).mockResolvedValue("C:/x/mala.aivtbackup");
    vi.mocked(backupIpc.prepareRestore).mockRejectedValue(new Error("el archivo no es una copia de seguridad válida"));
    render(<BackupSettings />);

    await user.click(screen.getByRole("button", { name: "Restaurar desde un archivo…" }));

    await vi.waitFor(() => expect(backupIpc.prepareRestore).toHaveBeenCalled());
    expect(screen.queryByText("¿Restaurar esta copia?")).not.toBeInTheDocument();
  });

  it("formats sizes for people", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(2048)).toBe("2.0 KB");
    expect(formatBytes(5 * 1024 * 1024)).toBe("5.0 MB");
    expect(formatBytes(150 * 1024 * 1024)).toBe("150 MB");
  });
});
