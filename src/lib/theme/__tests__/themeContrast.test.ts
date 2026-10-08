import { describe, expect, it } from "vitest";

import { TINT_ALPHA } from "../accentTheme";
import { blend, contrastRatio, MIN_CONTROL_CONTRAST } from "../contrast";
import { THEME_PRESET_IDS, THEME_PRESETS } from "../presets";
import { resolveThemeTokens } from "../useActiveProfileTheme";

// Auditoría de accesibilidad de los 15 temas (ADR-018): WCAG 2.1 AA, 4,5:1 para
// texto normal, sobre la paleta FINAL que ve el usuario (`resolveThemeTokens`),
// en modo Oscuro y en modo Claro, y con acentos propios de cualquier tono.
// Los bordes decorativos (separadores de tarjetas) no entran: WCAG 1.4.11 exime
// a los elementos que no son necesarios para entender la interfaz.

const MIN = 4.5;
/** Opacidad del fondo translúcido de las insignias y chips (`bg-x/10`). */
const TINT = TINT_ALPHA;

const CUSTOM_ACCENTS = ["#96FFF3", "#FFFFFF", "#000000", "#0000FF", "#FFFF00", "#FF0000", "#00FF00", "#808080", "#7A3B4F", "#FF9ECF"];

type Case = { name: string; theme: string | null; accentColor: string | null; mode: "dark" | "light" };

const CASES: Case[] = [
  ...THEME_PRESET_IDS.flatMap((theme) => (["dark", "light"] as const).map((mode) => ({ name: `${theme} (${mode})`, theme, accentColor: null, mode }))),
  ...(["dark", "light"] as const).flatMap((mode) => [{ name: `predeterminado (${mode})`, theme: null, accentColor: null, mode }]),
  ...CUSTOM_ACCENTS.flatMap((accent) =>
    (["dark", "light"] as const).flatMap((mode) => [
      { name: `predeterminado + ${accent} (${mode})`, theme: null, accentColor: accent, mode },
      { name: `forest + ${accent} (${mode})`, theme: "forest", accentColor: accent, mode },
    ]),
  ),
];

function failures(c: Case): string[] {
  const { tokens: t, status, controlBorder } = resolveThemeTokens({ theme: c.theme, accentColor: c.accentColor, mode: c.mode });
  const surfaces = { fondo: t.background, superficie: t.surface, "superficie 2": t.surfaceSecondary };
  const out: string[] = [];
  const check = (label: string, ratio: number) => {
    if (ratio < MIN) out.push(`${label}: ${ratio.toFixed(2)}`);
  };

  for (const [name, bg] of Object.entries(surfaces)) {
    check(`texto principal sobre ${name}`, contrastRatio(t.textPrimary, bg));
    check(`texto secundario sobre ${name}`, contrastRatio(t.textSecondary, bg));
    check(`acento como texto sobre ${name}`, contrastRatio(t.accent, bg));
    check(`acento secundario como texto sobre ${name}`, contrastRatio(t.accentSecondary, bg));
    check(`acento sobre su fondo translúcido en ${name}`, contrastRatio(t.accent, blend(t.accent, bg, TINT)));
    for (const key of ["success", "warning", "error"] as const) {
      check(`${key} sobre ${name}`, contrastRatio(status[key], bg));
      check(`${key} sobre su fondo translúcido en ${name}`, contrastRatio(status[key], blend(status[key], bg, TINT)));
    }
  }
  for (const [name, bg] of Object.entries(surfaces)) {
    const ratio = contrastRatio(controlBorder, bg);
    if (ratio < MIN_CONTROL_CONTRAST) out.push(`borde de controles sobre ${name}: ${ratio.toFixed(2)}`);
  }
  check("texto de botón sobre el acento", contrastRatio(t.accentForeground, t.accent));
  check("texto de botón sobre el acento secundario (hover)", contrastRatio(t.accentForeground, t.accentSecondary));
  // Hora y botones de las burbujas del usuario: `text-accent-foreground/85` sobre el acento.
  check("texto atenuado (85 %) sobre el acento", contrastRatio(blend(t.accentForeground, t.accent, 0.85), t.accent));
  check("texto del botón de peligro", contrastRatio(status.errorForeground, status.error));
  return out;
}

describe("accesibilidad de los temas (WCAG AA)", () => {
  it.each(CASES)("$name cumple 4,5:1 en todo el texto", (c) => {
    expect(failures(c)).toEqual([]);
  });

  it("cada tema ya viene limpio en su propio modo: el ajuste en tiempo de ejecución no lo toca", () => {
    for (const id of THEME_PRESET_IDS) {
      const mode = id === "paper" ? "light" : "dark";
      const { tokens } = resolveThemeTokens({ theme: id, accentColor: null, mode });
      expect({ id, tokens }).toEqual({ id, tokens: THEME_PRESETS[id] });
    }
  });

  it("los 15 temas están cubiertos en los dos modos", () => {
    expect(THEME_PRESET_IDS).toHaveLength(15);
    for (const id of THEME_PRESET_IDS) {
      expect(CASES.some((c) => c.theme === id && c.mode === "dark")).toBe(true);
      expect(CASES.some((c) => c.theme === id && c.mode === "light")).toBe(true);
    }
  });
});

describe("contrastRatio", () => {
  it("da 21 para negro sobre blanco y 1 para colores iguales", () => {
    expect(contrastRatio("0 0 0", "255 255 255")).toBeCloseTo(21, 5);
    expect(contrastRatio("10 20 30", "10 20 30")).toBeCloseTo(1, 5);
  });

  it("es simétrico", () => {
    expect(contrastRatio("20 20 20", "200 200 200")).toBeCloseTo(contrastRatio("200 200 200", "20 20 20"), 10);
  });

  it("blend mezcla con la opacidad indicada", () => {
    expect(blend("255 255 255", "0 0 0", 0.5)).toBe("128 128 128");
    expect(blend("10 20 30", "200 200 200", 1)).toBe("10 20 30");
  });
});
