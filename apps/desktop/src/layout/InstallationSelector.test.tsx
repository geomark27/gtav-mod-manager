import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { makeInstallation, renderWithProviders } from "../features/installations/testing";
import { InstallationSelector } from "./InstallationSelector";

afterEach(() => clearMocks());

describe("InstallationSelector", () => {
  it("links to the installations page when none is registered", async () => {
    mockIPC((cmd) => (cmd === "list_installations" ? [] : null));
    renderWithProviders(<InstallationSelector />);
    const link = await screen.findByRole("link", { name: "No installation" });
    expect(link).toHaveAttribute("href", "/installations");
  });

  it("lists installations and switches the active one", async () => {
    const a = makeInstallation();
    const b = makeInstallation({
      id: "0192f0e8-0000-7000-8000-000000000002",
      displayName: "GTA V Enhanced (Epic)",
    });
    const calls: unknown[] = [];
    mockIPC((cmd, args) => {
      calls.push([cmd, args]);
      if (cmd === "list_installations") return [a, b];
      if (cmd === "get_selected_installation") return a;
      return undefined;
    });
    renderWithProviders(<InstallationSelector />);
    const select = await screen.findByRole("combobox", { name: "Active installation" });
    await waitFor(() => expect(select).toHaveValue(a.id));
    expect(screen.getByRole("option", { name: "GTA V Enhanced (Epic)" })).toBeInTheDocument();
    await userEvent.selectOptions(select, b.id);
    await waitFor(() => expect(calls).toContainEqual(["select_installation", { id: b.id }]));
  });
});
