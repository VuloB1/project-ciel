// Catálogo de los 15 temas visuales completos (ADR-009), extraídos de los
// mockups `.dc.html` ya aprobados por el usuario. Cada preset define toda
// la paleta + radios que `useActiveProfileTheme` escribe como variables
// CSS — el mismo mecanismo que el acento por perfil (ADR-008), pero para
// las 12 variables que `globals.css`/`tailwind.config.js` ya exponen.

export const THEME_PRESET_IDS = [
  "cozy",
  "cyberpunk",
  "minimal",
  "pastel",
  "luxury",
  "vaporwave",
  "forest",
  "industrial",
  "ocean",
  "sakura",
  "terminal",
  "noir",
  "autumn",
  "royal",
  "paper",
] as const;

export type ThemePresetId = (typeof THEME_PRESET_IDS)[number];

export interface ThemeTokens {
  background: string;
  surface: string;
  surfaceSecondary: string;
  border: string;
  textPrimary: string;
  textSecondary: string;
  accent: string;
  accentSecondary: string;
  accentForeground: string;
  radiusSm: string;
  radiusMd: string;
  radiusLg: string;
}

/** Mirror exacto de los defaults de `:root` en `globals.css` — fallback
 * para `theme: null` ("Personalizado"). Sus 3 campos `accent*` nunca se
 * usan tal cual (`useActiveProfileTheme` los pisa con
 * `computeAccentTheme`), quedan solo para que el tipo sea total. */
export const DEFAULT_THEME_TOKENS: ThemeTokens = {
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
};

/** Los 6 valores de neutro (fondo/superficie/borde/texto) que reemplazan
 * los propios del preset activo cuando el modo de color resuelto es Claro
 * (ver `useActiveProfileTheme.ts`) — compartida entre los 15 presets en vez
 * de repintar cada uno a mano (decisión del usuario, sección "Modo
 * claro/oscuro" del plan). El acento/acento-secundario/acento-foreground y
 * el radio del preset NO se tocan acá — modo Claro es "mi tema elegido,
 * sobre una superficie clara", no un tema nuevo. Un preset que ya es claro
 * por sí mismo (hoy solo `paper`) no pasa por esta rampa — ver
 * `isRgbTripletLight` en `accentTheme.ts`. */
export const LIGHT_NEUTRAL_RAMP: Pick<ThemeTokens, "background" | "surface" | "surfaceSecondary" | "border" | "textPrimary" | "textSecondary"> = {
  background: "250 250 252",
  surface: "255 255 255",
  surfaceSecondary: "241 241 245",
  border: "226 226 232",
  textPrimary: "23 23 28",
  textSecondary: "100 100 112",
};

/** Base fija para el override GLOBAL de redondez (`AppearanceSettings.roundness`,
 * cuando no es `null`) — a diferencia del radio propio de cada preset (que
 * varía de 0px, ej. Terminal/Noir/Cyberpunk/Industrial, a 14/18/22px, ej.
 * Pastel/Sakura), esta base es la MISMA para los 15 temas, para que "más
 * anguloso" o "más redondeado" se pueda pedir de verdad sin que un tema de
 * esquinas cuadradas se quede pegado en 0 sin importar el porcentaje. 100%
 * reproduce los defaults de ADR-008 (`DEFAULT_THEME_TOKENS`); ver
 * `applyRoundness` en `useActiveProfileTheme.ts`. */
export const CANONICAL_RADIUS: Pick<ThemeTokens, "radiusSm" | "radiusMd" | "radiusLg"> = {
  radiusSm: "6px",
  radiusMd: "10px",
  radiusLg: "16px",
};

