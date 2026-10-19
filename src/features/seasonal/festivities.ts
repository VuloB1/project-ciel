/** Festive decorations: which festivity is "on" for a given day. Pure date logic, no UI. */

export const FESTIVITY_IDS = [
  "halloween",
  "muertos",
  "navidad",
  "anoNuevo",
  "reyes",
  "sanValentin",
  "sanPatricio",
  "pascua",
  "madre",
  "padre",
  "accionDeGracias",
] as const;
export type FestivityId = (typeof FESTIVITY_IDS)[number];

/** What the user picked in Configuración > Apariencia: nothing, follow the calendar, or one festivity forced. */
export type SeasonalMode = "off" | "auto" | FestivityId;

export type ParticleMotion = "fall" | "rise" | "cross";

/** Drawings of one festivity, as SVG strings (see scripts/generate-seasonal-art.mjs). */
export interface FestivityArt {
  motion: ParticleMotion;
  /** Garland, 680 units wide, meant to be tiled across the top. */
  top: string;
  /** Optional piece for the top-left corner, drawn once (a cobweb), not tiled with the garland. */
  topLeft?: string;
  /** Main piece for the bottom-right corner. */
  corner: string;
  /** Small piece for the sidebar. */
  side: string;
  /** Tiny version for the title bar. */
  icon: string;
  particles: string[];
}

const DAY_MS = 86_400_000;

/** A calendar day as a whole number, so windows compare without time-of-day or DST surprises. */
function dayNumber(year: number, month0: number, day: number): number {
  return Math.round(Date.UTC(year, month0, day) / DAY_MS);
}

/** Easter Sunday (Meeus/Jones/Butcher), as [month0, day]. */
function easterDate(year: number): [number, number] {
  const a = year % 19;
  const b = Math.floor(year / 100);
  const c = year % 100;
  const d = Math.floor(b / 4);
  const e = b % 4;
  const f = Math.floor((b + 8) / 25);
  const g = Math.floor((b - f + 1) / 3);
  const h = (19 * a + b - d - g + 15) % 30;
  const i = Math.floor(c / 4);
  const k = c % 4;
  const l = (32 + 2 * e + 2 * i - h - k) % 7;
  const m = Math.floor((a + 11 * h + 22 * l) / 451);
  const month = Math.floor((h + l - 7 * m + 114) / 31);
  const day = ((h + l - 7 * m + 114) % 31) + 1;
  return [month - 1, day];
}

/** The `n`-th (1-based) given weekday (0 = Sunday) of a month. */
function nthWeekday(year: number, month0: number, weekday: number, n: number): number {
  const first = new Date(Date.UTC(year, month0, 1)).getUTCDay();
  return dayNumber(year, month0, 1 + ((weekday - first + 7) % 7) + (n - 1) * 7);
}

interface Festivity {
  id: FestivityId;
  /** The day itself, for a year and a region (upper-case country code, may be empty). */
  date: (year: number, region: string) => number;
  /** Days the decoration shows before and after the day. */
  before: number;
  after: number;
  /** When set, "automatic" only turns this on in these regions (it can still be forced by hand). */
  onlyIn?: string[];
}

/** Listed in priority order: when two windows are equally close to their day, the first wins. */
const FESTIVITIES: Festivity[] = [
  { id: "halloween", date: (y) => dayNumber(y, 9, 31), before: 11, after: 1 },
  { id: "muertos", date: (y) => dayNumber(y, 10, 2), before: 5, after: 1 },
  { id: "navidad", date: (y) => dayNumber(y, 11, 25), before: 24, after: 1 },
  { id: "anoNuevo", date: (y) => dayNumber(y, 0, 1), before: 6, after: 1 },
  { id: "reyes", date: (y) => dayNumber(y, 0, 6), before: 3, after: 0 },
  { id: "sanValentin", date: (y) => dayNumber(y, 1, 14), before: 7, after: 1 },
  { id: "sanPatricio", date: (y) => dayNumber(y, 2, 17), before: 5, after: 0 },
  {
    id: "pascua",
    date: (y) => {
      const [month, day] = easterDate(y);
      return dayNumber(y, month, day);
    },
    before: 7,
    after: 1,
  },
  {
    id: "madre",
    // Mother's Day moves with the country: first Sunday of May in Spain and Portugal, the 10th in Mexico,
    // third Sunday of October in Argentina, second Sunday of May almost everywhere else.
    date: (y, region) => {
      if (region === "ES" || region === "PT") return nthWeekday(y, 4, 0, 1);
      if (region === "MX") return dayNumber(y, 4, 10);
      if (region === "AR") return nthWeekday(y, 9, 0, 3);
      return nthWeekday(y, 4, 0, 2);
    },
    before: 5,
    after: 0,
  },
  {
    id: "padre",
    // March 19th in Spain and Portugal, third Sunday of June elsewhere.
    date: (y, region) => (region === "ES" || region === "PT" ? dayNumber(y, 2, 19) : nthWeekday(y, 5, 0, 3)),
    before: 5,
    after: 0,
  },
  { id: "accionDeGracias", date: (y) => nthWeekday(y, 10, 4, 4), before: 7, after: 0, onlyIn: ["US"] },
];

/** `"es-MX"` -> `"MX"`; empty when the locale carries no region. */
export function regionOf(locale: string | undefined): string {
  return locale?.split(/[-_]/)[1]?.toUpperCase() ?? "";
}

/** The festivity whose window contains `now`, if any. If two overlap, the one whose day is nearer wins. */
export function activeFestivity(now: Date, region: string): FestivityId | null {
  const today = dayNumber(now.getFullYear(), now.getMonth(), now.getDate());
  let best: { id: FestivityId; distance: number } | null = null;
  for (const festivity of FESTIVITIES) {
    if (festivity.onlyIn && !festivity.onlyIn.includes(region)) continue;
    // A window can straddle New Year, so the day may belong to the next or previous year's festivity.
    for (const year of [now.getFullYear() - 1, now.getFullYear(), now.getFullYear() + 1]) {
      const date = festivity.date(year, region);
      if (today < date - festivity.before || today > date + festivity.after) continue;
      const distance = Math.abs(today - date);
      if (!best || distance < best.distance) best = { id: festivity.id, distance };
    }
  }
  return best?.id ?? null;
}

export function isFestivityId(value: string): value is FestivityId {
  return (FESTIVITY_IDS as readonly string[]).includes(value);
}

/** The festivity to show for a stored mode: none when off or unknown, today's when automatic, or the forced one. */
export function resolveFestivity(mode: string | undefined, now: Date, region: string): FestivityId | null {
  if (mode === "auto") return activeFestivity(now, region);
  return mode && isFestivityId(mode) ? mode : null;
}
