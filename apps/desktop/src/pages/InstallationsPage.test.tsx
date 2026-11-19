import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { screen } from "@testing-library/react";
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
});