export const THEME_PRESETS: Record<ThemePresetId, ThemeTokens> = {
  cozy: {
    background: "19 13 9",
    surface: "27 20 15",
    surfaceSecondary: "36 26 19",
    border: "58 42 30",
    textPrimary: "242 231 217",
    textSecondary: "180 154 133",
    accent: "232 149 95",
    accentSecondary: "240 172 128",
    accentForeground: "42 22 10",
    radiusSm: "10px",
    radiusMd: "14px",
    radiusLg: "18px",
  },
  cyberpunk: {
    background: "8 8 13",
    surface: "12 12 19",
    surfaceSecondary: "18 18 28",
    border: "26 26 36",
    textPrimary: "220 230 236",
    textSecondary: "101 131 146",
    accent: "5 242 242",
    accentSecondary: "107 251 251",
    accentForeground: "4 18 26",
    radiusSm: "0px",
    radiusMd: "0px",
    radiusLg: "0px",
  },
  minimal: {
    background: "14 14 15",
    surface: "17 17 18",
    surfaceSecondary: "29 30 33",
    border: "34 34 36",
    textPrimary: "228 228 230",
    textSecondary: "135 135 139",
    accent: "147 165 189",
    accentSecondary: "183 196 214",
    accentForeground: "23 23 23",
    radiusSm: "4px",
    radiusMd: "6px",
    radiusLg: "8px",
  },
  pastel: {
    background: "34 25 37",
    surface: "43 32 50",
    surfaceSecondary: "55 42 64",
    border: "74 58 84",
    textPrimary: "243 233 245",
    textSecondary: "182 160 194",
    accent: "255 158 207",
    accentSecondary: "143 227 196",
    accentForeground: "58 24 48",
    radiusSm: "14px",
    radiusMd: "18px",
    radiusLg: "22px",
  },
  luxury: {
    background: "13 18 16",
    surface: "16 22 19",
    surfaceSecondary: "22 31 26",
    border: "46 61 51",
    textPrimary: "237 230 214",
    textSecondary: "128 137 127",
    accent: "201 164 85",
    accentSecondary: "221 190 121",
    accentForeground: "16 22 18",
    radiusSm: "2px",
    radiusMd: "3px",
    radiusLg: "4px",
  },
  vaporwave: {
    background: "26 15 46",
    surface: "46 27 76",
    surfaceSecondary: "36 22 64",
    border: "74 53 112",
    textPrimary: "241 231 247",
    textSecondary: "183 159 203",
    accent: "255 95 162",
    accentSecondary: "77 238 234",
    accentForeground: "26 15 46",
    radiusSm: "10px",
    radiusMd: "13px",
    radiusLg: "16px",
  },
  forest: {
    background: "16 22 15",
    surface: "20 28 18",
    surfaceSecondary: "24 32 21",
    border: "52 67 44",
    textPrimary: "231 237 223",
    textSecondary: "142 156 129",
    accent: "143 187 104",
    accentSecondary: "217 164 65",
    accentForeground: "18 32 16",
    radiusSm: "10px",
    radiusMd: "13px",
    radiusLg: "16px",
  },
  industrial: {
    background: "25 25 25",
    surface: "29 29 29",
    surfaceSecondary: "34 34 34",
    border: "61 61 61",
    textPrimary: "237 237 237",
    textSecondary: "138 138 138",
    accent: "255 90 31",
    accentSecondary: "255 127 77",
    accentForeground: "26 10 0",
    radiusSm: "0px",
    radiusMd: "0px",
    radiusLg: "0px",
  },
  ocean: {
    background: "11 27 38",
    surface: "15 37 48",
    surfaceSecondary: "13 33 41",
    border: "29 61 72",
    textPrimary: "220 238 238",
    textSecondary: "110 147 154",
    accent: "47 182 176",
    accentSecondary: "127 217 168",
    accentForeground: "8 32 36",
    radiusSm: "10px",
    radiusMd: "13px",
    radiusLg: "16px",
  },
  sakura: {
    background: "36 21 34",
    surface: "51 30 45",
    surfaceSecondary: "43 24 38",
    border: "74 46 66",
    textPrimary: "243 228 236",
    textSecondary: "165 133 160",
    accent: "242 166 196",
    accentSecondary: "168 191 152",
    accentForeground: "58 26 44",
    radiusSm: "14px",
    radiusMd: "18px",
    radiusLg: "22px",
  },
  terminal: {
    background: "5 5 5",
    surface: "10 10 10",
    surfaceSecondary: "16 16 16",
    border: "27 59 34",
    textPrimary: "57 255 106",
    textSecondary: "31 143 63",
    accent: "57 255 106",
    accentSecondary: "156 255 184",
    accentForeground: "8 8 8",
    radiusSm: "0px",
    radiusMd: "0px",
    radiusLg: "0px",
  },
  noir: {
    background: "17 17 17",
    surface: "12 12 12",
    surfaceSecondary: "22 22 22",
    border: "38 38 38",
    textPrimary: "228 228 228",
    textSecondary: "129 129 129",
    accent: "228 228 228",
    accentSecondary: "154 154 154",
    accentForeground: "17 17 17",
    radiusSm: "0px",
    radiusMd: "0px",
    radiusLg: "0px",
  },
  autumn: {
    background: "28 20 13",
    surface: "32 22 13",
    surfaceSecondary: "28 19 11",
    border: "61 44 27",
    textPrimary: "237 224 208",
    textSecondary: "155 130 101",
    accent: "212 118 68",
    accentSecondary: "198 154 58",
    accentForeground: "36 18 6",
    radiusSm: "9px",
    radiusMd: "12px",
    radiusLg: "15px",
  },
  royal: {
    background: "15 11 34",
    surface: "23 17 58",
    surfaceSecondary: "23 15 58",
    border: "50 42 102",
    textPrimary: "232 228 247",
    textSecondary: "139 129 190",
    accent: "123 140 240",
    accentSecondary: "206 115 224",
    accentForeground: "23 15 58",
    radiusSm: "8px",
    radiusMd: "10px",
    radiusLg: "13px",
  },
  paper: {
    background: "246 241 231",
    surface: "239 232 217",
    surfaceSecondary: "255 255 255",
    border: "221 210 188",
    textPrimary: "43 42 61",
    textSecondary: "104 101 119",
    accent: "122 59 79",
    accentSecondary: "139 75 93",
    accentForeground: "251 243 245",
    radiusSm: "9px",
    radiusMd: "12px",
    radiusLg: "15px",
  },
};

/** Colores de estado (éxito/aviso/error) y el texto que va SOBRE el rojo de
 * error (botón "peligro"). Antes eran los mismos para todos los temas, tuneados
 * para fondo oscuro: sobre un fondo claro (modo Claro, o el tema Paper) el verde
 * y el ámbar quedaban en 1,7–2,2:1 y el rojo en 2,8–3,4:1. Cada juego se calculó
 * para llegar a 4,5:1 como texto sobre las superficies de TODOS los temas de su
 * modo, y también sobre su propio fondo translúcido (`bg-error/10` de los
 * Badge). El rojo oscuro es más claro que el de antes por esa misma razón, así
 * que su texto de botón es casi negro y no blanco. Ver ADR-018. */
export interface StatusColors {
  success: string;
  warning: string;
  error: string;
  errorForeground: string;
}

export const STATUS_COLORS: { dark: StatusColors; light: StatusColors } = {
  dark: { success: "52 199 89", warning: "255 159 10", error: "255 132 124", errorForeground: "23 23 23" },
  light: { success: "17 106 51", warning: "141 75 0", error: "179 28 28", errorForeground: "255 255 255" },
};
