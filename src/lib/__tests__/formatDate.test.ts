import { describe, expect, it } from "vitest";

import { formatDateTime, formatTime } from "../formatDate";

describe("formatDateTime", () => {
  it("formats a valid ISO timestamp", () => {
    const result = formatDateTime("2026-08-29T14:32:00Z", "en");
    expect(result).toContain("2026");
    expect(result.length).toBeGreaterThan(0);
  });

  it("respects the given locale", () => {
    const en = formatDateTime("2026-01-15T10:00:00Z", "en");
    const es = formatDateTime("2026-01-15T10:00:00Z", "es");
    // Different locales format month names differently ("Jan" vs "ene").
    expect(en).not.toBe(es);
  });

  it("falls back to the raw string for an unparseable value instead of throwing", () => {
    expect(formatDateTime("not-a-date", "en")).toBe("not-a-date");
  });
});

describe("formatTime", () => {
  it("formats just the time, no date", () => {
    const result = formatTime("2026-08-29T14:32:00Z", "en");
    expect(result).not.toContain("2026");
    expect(result.length).toBeGreaterThan(0);
  });

  it("returns an empty string for an unparseable value instead of the raw text", () => {
    expect(formatTime("not-a-date", "en")).toBe("");
  });
});
