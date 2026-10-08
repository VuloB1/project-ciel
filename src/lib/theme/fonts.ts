// Catálogo de fuentes personalizables (sección "Apariencia" > "Preferencias
// de la app"). Todas son Google Fonts auto-hosteadas vía `@fontsource/*`
// (licencia OFL, empaquetadas localmente — nada de `<link>` a
// fonts.googleapis.com, sigue la misma filosofía "auto-hosteado" que el
// servidor del avatar para OBS) en vez de depender de que el SO las tenga
// instaladas o de una request externa cada vez que se abre la app.
//
// Solo se importan los pesos 400/500/600/700 — los únicos que usa la app
// hoy (`font-normal`/`font-medium`/`font-semibold`/`font-bold` de Tailwind).
import "@fontsource/inter/400.css";
import "@fontsource/inter/500.css";
import "@fontsource/inter/600.css";
import "@fontsource/inter/700.css";
import "@fontsource/poppins/400.css";
import "@fontsource/poppins/500.css";
import "@fontsource/poppins/600.css";
import "@fontsource/poppins/700.css";
import "@fontsource/montserrat/400.css";
import "@fontsource/montserrat/500.css";
import "@fontsource/montserrat/600.css";
import "@fontsource/montserrat/700.css";
import "@fontsource/nunito/400.css";
import "@fontsource/nunito/500.css";
import "@fontsource/nunito/600.css";
import "@fontsource/nunito/700.css";
import "@fontsource/raleway/400.css";
import "@fontsource/raleway/500.css";
import "@fontsource/raleway/600.css";
import "@fontsource/raleway/700.css";
import "@fontsource/quicksand/400.css";
import "@fontsource/quicksand/500.css";
import "@fontsource/quicksand/600.css";
import "@fontsource/quicksand/700.css";
import "@fontsource/space-grotesk/400.css";
import "@fontsource/space-grotesk/500.css";
import "@fontsource/space-grotesk/600.css";
import "@fontsource/space-grotesk/700.css";
import "@fontsource/comfortaa/400.css";
import "@fontsource/comfortaa/500.css";
import "@fontsource/comfortaa/600.css";
import "@fontsource/comfortaa/700.css";
import "@fontsource/merriweather/400.css";
import "@fontsource/merriweather/700.css";
import "@fontsource/lora/400.css";
import "@fontsource/lora/500.css";
import "@fontsource/lora/600.css";
import "@fontsource/lora/700.css";
import "@fontsource/playfair-display/400.css";
import "@fontsource/playfair-display/500.css";
import "@fontsource/playfair-display/600.css";
import "@fontsource/playfair-display/700.css";
import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/500.css";
import "@fontsource/jetbrains-mono/600.css";
import "@fontsource/jetbrains-mono/700.css";

export type FontCategory = "sans" | "serif" | "display" | "mono";

export interface FontOption {
  id: string;
  label: string;
  /** Nombre exacto que registra el `@font-face` de `@fontsource` — usado
   * tal cual como `font-family` (con fallback genérico agregado aparte). */
  cssFamily: string;
  category: FontCategory;
}

export const DEFAULT_FONT_ID = "inter";

export const FONT_OPTIONS: FontOption[] = [
  { id: "inter", label: "Inter", cssFamily: "Inter", category: "sans" },
  { id: "poppins", label: "Poppins", cssFamily: "Poppins", category: "sans" },
  { id: "montserrat", label: "Montserrat", cssFamily: "Montserrat", category: "sans" },
  { id: "nunito", label: "Nunito", cssFamily: "Nunito", category: "sans" },
  { id: "raleway", label: "Raleway", cssFamily: "Raleway", category: "sans" },
  { id: "quicksand", label: "Quicksand", cssFamily: "Quicksand", category: "sans" },
  { id: "space-grotesk", label: "Space Grotesk", cssFamily: "Space Grotesk", category: "sans" },
  { id: "comfortaa", label: "Comfortaa", cssFamily: "Comfortaa", category: "display" },
  { id: "playfair-display", label: "Playfair Display", cssFamily: "Playfair Display", category: "serif" },
  { id: "merriweather", label: "Merriweather", cssFamily: "Merriweather", category: "serif" },
  { id: "lora", label: "Lora", cssFamily: "Lora", category: "serif" },
  { id: "jetbrains-mono", label: "JetBrains Mono", cssFamily: "JetBrains Mono", category: "mono" },
];

const FONT_BY_ID: Record<string, FontOption> = Object.fromEntries(FONT_OPTIONS.map((f) => [f.id, f]));

/** `fontId` guardado en `AppearanceSettings` -> valor listo para
 * `--font-family-app` — SOLO el nombre de la familia (entre comillas si
 * tiene espacios), sin fallbacks: `tailwind.config.js`'s `fontFamily.sans`
 * y `body` en `globals.css` ya agregan la cadena de fallback genérica
 * después de esta variable, agregarla acá también la duplicaría. Cae a
 * Inter si el id no se reconoce (ej. una versión vieja de la app con un
 * catálogo distinto). */
export function resolveFontFamily(fontId: string): string {
  const option = FONT_BY_ID[fontId] ?? FONT_BY_ID[DEFAULT_FONT_ID]!;
  return option.cssFamily.includes(" ") ? `"${option.cssFamily}"` : option.cssFamily;
}
