import { useEffect, useState } from "react";

import { useProfileStore } from "@/stores/profileStore";
import { useSettingsStore } from "@/stores/settingsStore";

import { computeAccentTheme, isRgbTripletLight, withReadableAccent } from "./accentTheme";
import { controlBorderColor } from "./contrast";
import {
  CANONICAL_RADIUS,
  DEFAULT_THEME_TOKENS,
  LIGHT_NEUTRAL_RAMP,
  STATUS_COLORS,
  THEME_PRESETS,
  type StatusColors,
  type ThemePresetId,
  type ThemeTokens,
} from "./presets";

const CSS_VAR: Record<keyof ThemeTokens, string> = {
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

function isKnownPreset(id: string): id is ThemePresetId {
  return id in THEME_PRESETS;
}

const supportsMatchMedia = typeof window !== "undefined" && typeof window.matchMedia === "function";

/** Resuelve "Oscuro"/"Claro"/"Sistema" (`AppearanceSettings.colorMode`) a
 * un modo real, siguiendo `prefers-color-scheme` del SO en caliente cuando
 * es "Sistema" — sin esto, cambiar el modo de Windows mientras la app está
 * abierta no se reflejaría hasta reiniciarla. `supportsMatchMedia` evita
 * romper en jsdom (los tests no implementan `matchMedia`) — "Sistema" ahí
 * simplemente cae a Oscuro. */
export function useResolvedColorMode(colorMode: "dark" | "light" | "system" | undefined): "dark" | "light" {
  const [systemPrefersDark, setSystemPrefersDark] = useState(
    () => supportsMatchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches,
  );

  useEffect(() => {
    if (!supportsMatchMedia) return;
    const query = window.matchMedia("(prefers-color-scheme: dark)");
    const handleChange = (event: MediaQueryListEvent) => setSystemPrefersDark(event.matches);
    query.addEventListener("change", handleChange);
    return () => query.removeEventListener("change", handleChange);
  }, []);

  if (colorMode === "light") return "light";
  if (colorMode === "system") return systemPrefersDark ? "dark" : "light";
  return "dark";
}

/** Escala un radio tipo `"10px"` por un porcentaje — redondeado a enteros,
 * nunca negativo. */
export function scaleRadius(px: string, percent: number): string {
  const value = Number.parseFloat(px);
  if (Number.isNaN(value)) return px;
  return `${Math.max(0, Math.round(value * (percent / 100)))}px`;
}

/** Aplica (o no) el override global de redondez sobre los radios de un
 * tema — `roundness == null` ("nunca lo tocó") deja los 3 radios del preset
 * tal cual, así un tema de esquinas cuadradas (Terminal/Noir/Cyberpunk/
 * Industrial) se sigue viendo cuadrado por defecto. En cuanto hay un
 * porcentaje explícito, los 3 radios se recalculan desde `CANONICAL_RADIUS`
 * (una base fija, IGUAL para los 15 presets) en vez de escalar el radio
 * propio del tema — si se escalara el propio, un preset en 0px se quedaría
 * en 0px sin importar el porcentaje, que es justo el caso que este override
 * existe para desbloquear. */
export function applyRoundness(tokens: ThemeTokens, roundness: number | null | undefined): ThemeTokens {
  if (roundness == null) return tokens;
  return {
    ...tokens,
    radiusSm: scaleRadius(CANONICAL_RADIUS.radiusSm, roundness),
    radiusMd: scaleRadius(CANONICAL_RADIUS.radiusMd, roundness),
    radiusLg: scaleRadius(CANONICAL_RADIUS.radiusLg, roundness),
  };
}

/** La paleta final que se escribe en las variables CSS, como función pura (así
 * la auditoría de accesibilidad prueba exactamente lo que ve el usuario):
 * preset (o defaults) → acento propio → neutros del modo Claro → acento legible
 * sobre esos neutros (`withReadableAccent`) → redondez; más los colores de
 * estado del modo que quedó. */
export function resolveThemeTokens(input: {
  theme: string | null;
  accentColor: string | null;
  mode: "dark" | "light";
  roundness?: number | null;
}): { tokens: ThemeTokens; status: StatusColors; controlBorder: string } {
  const baseTokens: ThemeTokens = input.theme && isKnownPreset(input.theme) ? THEME_PRESETS[input.theme] : DEFAULT_THEME_TOKENS;
  let tokens: ThemeTokens = input.accentColor ? { ...baseTokens, ...computeAccentTheme(input.accentColor) } : baseTokens;

  if (input.mode === "light" && !isRgbTripletLight(baseTokens.background)) {
    tokens = { ...tokens, ...LIGHT_NEUTRAL_RAMP };
  }

  tokens = withReadableAccent(tokens);
  tokens = applyRoundness(tokens, input.roundness);

  return {
    tokens,
    status: isRgbTripletLight(tokens.background) ? STATUS_COLORS.light : STATUS_COLORS.dark,
    controlBorder: controlBorderColor(tokens),
  };
}

/** Alimenta las 12 variables CSS de paleta+radios (ADR-009) con el `theme`
 * del perfil activo — todo lo que ya usa esas variables (fondo, superficies,
 * bordes, texto, acento, radios de Tailwind) sigue el tema del personaje
 * sin tocar componentes uno por uno.
 *
 * `accentColor` es un override independiente de `theme`, no un modo aparte:
 * un `theme` no nulo aporta la paleta base (o los defaults de ADR-008 si es
 * `null`), y `accentColor` — cuando el usuario lo eligió explícitamente vía
 * "Personalizado" — pisa solo `accent`/`accentSecondary`/`accentForeground`
 * encima de esa base. Así se puede tener, por ejemplo, la paleta de Forest
 * con un acento propio, en vez de que elegir un color de acento fuerce a
 * volver al tema por defecto.
 *
 * Un `theme` no reconocido (preset renombrado/eliminado a futuro) cae en
 * los defaults de ADR-008, igual que `theme: null`.
 *
 * Punto crítico: el efecto SIEMPRE escribe las 12 variables, sea cual sea
 * la rama — las variables CSS persisten en `documentElement` si no se
 * pisan, así que cambiar a un perfil sin tema ni acento propio debe
 * resetear fondo/superficie/borde/radios/acento, no dejarlos pegados.
 *
 * `AppearanceSettings.colorMode`/`roundness` (globales de la app, no del
 * perfil) se aplican encima de la base del preset: Oscuro no toca nada
 * (los 15 presets siguen exactamente como siempre); Claro sustituye los 6
 * neutros por `LIGHT_NEUTRAL_RAMP` salvo que el preset ya sea claro por sí
 * mismo; la redondez se resuelve vía `applyRoundness` — mientras el usuario
 * no la haya tocado (`roundness: null`), cada tema mantiene su propio radio
 * sin cambios; en cuanto elige un porcentaje, ESE valor se aplica sin
 * importar qué tema elija después. Ver el plan de la sección "Modo
 * claro/oscuro" para por qué no se repintó cada preset a mano. */
export function useActiveProfileTheme(): void {
  const profiles = useProfileStore((s) => s.profiles);
  const activeProfileId = useProfileStore((s) => s.activeProfileId);
  const active = profiles.find((p) => p.id === activeProfileId) ?? null;
  const theme = active?.theme ?? null;
  const accentColor = active?.accentColor ?? null;

  const appearance = useSettingsStore((s) => s.appearance);
  const resolvedMode = useResolvedColorMode(appearance?.colorMode);
  const roundness = appearance?.roundness;

  useEffect(() => {
    const { tokens, status, controlBorder } = resolveThemeTokens({ theme, accentColor, mode: resolvedMode, roundness });

    const root = document.documentElement.style;
    (Object.keys(CSS_VAR) as (keyof ThemeTokens)[]).forEach((key) => {
      root.setProperty(CSS_VAR[key], tokens[key]);
    });
    root.setProperty("--color-success", status.success);
    root.setProperty("--color-warning", status.warning);
    root.setProperty("--color-error", status.error);
    root.setProperty("--color-error-foreground", status.errorForeground);
    root.setProperty("--color-control-border", controlBorder);
  }, [theme, accentColor, resolvedMode, roundness]);
}
