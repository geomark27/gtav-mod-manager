import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { renderWithProviders } from "../installations/testing";
import { ActivityView } from "./ActivityView";
import { makeTxn } from "./testing";

afterEach(() => clearMocks());

describe("ActivityView", () => {
  it("shows an empty message", async () => {
    mockIPC(() => ({ items: [], total: 0 }));
    renderWithProviders(<ActivityView installationId="i" />);
    expect(await screen.findByText("No activity yet.")).toBeInTheDocument();
  });

  it("labels every status and shows the error text", async () => {
    mockIPC(() => ({
      total: 4,
      items: [
        makeTxn({ id: "a", description: "Enabled 1 mod" }),
        makeTxn({ id: "b", status: "rolled_back", description: "Disabled 1 mod", error: "disk full" }),
        makeTxn({ id: "c", status: "needs_attention", description: "Applied profile" }),
        makeTxn({ id: "d", status: "open", description: "Restored mod “X”", kind: "restore" }),
      ],
    }));
    renderWithProviders(<ActivityView installationId="i" />);
    expect(await screen.findByText("Done")).toBeInTheDocument();
    expect(screen.getByText("Rolled back")).toBeInTheDocument();
    expect(screen.getByText("Needs attention")).toBeInTheDocument();
    expect(screen.getByText("In progress")).toBeInTheDocument();
    expect(screen.getByText("disk full")).toBeInTheDocument();
    expect(screen.getByText("Restore")).toBeInTheDocument();
  });

  it("expands a row to show mod names and the step count", async () => {
    mockIPC(() => ({
      total: 1,
      items: [makeTxn({ modNames: ["Alpha", "Beta"], stepCount: 1 })],
    }));
    renderWithProviders(<ActivityView installationId="i" />);
    await userEvent.click(await screen.findByRole("button", { name: "Enabled 1 mod" }));
    expect(screen.getByText("Mods: Alpha, Beta")).toBeInTheDocument();
    expect(screen.getByText("1 file step")).toBeInTheDocument();
  });

  it("paginates with limit and offset", async () => {
    const calls: unknown[] = [];
    mockIPC((_cmd, args) => {
      calls.push(args);
      const offset = (args as { offset: number }).offset;
      return {
        total: 25,
        items: [makeTxn({ id: `t${offset}`, description: offset === 0 ? "First page" : "Second page" })],
      };
    });
    renderWithProviders(<ActivityView installationId="i" />);
    expect(await screen.findByText("First page")).toBeInTheDocument();
    expect(screen.getByText("Page 1 of 2")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Previous" })).toBeDisabled();
    await userEvent.click(screen.getByRole("button", { name: "Next" }));
    expect(await screen.findByText("Second page")).toBeInTheDocument();
    expect(calls).toContainEqual({ installationId: "i", limit: 20, offset: 20 });
    expect(screen.getByRole("button", { name: "Next" })).toBeDisabled();
    await userEvent.click(screen.getByRole("button", { name: "Previous" }));
    expect(await screen.findByText("First page")).toBeInTheDocument();
  });

  it("shows a load error", async () => {
    mockIPC(() => {
      throw { code: "INTERNAL", message: "boom", hint: null, details: null };
    });
    renderWithProviders(<ActivityView installationId="i" />);
    expect(await screen.findByText("INTERNAL")).toBeInTheDocument();
  });
});
