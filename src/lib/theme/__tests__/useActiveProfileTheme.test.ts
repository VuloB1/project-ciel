import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import { DEFAULT_IDENTITY, DEFAULT_PERSONALITY, DEFAULT_VOICE, type Profile } from "@/types/profile";

import { useProfileStore } from "@/stores/profileStore";
import { useSettingsStore } from "@/stores/settingsStore";

import { computeAccentTheme } from "../accentTheme";
import { CANONICAL_RADIUS, DEFAULT_THEME_TOKENS, THEME_PRESETS } from "../presets";
import { useActiveProfileTheme } from "../useActiveProfileTheme";

function makeProfile(overrides: Partial<Profile> = {}): Profile {
  return {
    id: "p1",
    name: "Ciel",
    nickname: null,
    avatarPath: null,
    description: null,
    ageFictional: null,
    pronouns: null,
    occupation: null,
    species: null,
    accentColor: null,
    theme: null,
    isArchived: false,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    identity: DEFAULT_IDENTITY,
    personality: DEFAULT_PERSONALITY,
    voice: DEFAULT_VOICE,
    avatarIdlePath: null,
    avatarThinkingPath: null,
    avatarTalkingPath: null,
    ...overrides,
  };
}

const CSS_VAR: Record<string, string> = {
  background: "--color-background",
  surface: "--color-surface",
  surfaceSecondary: "--color-surface-secondary",
  border: "--color-border",
  textPrimary: "--color-text-primary",
  textSecondary: "--color-text-secondary",
  accent: "--color-accent",
  accentSecondary: "--color-accent-secondary",
  accentForeground: "--color-accent-foreground",
  radiusSm: "--radius-sm",
  radiusMd: "--radius-md",
  radiusLg: "--radius-lg",
};

function readAllVars(): Record<string, string> {
  const style = document.documentElement.style;
  const result: Record<string, string> = {};
  for (const [key, cssVar] of Object.entries(CSS_VAR)) {
    result[key] = style.getPropertyValue(cssVar).trim();
  }
  return result;
}

function setActiveProfile(profile: Profile) {
  useProfileStore.setState({ profiles: [profile], activeProfileId: profile.id, isLoading: false, error: null });
}

describe("useActiveProfileTheme", () => {
  beforeEach(() => {
    document.documentElement.removeAttribute("style");
    useProfileStore.setState({ profiles: [], activeProfileId: null, isLoading: false, error: null });
    useSettingsStore.setState({ appearance: null });
  });

  it("falls back to DEFAULT_THEME_TOKENS for a profile with no theme and no accentColor", () => {
    setActiveProfile(makeProfile());
    renderHook(() => useActiveProfileTheme());

    expect(readAllVars()).toEqual(DEFAULT_THEME_TOKENS);
  });

  it("with no theme but a custom accentColor, only the 3 accent-* vars change — everything else stays default", () => {
    setActiveProfile(makeProfile({ accentColor: "#00eeff" }));
    renderHook(() => useActiveProfileTheme());

    const vars = readAllVars();
    expect(vars.accent).toBe("0 238 255");
    expect(vars.background).toBe(DEFAULT_THEME_TOKENS.background);
    expect(vars.surface).toBe(DEFAULT_THEME_TOKENS.surface);
    expect(vars.border).toBe(DEFAULT_THEME_TOKENS.border);
    expect(vars.radiusMd).toBe(DEFAULT_THEME_TOKENS.radiusMd);
  });

  it("a non-null theme with no accentColor override applies the full preset as-is", () => {
    setActiveProfile(makeProfile({ theme: "cyberpunk", accentColor: null }));
    renderHook(() => useActiveProfileTheme());

    expect(readAllVars()).toEqual(THEME_PRESETS.cyberpunk);
  });

  it("accentColor is an override independent of theme — it replaces just the accent tokens on top of the preset, keeping the rest", () => {
    setActiveProfile(makeProfile({ theme: "cyberpunk", accentColor: "#00eeff" }));
    renderHook(() => useActiveProfileTheme());

    const vars = readAllVars();
    const overriddenAccent = computeAccentTheme("#00eeff");
    expect(vars.accent).toBe(overriddenAccent.accent);
    expect(vars.accentSecondary).toBe(overriddenAccent.accentSecondary);
    expect(vars.accentForeground).toBe(overriddenAccent.accentForeground);
    // Everything else still comes from the preset, untouched.
    expect(vars.background).toBe(THEME_PRESETS.cyberpunk.background);
    expect(vars.surface).toBe(THEME_PRESETS.cyberpunk.surface);
    expect(vars.border).toBe(THEME_PRESETS.cyberpunk.border);
    expect(vars.radiusMd).toBe(THEME_PRESETS.cyberpunk.radiusMd);
  });

  it("an unknown/stale theme id falls back to the null-theme behavior instead of crashing", () => {
    setActiveProfile(makeProfile({ theme: "a-preset-that-no-longer-exists" }));
    renderHook(() => useActiveProfileTheme());

    expect(readAllVars()).toEqual(DEFAULT_THEME_TOKENS);
  });

  it("switching the active profile from a themed one to a null-themed one resets every var", () => {
    setActiveProfile(makeProfile({ theme: "paper" }));
    const { rerender } = renderHook(() => useActiveProfileTheme());
    expect(readAllVars()).toEqual(THEME_PRESETS.paper);

    act(() => {
      setActiveProfile(makeProfile({ theme: null }));
      rerender();
    });

    expect(readAllVars()).toEqual(DEFAULT_THEME_TOKENS);
  });

  it("roundness untouched (null) keeps a zero-radius theme (cyberpunk) fully square", () => {
    useSettingsStore.setState({
      appearance: { colorMode: "dark", roundness: null, density: "normal", fontId: "inter", fontScale: 100 },
    });
    setActiveProfile(makeProfile({ theme: "cyberpunk" }));
    renderHook(() => useActiveProfileTheme());

    expect(readAllVars().radiusMd).toBe(THEME_PRESETS.cyberpunk.radiusMd);
    expect(readAllVars().radiusMd).toBe("0px");
  });

  it("a customized roundness overrides even a zero-radius theme with the same absolute radius", () => {
    useSettingsStore.setState({
      appearance: { colorMode: "dark", roundness: 100, density: "normal", fontId: "inter", fontScale: 100 },
    });
    setActiveProfile(makeProfile({ theme: "cyberpunk" }));
    renderHook(() => useActiveProfileTheme());

    expect(readAllVars().radiusMd).toBe(CANONICAL_RADIUS.radiusMd);
    expect(readAllVars().radiusMd).not.toBe("0px");
  });

  it("a customized roundness applies the same radius no matter which theme is active", () => {
    useSettingsStore.setState({
      appearance: { colorMode: "dark", roundness: 50, density: "normal", fontId: "inter", fontScale: 100 },
    });
    setActiveProfile(makeProfile({ theme: "pastel" }));
    const { rerender } = renderHook(() => useActiveProfileTheme());
    const pastelRadius = readAllVars().radiusMd;

    act(() => {
      setActiveProfile(makeProfile({ theme: "terminal" }));
      rerender();
    });

    expect(readAllVars().radiusMd).toBe(pastelRadius);
  });
});
