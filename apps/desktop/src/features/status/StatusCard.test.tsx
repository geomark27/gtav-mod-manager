import { QueryClientProvider } from "@tanstack/react-query";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { render, screen } from "@testing-library/react";
import { createQueryClient } from "../../lib/queryClient";
import { StatusCard } from "./StatusCard";

function renderCard() {
  render(
    <QueryClientProvider client={createQueryClient()}>
      <StatusCard />
    </QueryClientProvider>,
  );
}

afterEach(() => clearMocks());

describe("StatusCard", () => {
  it("renders backend values", async () => {
    mockIPC(() => ({
      appVersion: "0.1.0",
      dataDir: "/data",
      vaultDir: "/data/vault",
      logsDir: "/data/logs",
      dbPath: "/data/gmm.sqlite3",
      schemaVersion: 1,
      journalMode: "wal",
      synchronous: "full",
      foreignKeys: true,
      startedAt: "2026-01-01T00:00:00Z",
    }));
    renderCard();
    expect(screen.getByText(/loading/i)).toBeInTheDocument();
    expect(await screen.findByText("wal")).toBeInTheDocument();
    expect(screen.getByText("full")).toBeInTheDocument();
    expect(screen.getByText("0.1.0")).toBeInTheDocument();
    expect(screen.getByText("/data/vault")).toBeInTheDocument();
    expect(screen.getByText("/data/logs")).toBeInTheDocument();
    expect(screen.getByText("On")).toBeInTheDocument();
  });

  it("renders ALREADY_RUNNING code, message and hint", async () => {
    mockIPC(() => {
      throw {
        code: "ALREADY_RUNNING",
        message: "GTA V Mod Manager is already running.",
        hint: "Close the other window of GTA V Mod Manager.",
        details: null,
      };
    });
    renderCard();
    expect(await screen.findByText("ALREADY_RUNNING")).toBeInTheDocument();
    expect(screen.getByText("GTA V Mod Manager is already running.")).toBeInTheDocument();
    expect(screen.getByText("Close the other window of GTA V Mod Manager.")).toBeInTheDocument();
  });

  it("shows INTERNAL for a non-AppError rejection", async () => {
    mockIPC(() => {
      throw "weird";
    });
    renderCard();
    expect(await screen.findByText("INTERNAL")).toBeInTheDocument();
    expect(screen.getByText("weird")).toBeInTheDocument();
  });
});
