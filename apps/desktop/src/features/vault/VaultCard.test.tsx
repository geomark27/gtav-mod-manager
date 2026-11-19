import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { VaultReportDto } from "../../bindings/VaultReportDto";
import { renderWithProviders } from "../installations/testing";
import { VaultCard } from "./VaultCard";

afterEach(() => clearMocks());

const REPORT: VaultReportDto = {
  objectCount: 10,
  totalBytes: 10240,
  unreferenced: [
    { sha256: "a".repeat(64), size: 1024 },
    { sha256: "b".repeat(64), size: 2048 },
  ],
  unreferencedBytes: 3072,
  strayFiles: ["objects/zz/odd.tmp"],
};

describe("VaultCard", () => {
  it("shows the report, explains unreferenced copies and lists stray files read-only", async () => {
    mockIPC(() => REPORT);
    renderWithProviders(<VaultCard />);
    expect(await screen.findByText("10 (10 KB)")).toBeInTheDocument();
    expect(screen.getByText("2 (3.0 KB)")).toBeInTheDocument();
    expect(
      screen.getByText("Includes copies kept when you restored a mod or re-adopted files."),
    ).toBeInTheDocument();
    const stray = screen.getByRole("list", { name: "Stray files" });
    expect(within(stray).getByText("objects/zz/odd.tmp")).toBeInTheDocument();
    expect(screen.getAllByRole("button")).toHaveLength(1);
  });

  it("deletes only after confirmation and sends every unreferenced digest", async () => {
    const calls: Array<{ cmd: string; args: Record<string, unknown> }> = [];
    let report = REPORT;
    mockIPC((cmd, payload) => {
      calls.push({ cmd, args: (payload ?? {}) as Record<string, unknown> });
      if (cmd === "vault_gc") {
        report = { ...REPORT, unreferenced: [], unreferencedBytes: 0, objectCount: 8 };
        return 3072;
      }
      return report;
    });
    renderWithProviders(<VaultCard />);
    await userEvent.click(await screen.findByRole("button", { name: "Delete unreferenced copies…" }));
    expect(calls.some((c) => c.cmd === "vault_gc")).toBe(false);
    await userEvent.click(screen.getByRole("button", { name: "Delete copies" }));
    await waitFor(() =>
      expect(calls.find((c) => c.cmd === "vault_gc")?.args).toEqual({
        shas: ["a".repeat(64), "b".repeat(64)],
      }),
    );
    expect(await screen.findByText("Freed 3.0 KB.")).toBeInTheDocument();
    await waitFor(() =>
      expect(screen.queryByRole("button", { name: "Delete unreferenced copies…" })).not.toBeInTheDocument(),
    );
  });

  it("cancelling the confirmation deletes nothing", async () => {
    const cmds: string[] = [];
    mockIPC((cmd) => {
      cmds.push(cmd);
      return REPORT;
    });
    renderWithProviders(<VaultCard />);
    await userEvent.click(await screen.findByRole("button", { name: "Delete unreferenced copies…" }));
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(cmds).not.toContain("vault_gc");
  });

  it("shows RECOVERY_REQUIRED from the backend", async () => {
    mockIPC((cmd) => {
      if (cmd === "vault_gc") {
        throw { code: "RECOVERY_REQUIRED", message: "Unresolved change.", hint: "Resolve it first.", details: null };
      }
      return REPORT;
    });
    renderWithProviders(<VaultCard />);
    await userEvent.click(await screen.findByRole("button", { name: "Delete unreferenced copies…" }));
    await userEvent.click(screen.getByRole("button", { name: "Delete copies" }));
    expect(await screen.findByText("RECOVERY_REQUIRED")).toBeInTheDocument();
  });
});
