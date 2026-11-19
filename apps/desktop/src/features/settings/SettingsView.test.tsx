import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { screen, within } from "@testing-library/react";
import { renderWithProviders } from "../installations/testing";
import { SettingsView, STORY_MODE_NOTICE } from "./SettingsView";

afterEach(() => clearMocks());

describe("SettingsView", () => {
  it("shows version, paths, recovery counts and the story-mode notice", async () => {
    mockIPC((cmd) => {
      if (cmd === "vault_report") {
        return { objectCount: 4, totalBytes: 2048, unreferenced: [], unreferencedBytes: 0, strayFiles: [] };
      }
      return {
      appVersion: "0.1.0",
      dataDir: "/data",
      vaultDir: "/data/vault",
      logsDir: "/data/logs",
      dbPath: "/data/gmm.sqlite3",
      schemaVersion: 3,
      journalMode: "wal",
      synchronous: "full",
      foreignKeys: true,
      startedAt: "2026-01-01T00:00:00Z",
      recovery: { rolledBack: 1, committed: 2, needsAttention: 3, skipped: 4 },
      };
    });
    renderWithProviders(<SettingsView />);
    expect(await screen.findByText("0.1.0")).toBeInTheDocument();
    expect(screen.getByText("/data/vault")).toBeInTheDocument();
    expect(screen.getByText("/data/logs")).toBeInTheDocument();
    expect(screen.getByText("/data/gmm.sqlite3")).toBeInTheDocument();
    const rec = screen.getByRole("region", { name: "Startup recovery" });
    for (const n of ["1", "2", "3", "4"]) expect(within(rec).getByText(n)).toBeInTheDocument();
    expect(screen.getByText(STORY_MODE_NOTICE)).toBeInTheDocument();
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  it("shows an error", async () => {
    mockIPC(() => {
      throw { code: "INTERNAL", message: "boom", hint: null, details: null };
    });
    renderWithProviders(<SettingsView />);
    expect(await screen.findByText("INTERNAL")).toBeInTheDocument();
  });
});
