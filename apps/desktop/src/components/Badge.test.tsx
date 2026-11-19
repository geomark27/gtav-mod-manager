import { render, screen } from "@testing-library/react";
import { Badge } from "./Badge";

describe("Badge", () => {
  it("renders its text with the tone class", () => {
    render(<Badge tone="success">Enabled</Badge>);
    const badge = screen.getByText("Enabled");
    expect(badge.className).toContain("success");
  });

  it("uses a different class per tone", () => {
    render(<Badge tone="danger">Broken</Badge>);
    expect(screen.getByText("Broken").className).toContain("danger");
  });
});
