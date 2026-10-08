import { beforeEach, describe, expect, it } from "vitest";

import { useToastStore } from "../toastStore";

describe("useToastStore", () => {
  beforeEach(() => {
    useToastStore.setState({ toasts: [] });
  });

  it("starts empty", () => {
    expect(useToastStore.getState().toasts).toHaveLength(0);
  });

  it("push adds a toast with a generated id", () => {
    useToastStore.getState().push({ variant: "success", title: "Hecho" });

    const { toasts } = useToastStore.getState();
    expect(toasts).toHaveLength(1);
    expect(toasts[0]).toMatchObject({ variant: "success", title: "Hecho" });
    expect(toasts[0]?.id).toBeTruthy();
  });

  it("dismiss removes only the matching toast", () => {
    useToastStore.getState().push({ variant: "info", title: "first" });
    useToastStore.getState().push({ variant: "info", title: "second" });

    const [first, second] = useToastStore.getState().toasts;
    useToastStore.getState().dismiss(first!.id);

    const remaining = useToastStore.getState().toasts;
    expect(remaining).toHaveLength(1);
    expect(remaining[0]?.id).toBe(second!.id);
  });
});
