import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { screen, within } from "@testing-library/react";
import { makeInstallation, renderWithProviders } from "../features/installations/testing";
import { InstallationsPage } from "./InstallationsPage";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

afterEach(() => clearMocks());

describe("InstallationsPage", () => {
  it("shows the empty state", async () => {
    mockIPC((cmd) => (cmd === "list_installations" ? [] : null));
    renderWithProviders(<InstallationsPage />);
    expect(await screen.findByText("No installation registered yet.")).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 1, name: "Installations" })).toBeInTheDocument();
  });

  it("lists installations and marks the active one", async () => {
    const a = makeInstallation();
    const b = makeInstallation({
      id: "0192f0e8-0000-7000-8000-000000000002",
      displayName: "GTA V Enhanced (Epic)",
      edition: "enhanced",
      platform: "epic",
    });
    mockIPC((cmd) => {
      if (cmd === "list_installations") return [a, b];
      if (cmd === "get_selected_installation") return a;
      return undefined;
    });
    renderWithProviders(<InstallationsPage />);
    expect(await screen.findByText("GTA V Enhanced (Epic)")).toBeInTheDocument();
    expect(screen.getByText("GTA V Legacy (Steam)")).toBeInTheDocument();
    expect(await screen.findAllByText("Active")).toHaveLength(1);
    expect(screen.getAllByRole("button", { name: /set .* as active/i })).toHaveLength(1);
  });

  it("groups installations by game", async () => {
    const gta = makeInstallation();
    const red = makeInstallation({
      id: "0192f0e8-0000-7000-8000-000000000003",
      game: "rdr2",
      edition: "rdr2",
      displayName: "Red Dead Redemption 2 (Steam)",
      rootPath: "/games/rdr2",
    });
    mockIPC((cmd) => {
      if (cmd === "list_installations") return [red, gta];
      if (cmd === "get_selected_installation") return gta;
      return undefined;
    });
    renderWithProviders(<InstallationsPage />);
    const groups = await screen.findAllByRole("heading", { level: 2, name: /^(Grand Theft Auto V|Red Dead Redemption 2)$/ });
    expect(groups.map((h) => h.textContent)).toEqual(["Grand Theft Auto V", "Red Dead Redemption 2"]);
    const redGroup = screen.getByRole("region", { name: "Red Dead Redemption 2" });
    expect(within(redGroup).getByText("Red Dead Redemption 2 (Steam)")).toBeInTheDocument();
    expect(within(redGroup).queryByText("GTA V Legacy (Steam)")).not.toBeInTheDocument();
  });
});
