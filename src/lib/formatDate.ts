/** Locale-aware "Aug 29, 2026, 2:32 PM" / "29 ago 2026, 14:32" formatting
 * for the ISO 8601 timestamps every backend DTO already carries
 * (`createdAt`, etc.) — `locale` should be `i18n.language` so it follows
 * whatever language the user picked in Configuración > General. */
export function formatDateTime(isoString: string, locale: string): string {
  const date = new Date(isoString);
  if (Number.isNaN(date.getTime())) return isoString;
  return new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" }).format(date);
}

/** Just "2:32 PM" / "14:32" — no date — for a per-message timestamp shown
 * inline in a live feed (Chat, Twitch, Discord, Llamada), where the full
 * date from `formatDateTime` would be redundant noise on every single
 * row. Returns `""` (not the raw string) on an unparseable value, since
 * this renders inline next to every message rather than standing alone. */
export function formatTime(isoString: string, locale: string): string {
  const date = new Date(isoString);
  if (Number.isNaN(date.getTime())) return "";
  return new Intl.DateTimeFormat(locale, { timeStyle: "short" }).format(date);
}
