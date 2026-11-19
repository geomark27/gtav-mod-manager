import { render, screen } from "@testing-library/react";
import { PlannedNotice } from "./PlannedNotice";

describe("PlannedNotice", () => {
  it("shows the milestone and description with no controls", () => {
    render(<PlannedNotice milestone="M5" description="Browse mods." />);
    expect(screen.getByText("Planned for milestone M5")).toBeInTheDocument();
    expect(screen.getByText("Browse mods.")).toBeInTheDocument();
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
    expect(screen.queryByRole("link")).not.toBeInTheDocument();
  });
});
