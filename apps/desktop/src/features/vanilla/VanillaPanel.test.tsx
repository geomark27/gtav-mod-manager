import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { PlanPreviewDto } from "../../bindings/PlanPreviewDto";
import type { VanillaStatusDto } from "../../bindings/VanillaStatusDto";
import { ONLINE_NOTICE } from "../../layout/AppShell";
import { renderWithProviders } from "../installations/testing";
import { APP_ERROR, NO_VANILLA } from "../mods/testing";
import { makeProfile } from "../profiles/testing";
import { unmanagedNote } from "./labels";
import { VanillaPanel } from "./VanillaPanel";

afterEach(() => clearMocks());

const RESTORE_POINT = makeProfile({
  id: "rp-1",
  name: "Before vanilla — 2026-11-19 14:03:09 UTC",
  kind: "restore_point",
  entryCount: 3,
});

const WITH_MODS: VanillaStatusDto = {
  ...NO_VANILLA,
  modsTotal: 3,
  modsInGame: 2,
  unmanagedFiles: 4,
};

const ACTIVE: VanillaStatusDto = {
  active: true,
  restorePoint: RESTORE_POINT,
  modsTotal: 3,
  modsInGame: 0,
  unmanagedFiles: 4,
};

const PREVIEW: PlanPreviewDto = {
  modsAffected: [
    { modId: "m1", name: "Trainer", from: "enabled", to: "disabled" },
    { modId: "m2", name: "Alpha", from: "enabled", to: "disabled" },
  ],
  actions: [],
  bytesToCopy: 0,
};

interface Mock {
  status: VanillaStatusDto;
  calls: Array<{ cmd: string; args: Record<string, unknown> }>;
  handlers: Record<string, (args: Record<string, unknown>) => unknown>;
}

function mockVanilla(status: VanillaStatusDto): Mock {
  const mock: Mock = { status, calls: [], handlers: {} };
  mockIPC((cmd, payload) => {
    const args = (payload ?? {}) as Record<string, unknown>;
    mock.calls.push({ cmd, args });
    const override = mock.handlers[cmd];
    if (override) return override(args);
    switch (cmd) {
      case "vanilla_status":
        return mock.status;
      case "preview_apply_profile":
        return PREVIEW;
      default:
        return undefined;
    }
  });
  return mock;
}

function renderPanel(blockedReason: string | null = null) {
  return renderWithProviders(<VanillaPanel installationId="inst-1" blockedReason={blockedReason} />);
}

describe("VanillaPanel", () => {
  it("renders nothing without managed mods or a restore point", async () => {
    const mock = mockVanilla(NO_VANILLA);
    renderPanel();
    await waitFor(() => expect(mock.calls.some((c) => c.cmd === "vanilla_status")).toBe(true));
    expect(screen.queryByRole("region", { name: "Vanilla mode" })).not.toBeInTheDocument();
  });

  it("previews, warns about online play and disables every mod", async () => {
    const mock = mockVanilla(WITH_MODS);
    mock.handlers.enter_vanilla = () => {
      mock.status = ACTIVE;
      return ACTIVE;
    };
    renderPanel();
    const panel = await screen.findByRole("region", { name: "Vanilla mode" });
    expect(within(panel).getByText("2 of 3 mods in the game folder")).toBeInTheDocument();
    expect(within(panel).queryByRole("button", { name: /Restore my mods/ })).not.toBeInTheDocument();

    await userEvent.click(within(panel).getByRole("button", { name: "Disable all mods (vanilla)…" }));
    const dialog = await screen.findByRole("dialog");
    expect(await within(dialog).findByText("Trainer: enabled → disabled")).toBeInTheDocument();
    expect(within(dialog).getByRole("note")).toHaveTextContent(ONLINE_NOTICE);
    expect(within(dialog).getByText(unmanagedNote(4) ?? "")).toBeInTheDocument();
    expect(mock.calls).toContainEqual({
      cmd: "preview_apply_profile",
      args: { installationId: "inst-1", profileId: "vanilla" },
    });

    await userEvent.click(within(dialog).getByRole("button", { name: "Disable all mods" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(mock.calls.filter((c) => c.cmd === "enter_vanilla")).toEqual([
      { cmd: "enter_vanilla", args: { installationId: "inst-1" } },
    ]);
    expect(await within(panel).findByText("Vanilla mode active")).toBeInTheDocument();
  });

  it("shows the active state and restores from the restore point", async () => {
    const mock = mockVanilla(ACTIVE);
    mock.handlers.restore_from_vanilla = () => {
      mock.status = WITH_MODS;
      return WITH_MODS;
    };
    renderPanel();
    const panel = await screen.findByRole("region", { name: "Vanilla mode" });
    expect(within(panel).getByText("Vanilla mode active")).toBeInTheDocument();
    expect(within(panel).getByText(/Restore point saved .* · 3 mods/)).toBeInTheDocument();
    expect(within(panel).getByRole("link", { name: "Review" })).toHaveAttribute("href", "/review");
    expect(
      within(panel).queryByRole("button", { name: "Disable all mods (vanilla)…" }),
    ).not.toBeInTheDocument();

    await userEvent.click(within(panel).getByRole("button", { name: "Restore my mods…" }));
    const dialog = await screen.findByRole("dialog");
    await within(dialog).findByText("Trainer: enabled → disabled");
    expect(mock.calls).toContainEqual({
      cmd: "preview_apply_profile",
      args: { installationId: "inst-1", profileId: "rp-1" },
    });
    await userEvent.click(within(dialog).getByRole("button", { name: "Restore my mods" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(mock.calls.some((c) => c.cmd === "restore_from_vanilla")).toBe(true);
    expect(await within(panel).findByText("2 of 3 mods in the game folder")).toBeInTheDocument();
  });

  it("blocks both actions while writes are blocked", async () => {
    mockVanilla({ ...ACTIVE, modsInGame: 1, active: false });
    renderPanel("GTA V is running.");
    const panel = await screen.findByRole("region", { name: "Vanilla mode" });
    expect(within(panel).getByRole("button", { name: "Disable all mods (vanilla)…" })).toBeDisabled();
    expect(within(panel).getByRole("button", { name: "Restore my mods…" })).toBeDisabled();
  });

  it("keeps the dialog open and shows the error when the operation fails", async () => {
    const mock = mockVanilla(WITH_MODS);
    mock.handlers.enter_vanilla = () => {
      throw APP_ERROR;
    };
    renderPanel();
    const panel = await screen.findByRole("region", { name: "Vanilla mode" });
    await userEvent.click(within(panel).getByRole("button", { name: "Disable all mods (vanilla)…" }));
    const dialog = await screen.findByRole("dialog");
    await within(dialog).findByText("Trainer: enabled → disabled");
    await userEvent.click(within(dialog).getByRole("button", { name: "Disable all mods" }));
    expect(await within(dialog).findByText(APP_ERROR.message)).toBeInTheDocument();
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });
});

describe("unmanagedNote", () => {
  it("is empty when nothing is left over", () => {
    expect(unmanagedNote(null)).toBeNull();
    expect(unmanagedNote(0)).toBeNull();
    expect(unmanagedNote(1)).toMatch(/^1 file the app/);
    expect(unmanagedNote(5)).toMatch(/^5 files the app/);
  });
});
