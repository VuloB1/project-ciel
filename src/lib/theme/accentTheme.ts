// Convierte el `accentColor` de un perfil (un hex arbitrario elegido por
// el usuario) en las variables CSS que ya usa toda la app —
// `tailwind.config.js` resuelve `bg-accent`/`text-accent`/`ring-accent`/etc.
// a `rgb(var(--color-accent) / <alpha-value>)`, así que setear esa
// variable alcanza para que todo lo ya cableado (botones, switches,
// sliders, foco, links) siga el color del personaje sin tocar esos
// componentes uno por uno. Ver docs/ADR/008-theme-engine.md.

import { blend, contrastRatio } from "./contrast";

/** Acento de fábrica de la app (#96fff3): el que ve cualquiera que abra una
 * instalación limpia, antes de elegir un tema o un color propio. Debe coincidir
 * con `--color-accent` en `globals.css` y `DEFAULT_THEME_TOKENS`. Única fuente
 * para `ProfileAvatarBadge.tsx`, `IdentityEditor.tsx` y el onboarding. */
export const DEFAULT_ACCENT_HEX = "#96FFF3";

export interface AccentTheme {
  accent: string;
  accentSecondary: string;
  accentForeground: string;
}

interface Rgb {
  r: number;
  g: number;
  b: number;
}

