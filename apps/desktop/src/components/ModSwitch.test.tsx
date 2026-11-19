import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ModSwitch } from "./ModSwitch";

describe("ModSwitch", () => {
  it("off: click requests true once and stays unchecked", async () => {
    const onRequestChange = vi.fn();
    render(<ModSwitch state="off" label="Mod A" onRequestChange={onRequestChange} />);
    const sw = screen.getByRole("switch", { name: "Mod A" });
    await userEvent.click(sw);
    expect(onRequestChange).toHaveBeenCalledTimes(1);
    expect(onRequestChange).toHaveBeenCalledWith(true);
    expect(sw).not.toBeChecked();
  });

  it("on: click requests false and stays checked", async () => {
    const onRequestChange = vi.fn();
    render(<ModSwitch state="on" label="Mod A" onRequestChange={onRequestChange} />);
    const sw = screen.getByRole("switch", { name: "Mod A" });
    expect(sw).toBeChecked();
    await userEvent.click(sw);
    expect(onRequestChange).toHaveBeenCalledWith(false);
    expect(sw).toBeChecked();
  });

  it("pending: disabled, busy and ignores clicks", async () => {
    const onRequestChange = vi.fn();
    render(<ModSwitch state="pending" label="Mod A" onRequestChange={onRequestChange} />);
    const sw = screen.getByRole("switch", { name: "Mod A" });
    expect(sw).toBeDisabled();
    expect(sw).toHaveAttribute("aria-busy", "true");
    await userEvent.click(sw);
    expect(onRequestChange).not.toHaveBeenCalled();
  });

  it("blocked: disabled and the reason is reachable", async () => {
    const onRequestChange = vi.fn();
    render(
      <ModSwitch
        state="blocked"
        label="Mod A"
        blockedReason="Game is running"
        onRequestChange={onRequestChange}
      />,
    );
    const sw = screen.getByRole("switch", { name: "Mod A" });
    expect(sw).toBeDisabled();
    expect(sw).toHaveAccessibleDescription("Game is running");
    await userEvent.click(sw);
    expect(onRequestChange).not.toHaveBeenCalled();
  });

  it("error: shows an indicator and allows retry", async () => {
    const onRequestChange = vi.fn();
    render(<ModSwitch state="error" label="Mod A" onRequestChange={onRequestChange} />);
    expect(screen.getByRole("img", { name: "Last change failed" })).toBeInTheDocument();
    const sw = screen.getByRole("switch", { name: "Mod A" });
    expect(sw).toBeEnabled();
    await userEvent.click(sw);
    expect(onRequestChange).toHaveBeenCalledTimes(1);
  });
});
