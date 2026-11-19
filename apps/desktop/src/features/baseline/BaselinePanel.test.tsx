import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { act, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { BaselineDto } from "../../bindings/BaselineDto";
import { emitProgress } from "../../lib/eventMock";
import { renderWithProviders } from "../installations/testing";
import { deferred, makeProgress, makeSummary } from "../inventory/testing";
import { BaselinePanel } from "./BaselinePanel";

vi.mock("@tauri-apps/api/event", () => import("../../lib/eventMock"));

const ID = "0192f0e8-0000-7000-8000-000000000001";
const CAPTURED: BaselineDto = { entries: 12, trust: "trusted", capturedAt: "2026-01-05T15:04:00Z" };
const VERIFIED = /I verified the game files with the launcher/;

afterEach(() => clearMocks());

function open() {
  return userEvent.click(screen.getByRole("button", { name: "Capture baseline" }));
}

describe("BaselinePanel", () => {
  it("shows the current baseline trust", async () => {
    mockIPC((cmd) =>
      cmd === "inventory_summary"
        ? makeSummary({ baseline: { entries: 7, trust: "untrusted", capturedAt: null } })
        : undefined,
    );
    renderWithProviders(<BaselinePanel installationId={ID} />);
    expect(await screen.findByText("Untrusted")).toBeInTheDocument();
    expect(screen.getByText("7 files")).toBeInTheDocument();
  });

  it("explains the capture and starts untrusted by default", async () => {
    const calls: Array<{ cmd: string; args: unknown }> = [];
    mockIPC((cmd, args) => {
      calls.push({ cmd, args });
      if (cmd === "capture_baseline") return { ...CAPTURED, trust: "untrusted" };
      return cmd === "inventory_summary" ? makeSummary() : undefined;
    });
    renderWithProviders(<BaselinePanel installationId={ID} />);
    expect(await screen.findByText("No baseline")).toBeInTheDocument();
    await open();
    expect(
      screen.getByText(
        "Reads and hashes every game file. This can take several minutes for a full installation.",
      ),
    ).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: VERIFIED })).not.toBeChecked();
    await userEvent.click(screen.getByRole("button", { name: "Start" }));
    await waitFor(() =>
      expect(calls).toContainEqual({
        cmd: "capture_baseline",
        args: { id: ID, trusted: false },
      }),
    );
    expect(await screen.findByText(/Baseline captured \(12 files\)/)).toBeInTheDocument();
  });

  it("passes trusted=true when the launcher check is ticked", async () => {
    const calls: Array<{ cmd: string; args: unknown }> = [];
    mockIPC((cmd, args) => {
      calls.push({ cmd, args });
      if (cmd === "capture_baseline") return CAPTURED;
      return cmd === "inventory_summary" ? makeSummary() : undefined;
    });
    renderWithProviders(<BaselinePanel installationId={ID} />);
    await screen.findByText("No baseline");
    await open();
    await userEvent.click(screen.getByRole("checkbox", { name: VERIFIED }));
    await userEvent.click(screen.getByRole("button", { name: "Start" }));
    await waitFor(() =>
      expect(calls).toContainEqual({ cmd: "capture_baseline", args: { id: ID, trusted: true } }),
    );
  });

  it("shows progress and can be cancelled", async () => {
    const capture = deferred<BaselineDto>();
    const cmds: string[] = [];
    mockIPC((cmd) => {
      cmds.push(cmd);
      if (cmd === "capture_baseline") return capture.promise;
      return cmd === "inventory_summary" ? makeSummary() : undefined;
    });
    renderWithProviders(<BaselinePanel installationId={ID} />);
    await screen.findByText("No baseline");
    await open();
    await userEvent.click(screen.getByRole("button", { name: "Start" }));
    expect(await screen.findByText("Starting…")).toBeInTheDocument();

    act(() =>
      emitProgress(makeProgress({ kind: "baseline", filesSeen: 5, bytesHashed: 2048, current: "x.rpf" })),
    );
    expect(await screen.findByText(/5 files read · 2\.0 KB hashed/)).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(cmds).toContain("cancel_operation"));
    await act(async () =>
      capture.reject({ code: "CANCELLED", message: "The operation was cancelled.", hint: null, details: null }),
    );
    expect(
      await screen.findByText("Capture cancelled. The previous baseline was kept."),
    ).toBeInTheDocument();
  });

  it("renders the busy error", async () => {
    mockIPC((cmd) => {
      if (cmd === "capture_baseline") {
        throw {
          code: "OPERATION_IN_PROGRESS",
          message: "Another operation is running. Wait for it to finish or cancel it.",
          hint: null,
          details: null,
        };
      }
      return cmd === "inventory_summary" ? makeSummary() : undefined;
    });
    renderWithProviders(<BaselinePanel installationId={ID} />);
    await screen.findByText("No baseline");
    await open();
    await userEvent.click(screen.getByRole("button", { name: "Start" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("OPERATION_IN_PROGRESS");
  });
});
