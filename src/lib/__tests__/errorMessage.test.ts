import { describe, expect, it } from "vitest";

import { errorMessage } from "../errorMessage";

describe("errorMessage", () => {
  it("extracts message from a real Error instance", () => {
    expect(errorMessage(new Error("boom"))).toBe("boom");
  });

  it("extracts message from an AppError-shaped object (Tauri IPC rejection)", () => {
    expect(errorMessage({ module: "twitch", code: "TWITCH_NOT_CONNECTED", message: "no hay una cuenta conectada" })).toBe(
      "no hay una cuenta conectada",
    );
  });

  it("falls back to String() for anything without a usable message", () => {
    expect(errorMessage("plain string")).toBe("plain string");
    expect(errorMessage(42)).toBe("42");
  });

  it("does not crash on null or undefined", () => {
    expect(errorMessage(null)).toBe("null");
    expect(errorMessage(undefined)).toBe("undefined");
  });
});
