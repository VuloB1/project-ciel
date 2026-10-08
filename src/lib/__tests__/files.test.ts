import { describe, expect, it } from "vitest";

import { computeFitDimensions } from "../files";

describe("computeFitDimensions", () => {
  it("scales a square image down to exactly the max dimension", () => {
    expect(computeFitDimensions(4000, 4000, 1500)).toEqual({ width: 1500, height: 1500 });
  });

  it("scales a landscape image so width hits the max, height stays proportional", () => {
    expect(computeFitDimensions(3000, 1500, 1500)).toEqual({ width: 1500, height: 750 });
  });

  it("scales a portrait image so height hits the max, width stays proportional", () => {
    expect(computeFitDimensions(1500, 3000, 1500)).toEqual({ width: 750, height: 1500 });
  });

  it("leaves an image already within bounds unchanged", () => {
    expect(computeFitDimensions(800, 600, 1500)).toEqual({ width: 800, height: 600 });
  });

  it("leaves an image exactly at the max dimension unchanged", () => {
    expect(computeFitDimensions(1500, 900, 1500)).toEqual({ width: 1500, height: 900 });
  });

  it("never upscales a smaller image", () => {
    expect(computeFitDimensions(200, 100, 1500)).toEqual({ width: 200, height: 100 });
  });

  it("never produces a zero-pixel dimension for extreme aspect ratios", () => {
    const result = computeFitDimensions(10000, 1, 1500);
    expect(result.height).toBeGreaterThanOrEqual(1);
  });
});
