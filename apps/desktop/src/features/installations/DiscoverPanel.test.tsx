import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { DiscoveredInstallationDto } from "../../bindings/DiscoveredInstallationDto";
import { DiscoverPanel } from "./DiscoverPanel";
import { makeInstallation, renderWithProviders } from "./testing";

afterEach(() => clearMocks());

function candidate(over: Partial<DiscoveredInstallationDto> = {}): DiscoveredInstallationDto {
  return {
    rootPath: "/steam/GTA V",
    platform: "steam",
    source: "steam:libraryfolders",
    edition: "legacy",
    status: "ok",
    missingFiles: [],
    exeVersion: null,
    writable: true,
    alreadyRegistered: false,
    registrable: true,
    problems: [],
    ...over,
  };
}

describe("DiscoverPanel", () => {
  it("lists candidates and registers one", async () => {
    const calls: unknown[] = [];
    mockIPC((cmd, args) => {
      calls.push([cmd, args]);
      return cmd === "discover_installations" ? [candidate()] : makeInstallation();
    });
    renderWithProviders(<DiscoverPanel />);
    await userEvent.click(screen.getByRole("button", { name: "Detect installations" }));
    expect(await screen.findByText("/steam/GTA V")).toBeInTheDocument();
    expect(screen.getByText("Ready")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Register" }));
    await waitFor(() =>
      expect(calls).toContainEqual([
        "register_installation",
        { req: { rootPath: "/steam/GTA V", displayName: null, platform: "steam" } },
      ]),
    );
  });

  it("disables Register with a reason for a non registrable candidate", async () => {
    mockIPC(() => [
      candidate({
        edition: "legacy",
        status: "incomplete",
        registrable: false,
        missingFiles: ["common.rpf"],
        problems: ["Some required game files are missing."],
      }),
    ]);
    renderWithProviders(<DiscoverPanel />);
    await userEvent.click(screen.getByRole("button", { name: "Detect installations" }));
    expect(await screen.findByText("Some required game files are missing.")).toBeInTheDocument();
    expect(screen.getByText("common.rpf")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Register" })).toBeDisabled();
    expect(screen.getByText(/cannot be registered/i)).toBeInTheDocument();
  });

  it("disables Register for an already registered candidate", async () => {
    mockIPC(() => [candidate({ alreadyRegistered: true })]);
    renderWithProviders(<DiscoverPanel />);
    await userEvent.click(screen.getByRole("button", { name: "Detect installations" }));
    expect(await screen.findByText("Already registered.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Register" })).toBeDisabled();
  });

  it("says so when nothing is detected", async () => {
    mockIPC(() => []);
    renderWithProviders(<DiscoverPanel />);
    await userEvent.click(screen.getByRole("button", { name: "Detect installations" }));
    expect(await screen.findByText("No GTA V installations were detected.")).toBeInTheDocument();
  });
});
