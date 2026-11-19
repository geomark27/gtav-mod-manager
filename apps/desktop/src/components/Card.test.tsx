import { render, screen } from "@testing-library/react";
import { Card } from "./Card";

describe("Card", () => {
  it("renders the title and children", () => {
    render(<Card title="Status">Body</Card>);
    expect(screen.getByRole("heading", { name: "Status" })).toBeInTheDocument();
    expect(screen.getByText("Body")).toBeInTheDocument();
  });

  it("renders without a title", () => {
    render(<Card>Only body</Card>);
    expect(screen.queryByRole("heading")).not.toBeInTheDocument();
    expect(screen.getByText("Only body")).toBeInTheDocument();
  });
});
