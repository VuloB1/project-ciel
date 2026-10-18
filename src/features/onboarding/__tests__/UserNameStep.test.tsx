import "@/lib/i18n";

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/ipc/settings", () => ({
  getUserName: vi.fn(),
  setUserName: vi.fn(),
}));

import * as settingsIpc from "@/lib/ipc/settings";

import { UserNameStep } from "../UserNameStep";

describe("UserNameStep", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(settingsIpc.getUserName).mockResolvedValue(null);
    vi.mocked(settingsIpc.setUserName).mockResolvedValue("Vulito");
  });

  it("saves the typed name when the footer's Continuar runs the registered submit", async () => {
    let submit: (() => Promise<void>) | null = null;
    render(<UserNameStep registerSubmit={(fn) => (submit = fn)} />);

    await userEvent.type(screen.getByRole("textbox"), "Vulito");
    await submit!();

    expect(settingsIpc.setUserName).toHaveBeenCalledWith("Vulito");
  });

  it("starts with the name already saved and unregisters when it unmounts", async () => {
    vi.mocked(settingsIpc.getUserName).mockResolvedValue("Ana");
    const register = vi.fn();
    const { unmount } = render(<UserNameStep registerSubmit={register} />);

    expect(await screen.findByDisplayValue("Ana")).toBeInTheDocument();
    unmount();
    expect(register).toHaveBeenLastCalledWith(null);
  });

  it("keeps the step open (rethrows) when saving fails", async () => {
    vi.mocked(settingsIpc.setUserName).mockRejectedValue(new Error("boom"));
    let submit: (() => Promise<void>) | null = null;
    render(<UserNameStep registerSubmit={(fn) => (submit = fn)} />);

    await expect(submit!()).rejects.toThrow("boom");
  });
});
