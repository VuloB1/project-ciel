import { describe, expect, it } from "vitest";

import { activeFestivity, regionOf, resolveFestivity } from "../festivities";

const on = (iso: string, region = "") => activeFestivity(new Date(`${iso}T12:00:00`), region);

describe("activeFestivity", () => {
  it("is quiet outside every window", () => {
    expect(on("2026-09-15")).toBeNull();
    expect(on("2026-06-01")).toBeNull();
  });

  it("shows Halloween all October and hands over to the Day of the Dead on November 1st", () => {
    expect(on("2026-09-30")).toBeNull();
    expect(on("2026-10-01")).toBe("halloween");
    expect(on("2026-10-19")).toBe("halloween");
    expect(on("2026-10-31")).toBe("halloween");
    expect(on("2026-11-01")).toBe("muertos");
    expect(on("2026-11-03")).toBe("muertos");
    expect(on("2026-11-04")).toBeNull();
  });

  it("hands Christmas over to New Year once Christmas is further than New Year", () => {
    expect(on("2026-12-26")).toBe("navidad");
    expect(on("2026-12-27")).toBe("anoNuevo");
    expect(on("2027-01-02")).toBe("anoNuevo");
    expect(on("2027-01-03")).toBe("reyes");
    expect(on("2027-01-06")).toBe("reyes");
    expect(on("2027-01-07")).toBeNull();
  });

  it("finds Easter on the right Sunday", () => {
    expect(on("2026-04-05")).toBe("pascua");
    expect(on("2026-03-29")).toBe("pascua");
    expect(on("2026-03-28")).toBeNull();
    expect(on("2027-03-28")).toBe("pascua");
    expect(on("2027-03-21")).toBe("pascua");
  });

  it("moves Mother's Day and Father's Day with the country", () => {
    expect(on("2026-05-10")).toBe("madre"); // second Sunday of May
    expect(on("2026-05-03", "ES")).toBe("madre"); // first Sunday of May
    expect(on("2026-05-10", "MX")).toBe("madre");
    expect(on("2026-10-18", "AR")).toBe("madre"); // third Sunday of October (also Halloween's month: the nearer day wins)
    expect(on("2026-06-21")).toBe("padre"); // third Sunday of June
    expect(on("2026-03-19", "ES")).toBe("padre");
    expect(on("2026-03-19")).toBeNull();
  });

  it("only turns Thanksgiving on by itself in the United States", () => {
    expect(on("2026-11-26", "US")).toBe("accionDeGracias");
    expect(on("2026-11-20", "US")).toBe("accionDeGracias");
    expect(on("2026-11-27", "US")).toBeNull();
    expect(on("2026-11-26", "ES")).toBeNull();
    expect(on("2026-11-26")).toBeNull();
  });

  it("covers the rest of the calendar", () => {
    expect(on("2027-02-14")).toBe("sanValentin");
    expect(on("2027-03-17")).toBe("sanPatricio");
    expect(on("2027-03-18")).toBeNull();
  });
});

describe("turned-off festivities", () => {
  it("are skipped, and leave the day free for the next one", () => {
    const at = (iso: string, disabled: string[]) => activeFestivity(new Date(`${iso}T12:00:00`), "", disabled);
    expect(at("2026-10-19", ["halloween"])).toBeNull();
    expect(at("2026-11-02", ["muertos"])).toBeNull();
    expect(at("2026-12-25", ["navidad"])).toBeNull();
    expect(at("2026-12-26", ["navidad"])).toBe("anoNuevo");
    expect(at("2026-10-19", ["navidad"])).toBe("halloween");
  });
});

describe("resolveFestivity", () => {
  const now = new Date("2026-10-19T12:00:00");

  it("shows nothing when off, unknown or unset", () => {
    expect(resolveFestivity("off", now, "")).toBeNull();
    expect(resolveFestivity(undefined, now, "")).toBeNull();
    expect(resolveFestivity("carnaval", now, "")).toBeNull();
  });

  it("follows the calendar when automatic", () => {
    expect(resolveFestivity("auto", now, "")).toBe("halloween");
    expect(resolveFestivity("auto", new Date("2026-09-01T12:00:00"), "")).toBeNull();
  });

  it("forces the chosen festivity on any day, even one that is turned off", () => {
    expect(resolveFestivity("navidad", now, "")).toBe("navidad");
    expect(resolveFestivity("navidad", now, "", ["navidad"])).toBe("navidad");
  });

  it("skips turned-off festivities when automatic", () => {
    expect(resolveFestivity("auto", now, "", ["halloween"])).toBeNull();
  });
});

describe("regionOf", () => {
  it("reads the country from a locale", () => {
    expect(regionOf("es-MX")).toBe("MX");
    expect(regionOf("en_us")).toBe("US");
    expect(regionOf("es")).toBe("");
    expect(regionOf(undefined)).toBe("");
  });
});
