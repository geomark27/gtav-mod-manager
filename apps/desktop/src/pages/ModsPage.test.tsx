import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { screen } from "@testing-library/react";
import { makeInstallation, renderWithProviders } from "../features/installations/testing";
import { makeMod, OK_GUARD } from "../features/mods/testing";
import { ModsPage } from "./ModsPage";

afterEach(() => clearMocks());

describe("ModsPage", () => {
  it("links to Installations without an active installation", async () => {
    mockIPC((cmd) => (cmd === "get_selected_installation" ? null : undefined));
    renderWithProviders(<ModsPage />);
    expect(await screen.findByRole("link", { name: "Go to Installations" })).toHaveAttribute(
      "href",
      "/installations",
    );
    expect(screen.queryByText(/Planned for milestone/)).not.toBeInTheDocument();
  });

  it("shows the installation name and its mods", async () => {
    mockIPC((cmd) => {
      if (cmd === "get_selected_installation") return makeInstallation();
      if (cmd === "guard_status") return OK_GUARD;
      if (cmd === "list_mods") return [makeMod({ state: "enabled" })];
      return [];
    });
    renderWithProviders(<ModsPage />);
    expect(await screen.findByText("GTA V Legacy (Steam)")).toBeInTheDocument();
    expect(await screen.findByRole("switch", { name: "Alpha" })).toBeChecked();
  });
});
