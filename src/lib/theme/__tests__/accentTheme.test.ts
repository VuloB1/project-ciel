import { describe, expect, it } from "vitest";

import { computeAccentTheme, DEFAULT_ACCENT_HEX, hexToRgbTriplet } from "../accentTheme";

describe("hexToRgbTriplet", () => {
  it("parses a 6-digit hex color", () => {
    expect(hexToRgbTriplet("#00eeff")).toBe("0 238 255");
  });

  it("parses a 3-digit shorthand hex color", () => {
    expect(hexToRgbTriplet("#0ef")).toBe("0 238 255");
  });

  it("falls back to the default accent for invalid input", () => {
    expect(hexToRgbTriplet("not-a-color")).toBe(hexToRgbTriplet(DEFAULT_ACCENT_HEX));
  });
});

describe("computeAccentTheme", () => {
  it("keeps the base accent as-is", () => {
    const theme = computeAccentTheme("#00eeff");
    expect(theme.accent).toBe("0 238 255");
  });

  it("picks a near-black foreground for a bright accent (Ciel's real cyan)", () => {
    const theme = computeAccentTheme("#00eeff");
    expect(theme.accentForeground).toBe("23 23 23");
  });

  it("picks a white foreground for a dark accent", () => {
    const theme = computeAccentTheme("#1a1a2e");
    expect(theme.accentForeground).toBe("255 255 255");
  });

  it("uses a dark foreground for the default accent (#96fff3 is light) and keeps it in sync with globals.css", () => {
    const theme = computeAccentTheme(DEFAULT_ACCENT_HEX);
    expect(theme.accent).toBe("150 255 243");
    expect(theme.accentSecondary).toBe("211 255 250");
    expect(theme.accentForeground).toBe("23 23 23");
  });

  it("lightens the secondary shade relative to the base", () => {
    const theme = computeAccentTheme(DEFAULT_ACCENT_HEX);
    const sum = (triplet: string) => triplet.split(" ").reduce((total, c) => total + Number(c), 0);
    expect(sum(theme.accentSecondary)).toBeGreaterThan(sum(theme.accent));
  });

  it("never lightens past white even for an already near-white accent", () => {
    const theme = computeAccentTheme("#fefefe");
    for (const channel of theme.accentSecondary.split(" ")) {
      expect(Number(channel)).toBeLessThanOrEqual(255);
    }
  });

  it("falls back to the default accent for invalid input", () => {
    expect(computeAccentTheme("nonsense")).toEqual(computeAccentTheme(DEFAULT_ACCENT_HEX));
  });
});
