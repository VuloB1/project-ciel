// Contraste de color según WCAG 2.x (https://www.w3.org/TR/WCAG21/#dfn-contrast-ratio).
// Trabaja con los tripletes "R G B" espacio-separados que usan los temas
// (`ThemeTokens`). Lo usan la auditoría de accesibilidad de los 15 temas
// (`__tests__/themeContrast.test.ts`) y la elección del texto sobre el acento.

export type RgbTriplet = string;

export function parseTriplet(triplet: RgbTriplet): [number, number, number] {
  const [r = 0, g = 0, b = 0] = triplet.trim().split(/\s+/).map(Number);
  return [r, g, b];
}

function channelToLinear(value: number): number {
  const c = value / 255;
  return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

/** Luminancia relativa WCAG, de 0 (negro) a 1 (blanco). */
export function relativeLuminance(triplet: RgbTriplet): number {
  const [r, g, b] = parseTriplet(triplet);
  return 0.2126 * channelToLinear(r) + 0.7152 * channelToLinear(g) + 0.0722 * channelToLinear(b);
}

/** Razón de contraste entre dos colores, de 1 (iguales) a 21 (negro sobre blanco). */
export function contrastRatio(a: RgbTriplet, b: RgbTriplet): number {
  const la = relativeLuminance(a);
  const lb = relativeLuminance(b);
  const [light, dark] = la >= lb ? [la, lb] : [lb, la];
  return (light + 0.05) / (dark + 0.05);
}

/** Mezcla `top` sobre `bottom` con `alpha` (0 a 1): el color que realmente se
 * ve con `text-accent/50`, `bg-accent/10` y similares. */
export function blend(top: RgbTriplet, bottom: RgbTriplet, alpha: number): RgbTriplet {
  const t = parseTriplet(top);
  const b = parseTriplet(bottom);
  return t.map((channel, i) => Math.round(channel * alpha + (b[i] ?? 0) * (1 - alpha))).join(" ");
}

/** Contraste mínimo WCAG 1.4.11 para el borde que delimita un control (campo,
 * selector, interruptor) cuando es lo que lo distingue del fondo. */
export const MIN_CONTROL_CONTRAST = 3;

/** Borde de los controles de formulario: el borde decorativo del tema, acercado
 * al color de texto secundario lo justo para llegar a 3:1 sobre el fondo y las
 * dos superficies. Los separadores de tarjetas siguen con el borde suave del
 * tema (WCAG exime a lo decorativo); este es solo para lo que hay que ver para
 * saber dónde escribir o hacer clic. */
export function controlBorderColor(tokens: {
  border: RgbTriplet;
  textSecondary: RgbTriplet;
  background: RgbTriplet;
  surface: RgbTriplet;
  surfaceSecondary: RgbTriplet;
}): RgbTriplet {
  const surfaces = [tokens.background, tokens.surface, tokens.surfaceSecondary];
  const passes = (candidate: RgbTriplet) => surfaces.every((bg) => contrastRatio(candidate, bg) >= MIN_CONTROL_CONTRAST);
  if (passes(tokens.border)) return tokens.border;
  for (let percent = 1; percent <= 100; percent += 1) {
    const candidate = blend(tokens.textSecondary, tokens.border, percent / 100);
    if (passes(candidate)) return candidate;
  }
  return tokens.textSecondary;
}
