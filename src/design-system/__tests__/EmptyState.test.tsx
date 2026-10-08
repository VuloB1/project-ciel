import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { EmptyState } from "../EmptyState";

describe("EmptyState", () => {
  it("renders title and description", () => {
    render(<EmptyState icon={<span>icon</span>} title="Título" description="Descripción" />);

    expect(screen.getByText("Título")).toBeInTheDocument();
    expect(screen.getByText("Descripción")).toBeInTheDocument();
  });

  it("renders without a description", () => {
    render(<EmptyState icon={<span>icon</span>} title="Solo título" />);

    expect(screen.getByText("Solo título")).toBeInTheDocument();
  });
});
