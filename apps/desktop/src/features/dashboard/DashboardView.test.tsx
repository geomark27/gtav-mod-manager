import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { screen, within } from "@testing-library/react";
import type { DashboardDto } from "../../bindings/DashboardDto";
import { makeTxn } from "../activity/testing";
import { renderWithProviders } from "../installations/testing";
import { NO_VANILLA, OK_GUARD } from "../mods/testing";
import { DashboardView } from "./DashboardView";

afterEach(() => clearMocks());

function makeDashboard(over: Partial<DashboardDto> = {}): DashboardDto {
  return {
    mods: { total: 5, enabled: 3, disabled: 1, drifted: 1, error: 0 },
    inventory: [
      { classification: "candidate", count: 7 },
      { classification: "unknown", count: 2 },
      { classification: "original_modified", count: 4 },
    ],
    lastScan: {
      id: "s1",
      startedAt: new Date(Date.now() - 3 * 3600_000).toISOString(),
      finishedAt: null,
      status: "completed",
      filesSeen: 10,
      error: null,
    },
    baseline: { entries: 12, trust: "trusted", capturedAt: "2026-01-05T15:04:00Z" },
    vaultBytes: 2048,
    recentTransactions: [makeTxn({ description: "Enabled 2 mods" })],
    guard: OK_GUARD,
    ...over,
  };
}

function mockDashboard(data: DashboardDto) {
  mockIPC((cmd) => {
    if (cmd === "dashboard") return data;
    if (cmd === "vanilla_status") return NO_VANILLA;
    return undefined;
  });
}

describe("DashboardView", () => {
  it("renders the numbers", async () => {
    mockDashboard(makeDashboard());
    renderWithProviders(<DashboardView installationId="i" />);
    const mods = await screen.findByRole("region", { name: "Mods" });
    expect(within(mods).getByText("3 / 5")).toBeInTheDocument();
    expect(within(mods).getByText("1 drifted")).toBeInTheDocument();
    const inv = screen.getByRole("region", { name: "Inventory" });
    expect(within(inv).getByText("7")).toBeInTheDocument();
    expect(within(inv).getByText("2")).toBeInTheDocument();
    expect(within(inv).getByText("4")).toBeInTheDocument();
    const scan = screen.getByRole("region", { name: "Last scan" });
    expect(within(scan).getByText("3 hours ago")).toBeInTheDocument();
    expect(within(scan).getByText("Completed")).toBeInTheDocument();
    const baseline = screen.getByRole("region", { name: "Baseline" });
    expect(within(baseline).getByText("Trusted")).toBeInTheDocument();
    expect(within(baseline).getByText(/12 files · captured/)).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Vault usage" })).toHaveTextContent("2.0 KB");
    const recent = screen.getByRole("region", { name: "Recent activity" });
    expect(within(recent).getByText(/Enabled 2 mods/)).toBeInTheDocument();
    expect(within(recent).getByRole("link", { name: "View all activity" })).toHaveAttribute(
      "href",
      "/activity",
    );
  });

  it("shows the guard banner when writes are blocked", async () => {
    mockDashboard(
      makeDashboard({ guard: { ...OK_GUARD, gameRunning: ["GTA5.exe"] } }),
    );
    renderWithProviders(<DashboardView installationId="i" />);
    expect(await screen.findByText(/The game or a launcher is running/)).toBeInTheDocument();
  });

  it("handles no scan, no baseline and no activity", async () => {
    mockDashboard(
      makeDashboard({
        lastScan: null,
        baseline: { entries: 0, trust: "none", capturedAt: null },
        recentTransactions: [],
        inventory: [],
        mods: { total: 0, enabled: 0, disabled: 0, drifted: 0, error: 0 },
      }),
    );
    renderWithProviders(<DashboardView installationId="i" />);
    expect(await screen.findByText("Never scanned.")).toBeInTheDocument();
    expect(screen.getByText("No baseline")).toBeInTheDocument();
    expect(screen.getByText("No activity yet.")).toBeInTheDocument();
    expect(screen.queryByText(/drifted/)).not.toBeInTheDocument();
  });

  it("shows an error", async () => {
    mockIPC(() => {
      throw { code: "INTERNAL", message: "boom", hint: null, details: null };
    });
    renderWithProviders(<DashboardView installationId="i" />);
    expect(await screen.findByText("INTERNAL")).toBeInTheDocument();
  });
});
