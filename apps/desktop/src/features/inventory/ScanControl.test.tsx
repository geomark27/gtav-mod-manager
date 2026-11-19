import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { act, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ScanSummaryDto } from "../../bindings/ScanSummaryDto";
import { emitProgress } from "../../lib/eventMock";
import { renderWithProviders } from "../installations/testing";
import { ScanControl } from "./ScanControl";
import { deferred, makeProgress, makeSummary } from "./testing";

vi.mock("@tauri-apps/api/event", () => import("../../lib/eventMock"));

const ID = "0192f0e8-0000-7000-8000-000000000001";
const SUMMARY: ScanSummaryDto = {
  scanId: "s1",
  filesSeen: 3,
  bytesHashed: 0,
  counts: [],
  finishedAt: "2026-01-05T15:04:00Z",
};

afterEach(() => clearMocks());

describe("ScanControl", () => {
  it("shows the last scan, or that there is none", async () => {
    mockIPC((cmd) => (cmd === "inventory_summary" ? makeSummary() : undefined));
    renderWithProviders(<ScanControl installationId={ID} />);
    expect(await screen.findByText("Not scanned yet.")).toBeInTheDocument();
  });

  it("shows the last scan time and status", async () => {
    mockIPC((cmd) =>
      cmd === "inventory_summary"
        ? makeSummary({
            lastScan: {
              id: "s0",
              startedAt: "2026-01-05T15:04:00Z",
              finishedAt: "2026-01-05T15:05:00Z",
              status: "cancelled",
              filesSeen: 10,
              error: null,
            },
          })
        : undefined,
    );
    renderWithProviders(<ScanControl installationId={ID} />);
    expect(await screen.findByText("Cancelled")).toBeInTheDocument();
    expect(screen.getByText(/Last scan:/)).toBeInTheDocument();
  });

  it("scans, shows live progress, then refreshes the summary", async () => {
    const scan = deferred<ScanSummaryDto>();
    const calls: Array<{ cmd: string; args: unknown }> = [];
    mockIPC((cmd, args) => {
      calls.push({ cmd, args });
      if (cmd === "scan_installation") return scan.promise;
      if (cmd === "inventory_summary") return makeSummary();
      return undefined;
    });
    renderWithProviders(<ScanControl installationId={ID} />);
    await userEvent.click(await screen.findByRole("button", { name: "Scan now" }));

    expect(await screen.findByText("Starting scan…")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cancel" })).toBeInTheDocument();
    expect(calls).toContainEqual({ cmd: "scan_installation", args: { id: ID } });

    act(() =>
      emitProgress(
        makeProgress({ filesSeen: 42, bytesHashed: 3 * 1024 * 1024, current: "update/update.rpf" }),
      ),
    );
    expect(await screen.findByText("42 files seen · 3.0 MB hashed")).toBeInTheDocument();
    expect(screen.getByText("update/update.rpf")).toBeInTheDocument();

    // Progress of another operation kind is ignored.
    act(() => emitProgress(makeProgress({ kind: "baseline", filesSeen: 999 })));
    expect(screen.queryByText(/999 files/)).not.toBeInTheDocument();

    const before = calls.filter((c) => c.cmd === "inventory_summary").length;
    await act(async () => scan.resolve(SUMMARY));
    await waitFor(() => expect(screen.queryByRole("button", { name: "Cancel" })).toBeNull());
    await waitFor(() =>
      expect(calls.filter((c) => c.cmd === "inventory_summary").length).toBeGreaterThan(before),
    );
    expect(screen.getByRole("button", { name: "Scan now" })).toBeEnabled();
  });

  it("cancels the running scan and reports it without an error", async () => {
    const scan = deferred<ScanSummaryDto>();
    const cmds: string[] = [];
    mockIPC((cmd) => {
      cmds.push(cmd);
      if (cmd === "scan_installation") return scan.promise;
      if (cmd === "inventory_summary") return makeSummary();
      return undefined;
    });
    renderWithProviders(<ScanControl installationId={ID} />);
    await userEvent.click(await screen.findByRole("button", { name: "Scan now" }));
    await userEvent.click(await screen.findByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(cmds).toContain("cancel_operation"));

    await act(async () =>
      scan.reject({ code: "CANCELLED", message: "The operation was cancelled.", hint: null, details: null }),
    );
    expect(
      await screen.findByText("Scan cancelled. The previous results were kept."),
    ).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("renders the busy error", async () => {
    mockIPC((cmd) => {
      if (cmd === "scan_installation") {
        throw {
          code: "OPERATION_IN_PROGRESS",
          message: "Another operation is running. Wait for it to finish or cancel it.",
          hint: null,
          details: null,
        };
      }
      return cmd === "inventory_summary" ? makeSummary() : undefined;
    });
    renderWithProviders(<ScanControl installationId={ID} />);
    await userEvent.click(await screen.findByRole("button", { name: "Scan now" }));
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("OPERATION_IN_PROGRESS");
    expect(alert).toHaveTextContent("Another operation is running.");
  });
});
