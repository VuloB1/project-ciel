import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { Button } from "../Button";

describe("Button", () => {
  it("renders its children", () => {
    render(<Button>Guardar</Button>);
    expect(screen.getByRole("button", { name: "Guardar" })).toBeInTheDocument();
  });

  it("calls onClick when clicked", () => {
    const onClick = vi.fn();
    render(<Button onClick={onClick}>Guardar</Button>);

    fireEvent.click(screen.getByRole("button", { name: "Guardar" }));
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it("does not call onClick when disabled", () => {
    const onClick = vi.fn();
    render(
      <Button onClick={onClick} disabled>
        Guardar
      </Button>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Guardar" }));
    expect(onClick).not.toHaveBeenCalled();
  });
});