function parseHex(hex: string): Rgb | null {
  const cleaned = hex.trim().replace(/^#/, "");
  const expanded =
    cleaned.length === 3
      ? cleaned
          .split("")
          .map((c) => c + c)
          .join("")
      : cleaned;
  if (!/^[0-9a-fA-F]{6}$/.test(expanded)) return null;

  return {
    r: parseInt(expanded.slice(0, 2), 16),
    g: parseInt(expanded.slice(2, 4), 16),
    b: parseInt(expanded.slice(4, 6), 16),
  };
}

function rgbTriplet({ r, g, b }: Rgb): string {
  return `${r} ${g} ${b}`;
}

/** `"#00eeff"` -> `"0 238 255"` — el formato espacio-separado que ya usa
 * toda `globals.css` (para poder combinarse con `<alpha-value>` de
 * Tailwind). Cae al violeta por defecto si `hex` no es un color válido —
 * nunca deja las variables CSS en un estado roto. */
export function hexToRgbTriplet(hex: string): string {
  const rgb = parseHex(hex) ?? parseHex(DEFAULT_ACCENT_HEX);
  return rgbTriplet(rgb as Rgb);
}

function rgbToHsl({ r, g, b }: Rgb): { h: number; s: number; l: number } {
  const rn = r / 255;
  const gn = g / 255;
  const bn = b / 255;
  const max = Math.max(rn, gn, bn);
  const min = Math.min(rn, gn, bn);
  const l = (max + min) / 2;

  if (max === min) return { h: 0, s: 0, l };

  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  let h: number;
  switch (max) {
    case rn:
      h = (gn - bn) / d + (gn < bn ? 6 : 0);
      break;
    case gn:
      h = (bn - rn) / d + 2;
      break;
    default:
      h = (rn - gn) / d + 4;
  }
  return { h: h / 6, s, l };
}

function hueToRgb(p: number, q: number, t: number): number {
  let tn = t;
  if (tn < 0) tn += 1;
  if (tn > 1) tn -= 1;
  if (tn < 1 / 6) return p + (q - p) * 6 * tn;
  if (tn < 1 / 2) return q;
  if (tn < 2 / 3) return p + (q - p) * (2 / 3 - tn) * 6;
  return p;
}

function hslToRgb(h: number, s: number, l: number): Rgb {
  if (s === 0) {
    const v = Math.round(l * 255);
    return { r: v, g: v, b: v };
  }
  const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
  const p = 2 * l - q;
  return {
    r: Math.round(hueToRgb(p, q, h + 1 / 3) * 255),
    g: Math.round(hueToRgb(p, q, h) * 255),
    b: Math.round(hueToRgb(p, q, h - 1 / 3) * 255),
  };
}

/** Aclara en HSL en vez de mezclar con blanco en RGB — preserva el matiz
 * del color elegido en vez de "lavarlo" hacia gris. Recorta a 0.97 de
 * luminosidad (no 1.0 exacto) para que un color ya casi blanco no termine
 * indistinguible del fondo en el estado hover. */
function lighten(rgb: Rgb, amount: number): Rgb {
  const hsl = rgbToHsl(rgb);
  return hslToRgb(hsl.h, hsl.s, Math.min(hsl.l + amount, 0.97));
}

/** Brillo percibido (fórmula YIQ, 0-255) — más simple que luminancia
 * relativa WCAG y, a diferencia de esa, no le cambia el veredicto al
 * violeta por defecto que ya se usaba con texto blanco en toda la app
 * (WCAG por contraste puro daba casi-negro para ese mismo violeta,
 * "corrigiendo" algo que nunca estuvo roto). Este es el criterio estándar
 * que usan la mayoría de las librerías de "elegí texto legible". */
function perceivedBrightness({ r, g, b }: Rgb): number {
  return (r * 299 + g * 587 + b * 114) / 1000;
}

/** Blanco o casi-negro (`23 23 23`, no `0 0 0` puro) según qué tan claro
 * se percibe `rgb` — un acento claro (ej. el cyan real de un perfil de
 * esta sesión, `#00eeff`) deja `text-white` ilegible encima si no se
 * elige el opuesto. */
function pickForeground(rgb: Rgb): Rgb {
  return perceivedBrightness(rgb) >= 128 ? { r: 23, g: 23, b: 23 } : { r: 255, g: 255, b: 255 };
}

function parseRgbTriplet(triplet: string): Rgb {
  const [r = 0, g = 0, b = 0] = triplet.trim().split(/\s+/).map(Number);
  return { r, g, b };
}

/** `true` si un valor "R G B" espacio-separado (el formato que usa
 * `ThemeTokens`, ej. `tokens.background`) ya se percibe como claro —
 * reusa la misma fórmula YIQ que `pickForeground` en vez de hardcodear
 * qué presets son "los claros" (hoy solo `paper`, pero no debería hacer
 * falta tocar este archivo si se agrega un preset claro más adelante). */
export function isRgbTripletLight(triplet: string): boolean {
  return perceivedBrightness(parseRgbTriplet(triplet)) >= 128;
}

export function computeAccentTheme(hex: string): AccentTheme {
  const rgb = parseHex(hex) ?? (parseHex(DEFAULT_ACCENT_HEX) as Rgb);
  return {
    accent: rgbTriplet(rgb),
    accentSecondary: rgbTriplet(lighten(rgb, 0.12)),
    accentForeground: rgbTriplet(pickForeground(rgb)),
  };
}

/** Contraste mínimo WCAG AA para texto normal. */
export const MIN_TEXT_CONTRAST = 4.5;
/** Opacidad del fondo translúcido de insignias y chips (`bg-x/10`). */
export const TINT_ALPHA = 0.1;

function tripletToRgb(triplet: string): Rgb {
  return parseRgbTriplet(triplet);
}

/** Mueve un color en luminosidad HSL (conservando su matiz) en la dirección
 * indicada hasta que tenga al menos `MIN_TEXT_CONTRAST` sobre todos los
 * `backgrounds`. Si ya cumple, lo devuelve intacto. `direction` es `1` para
 * aclarar (fondos oscuros) o `-1` para oscurecer (fondos claros). */
function ensureContrast(triplet: string, backgrounds: string[], direction: 1 | -1): string {
  // También sobre su propio fondo translúcido (`bg-accent/10 text-accent`, chips
  // e insignias): ahí el contraste es menor que sobre la superficie sola.
  const passes = (candidate: string) =>
    backgrounds.every(
      (bg) => contrastRatio(candidate, bg) >= MIN_TEXT_CONTRAST && contrastRatio(candidate, blend(candidate, bg, TINT_ALPHA)) >= MIN_TEXT_CONTRAST,
    );
  if (passes(triplet)) return triplet;

  const base = rgbToHsl(tripletToRgb(triplet));
  for (let step = 1; step <= 100; step += 1) {
    const l = Math.min(1, Math.max(0, base.l + direction * step * 0.01));
    const candidate = rgbTriplet(hslToRgb(base.h, base.s, l));
    if (passes(candidate)) return candidate;
  }
  return rgbTriplet(hslToRgb(base.h, base.s, direction === 1 ? 1 : 0));
}

/** Opacidad del texto atenuado sobre el acento (`text-accent-foreground/85`:
 * la hora de las burbujas, los botones de copiar). */
export const MUTED_FOREGROUND_ALPHA = 0.85;

function foregroundPasses(foreground: string, background: string): boolean {
  return (
    contrastRatio(foreground, background) >= MIN_TEXT_CONTRAST &&
    contrastRatio(blend(foreground, background, MUTED_FOREGROUND_ALPHA), background) >= MIN_TEXT_CONTRAST
  );
}

/** Texto que va sobre el acento, y el acento ajustado si hace falta. Se conserva
 * el texto elegido (blanco o casi negro por brillo percibido, ver
 * `pickForeground`) mientras se lea, también atenuado; si no, se prueba con
 * casi negro y con blanco. Si un acento de tono medio no da con ninguno, se
 * aclara (fondo oscuro) u oscurece (fondo claro) un poco más, en la misma
 * dirección que ya usa `ensureContrast`, hasta que alguno sirva. */
function readableForeground(current: string, accent: string, direction: 1 | -1): { accent: string; foreground: string } {
  const candidates = [current, "23 23 23", "255 255 255"];
  const base = rgbToHsl(tripletToRgb(accent));
  for (let step = 0; step <= 100; step += 1) {
    const l = Math.min(1, Math.max(0, base.l + direction * step * 0.01));
    const candidate = step === 0 ? accent : rgbTriplet(hslToRgb(base.h, base.s, l));
    const foreground = candidates.find((fg) => foregroundPasses(fg, candidate));
    if (foreground) return { accent: candidate, foreground };
  }
  return { accent, foreground: current };
}

/** Garantiza que el acento (y el acento secundario) se puedan leer como texto
 * sobre el fondo y las dos superficies, y que el texto de los botones de acento
 * se lea sobre ellos. Un acento que ya cumple no cambia: solo se ajusta el que
 * no llega a 4,5:1 — sobre todo el acento de un tema oscuro en modo Claro, o un
 * color propio muy claro u oscuro. */
export function withReadableAccent<T extends AccentTheme & { background: string; surface: string; surfaceSecondary: string }>(tokens: T): T {
  const backgrounds = [tokens.background, tokens.surface, tokens.surfaceSecondary];
  const direction = isRgbTripletLight(tokens.background) ? -1 : 1;
  const { accent, foreground: accentForeground } = readableForeground(
    tokens.accentForeground,
    ensureContrast(tokens.accent, backgrounds, direction),
    direction,
  );
  const accentSecondary = ensureContrast(tokens.accentSecondary, backgrounds, direction);
  return { ...tokens, accent, accentSecondary, accentForeground };
}
