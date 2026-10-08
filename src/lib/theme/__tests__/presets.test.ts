import { describe, expect, it } from "vitest";

import { DEFAULT_THEME_TOKENS, THEME_PRESET_IDS, THEME_PRESETS, type ThemeTokens } from "../presets";

const TOKEN_KEYS: (keyof ThemeTokens)[] = [
  "background",
  "surface",
  "surfaceSecondary",
  "border",
  "textPrimary",
  "textSecondary",
  "accent",
  "accentSecondary",
  "accentForeground",
  "radiusSm",
  "radiusMd",
  "radiusLg",
];

const COLOR_KEYS: (keyof ThemeTokens)[] = [
  "background",
  "surface",
  "surfaceSecondary",
  "border",
  "textPrimary",
  "textSecondary",
  "accent",
  "accentSecondary",
  "accentForeground",
];

const RADIUS_KEYS: (keyof ThemeTokens)[] = ["radiusSm", "radiusMd", "radiusLg"];

describe("THEME_PRESET_IDS", () => {
  it("has exactly 15 entries", () => {
    expect(THEME_PRESET_IDS).toHaveLength(15);
  });

  it("has no duplicate ids", () => {
    expect(new Set(THEME_PRESET_IDS).size).toBe(THEME_PRESET_IDS.length);
  });
});

describe("THEME_PRESETS", () => {
  it("has one entry per id in THEME_PRESET_IDS, no more, no less", () => {
    expect(Object.keys(THEME_PRESETS).sort()).toEqual([...THEME_PRESET_IDS].sort());
  });

  it.each(THEME_PRESET_IDS)("%s defines every ThemeTokens key as a non-empty string", (id) => {
    const tokens = THEME_PRESETS[id];
    for (const key of TOKEN_KEYS) {
      expect(typeof tokens[key]).toBe("string");
      expect(tokens[key].length).toBeGreaterThan(0);
    }
  });

  it.each(THEME_PRESET_IDS)("%s colors match the space-separated 'R G B' format", (id) => {
    const tokens = THEME_PRESETS[id];
    for (const key of COLOR_KEYS) {
      expect(tokens[key]).toMatch(/^\d{1,3} \d{1,3} \d{1,3}$/);
      for (const channel of tokens[key].split(" ")) {
        expect(Number(channel)).toBeGreaterThanOrEqual(0);
        expect(Number(channel)).toBeLessThanOrEqual(255);
      }
    }
  });

  it.each(THEME_PRESET_IDS)("%s radii match the '<number>px' format", (id) => {
    const tokens = THEME_PRESETS[id];
    for (const key of RADIUS_KEYS) {
      expect(tokens[key]).toMatch(/^\d+(\.\d+)?px$/);
    }
  });
});

describe("DEFAULT_THEME_TOKENS", () => {
  it("mirrors globals.css's :root defaults", () => {
    expect(DEFAULT_THEME_TOKENS).toEqual({
      background: "13 13 17",
      surface: "19 19 24",
      surfaceSecondary: "27 27 34",
      border: "38 38 48",
      textPrimary: "237 237 241",
      textSecondary: "156 156 168",
      accent: "150 255 243",
      accentSecondary: "211 255 250",
      accentForeground: "23 23 23",
      radiusSm: "6px",
      radiusMd: "10px",
      radiusLg: "16px",
    });
  });
});
