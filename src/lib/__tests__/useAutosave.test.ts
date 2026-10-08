import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useAutosave } from "../useAutosave";

describe("useAutosave", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("does not save the value it's first rendered with", () => {
    const save = vi.fn().mockResolvedValue(undefined);
    renderHook(({ value }) => useAutosave(value, save, { onError: vi.fn() }), { initialProps: { value: "initial" } });

    act(() => vi.advanceTimersByTime(10_000));

    expect(save).not.toHaveBeenCalled();
  });

  it("debounces repeated changes into a single save of the latest value", async () => {
    const save = vi.fn().mockResolvedValue(undefined);
    const { rerender } = renderHook(({ value }) => useAutosave(value, save, { debounceMs: 700, onError: vi.fn() }), {
      initialProps: { value: "a" },
    });

    rerender({ value: "ab" });
    act(() => vi.advanceTimersByTime(400));
    rerender({ value: "abc" });
    act(() => vi.advanceTimersByTime(699));
    expect(save).not.toHaveBeenCalled();

    await act(async () => {
      vi.advanceTimersByTime(1);
      await Promise.resolve();
    });
    expect(save).toHaveBeenCalledTimes(1);
    expect(save).toHaveBeenCalledWith("abc");
  });

  it("flush() saves immediately and cancels the pending debounce", async () => {
    const save = vi.fn().mockResolvedValue(undefined);
    const { result, rerender } = renderHook(({ value }) => useAutosave(value, save, { debounceMs: 700, onError: vi.fn() }), {
      initialProps: { value: "a" },
    });

    rerender({ value: "b" });
    await act(async () => {
      result.current.flush();
      await Promise.resolve();
    });
    expect(save).toHaveBeenCalledTimes(1);
    expect(save).toHaveBeenCalledWith("b");

    // The debounce that would have fired for this same edit must not
    // also fire after flush() already saved it.
    act(() => vi.advanceTimersByTime(700));
    expect(save).toHaveBeenCalledTimes(1);
  });

  it("does not treat the value right after enabled flips true as an edit", async () => {
    const save = vi.fn().mockResolvedValue(undefined);
    const { rerender } = renderHook(({ value, enabled }) => useAutosave(value, save, { enabled, onError: vi.fn() }), {
      initialProps: { value: "loading-placeholder", enabled: false },
    });

    // The real value arrives together with `enabled` turning true, like a
    // settings screen's `fetch().then(setState)` — not an edit.
    rerender({ value: "loaded-from-backend", enabled: true });
    act(() => vi.advanceTimersByTime(10_000));
    expect(save).not.toHaveBeenCalled();

    rerender({ value: "actually edited", enabled: true });
    await act(async () => {
      vi.advanceTimersByTime(700);
      await Promise.resolve();
    });
    expect(save).toHaveBeenCalledTimes(1);
    expect(save).toHaveBeenCalledWith("actually edited");
  });

  it("reports a rejected save via onError without throwing", async () => {
    const err = new Error("boom");
    const save = vi.fn().mockRejectedValue(err);
    const onError = vi.fn();
    const { rerender } = renderHook(({ value }) => useAutosave(value, save, { debounceMs: 100, onError }), {
      initialProps: { value: "a" },
    });

    rerender({ value: "b" });
    await act(async () => {
      vi.advanceTimersByTime(100);
      await Promise.resolve();
    });

    expect(onError).toHaveBeenCalledWith(err);
  });
});
