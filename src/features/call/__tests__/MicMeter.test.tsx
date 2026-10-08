import "@/lib/i18n";

import { act, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import { useCallStore } from "@/stores/callStore";

import { MicMeter } from "../MicMeter";

describe("MicMeter", () => {
  beforeEach(() => {
    useCallStore.setState({ micLevel: 0 });
  });

  it("shows the current microphone level and follows the store", () => {
    render(<MicMeter />);
    expect(screen.getByRole("meter", { name: "Nivel del micrófono" })).toHaveAttribute("aria-valuenow", "0");

    act(() => useCallStore.getState().handleAppEvent({ type: "call_mic_level", level: 0.5 }));

    expect(screen.getByRole("meter")).toHaveAttribute("aria-valuenow", "50");
  });

  it("reads zero while muted, whatever the store says", () => {
    useCallStore.setState({ micLevel: 0.8 });
    render(<MicMeter muted />);
    expect(screen.getByRole("meter")).toHaveAttribute("aria-valuenow", "0");
  });
});
