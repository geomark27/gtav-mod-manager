import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Modal } from "./Modal";

describe("Modal", () => {
  it("renders title, description and children, and closes on Escape", async () => {
    const onOpenChange = vi.fn();
    render(
      <Modal open onOpenChange={onOpenChange} title="Title" description="About it">
        <p>Body</p>
      </Modal>,
    );
    expect(screen.getByRole("dialog", { name: "Title" })).toBeInTheDocument();
    expect(screen.getByText("About it")).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("does not close when not dismissible", async () => {
    const onOpenChange = vi.fn();
    render(
      <Modal open onOpenChange={onOpenChange} title="Busy" description="Working" dismissible={false}>
        <p>Body</p>
      </Modal>,
    );
    await userEvent.keyboard("{Escape}");
    expect(onOpenChange).not.toHaveBeenCalled();
  });

  it("renders nothing when closed", () => {
    render(
      <Modal open={false} onOpenChange={() => {}} title="Hidden" description="x">
        <p>Body</p>
      </Modal>,
    );
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});
