import { beforeEach, describe, expect, it, vi } from "vitest";

import { clearImageCache, invalidateCachedImages, loadCachedImage, peekCachedImage } from "@/lib/imageCache";

// 1x1 PNG
const PNG = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

describe("imageCache", () => {
  beforeEach(() => {
    clearImageCache();
    URL.createObjectURL = vi.fn(() => "blob:test");
    URL.revokeObjectURL = vi.fn();
  });

  it("asks for the same image once, even from simultaneous callers", async () => {
    const load = vi.fn(async () => PNG);
    const [a, b] = await Promise.all([loadCachedImage("k", load), loadCachedImage("k", load)]);
    await loadCachedImage("k", load);
    expect(load).toHaveBeenCalledTimes(1);
    expect(a).toBe("blob:test");
    expect(b).toBe("blob:test");
    expect(URL.createObjectURL).toHaveBeenCalledTimes(1);
  });

  it("does not remember a failed load", async () => {
    const load = vi.fn().mockRejectedValueOnce(new Error("boom")).mockResolvedValueOnce(PNG);
    await expect(loadCachedImage("k", load)).rejects.toThrow("boom");
    expect(peekCachedImage("k")).toBeUndefined();
    expect(await loadCachedImage("k", load)).toBe("blob:test");
  });

  it("invalidating by prefix forces a new request", async () => {
    const load = vi.fn(async () => PNG);
    await loadCachedImage("profile:1:avatar", load);
    await loadCachedImage("other", load);
    invalidateCachedImages("profile:1:");
    expect(peekCachedImage("profile:1:avatar")).toBeUndefined();
    expect(peekCachedImage("other")).toBe("blob:test");
  });
});
