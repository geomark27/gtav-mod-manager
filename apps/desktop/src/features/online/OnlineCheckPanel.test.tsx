import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { OnlineCheckDto } from "../../bindings/OnlineCheckDto";
import type { ReadinessCheckDto } from "../../bindings/ReadinessCheckDto";
import { ONLINE_NOTICE } from "../../layout/AppShell";
import { renderWithProviders } from "../installations/testing";
import { APP_ERROR } from "../mods/testing";
import { describeCheck, verdictText } from "./labels";
import { OnlineCheckPanel } from "./OnlineCheckPanel";

afterEach(() => clearMocks());

const pass = (id: ReadinessCheckDto["id"]): ReadinessCheckDto => ({ id, status: "pass", count: 0, items: [] });

function makeResult(over: Partial<OnlineCheckDto> = {}): OnlineCheckDto {
  return {
    checkedAt: "2026-11-19T20:00:00Z",
    scannedAt: "2026-11-19T20:00:00Z",
    verdict: "clear",
    checks: [
      pass("game_closed"),
      pass("recovery"),
      pass("managed_mods"),
      pass("mod_files"),
      pass("unknown_files"),
      pass("originals"),
      pass("leftovers"),
    ],
    baseline: "trusted",
    vanillaApplied: false,
    actionError: null,
    ...over,
  };
}

const BLOCKED = makeResult({
  verdict: "blocked",
  checks: [
    pass("game_closed"),
    pass("recovery"),
    pass("managed_mods"),
    { id: "mod_files", status: "fail", count: 2, items: ["trainer.asi", "dinput8.dll (in the baseline)"] },
    { id: "unknown_files", status: "warn", count: 1, items: ["notes.txt"] },
    { id: "originals", status: "fail", count: 1, items: ["update/update.rpf"] },
    pass("leftovers"),
  ],
  vanillaApplied: true,
});

interface Mock {
  calls: string[];
  handlers: Record<string, () => unknown>;
}

function mock(): Mock {
  const m: Mock = { calls: [], handlers: {} };
  mockIPC((cmd) => {
    m.calls.push(cmd);
    const h = m.handlers[cmd];
    return h ? h() : undefined;
  });
  return m;
}

function renderPanel(blockedReason: string | null = null) {
  return renderWithProviders(<OnlineCheckPanel installationId="inst-1" blockedReason={blockedReason} />);
}

describe("OnlineCheckPanel", () => {
  it("starts with the two actions and the online notice, without running anything", () => {
    const m = mock();
    renderPanel();
    const card = screen.getByRole("region", { name: "Before playing online" });
    expect(within(card).getByRole("button", { name: "Prepare for online" })).toBeEnabled();
    expect(within(card).getByRole("button", { name: "Check only" })).toBeEnabled();
    expect(within(card).getByRole("note")).toHaveTextContent(ONLINE_NOTICE);
    expect(m.calls).toEqual([]);
  });

  it("prepares for online and lists what is left with how to fix it", async () => {
    const m = mock();
    m.handlers.prepare_for_online = () => BLOCKED;
    renderPanel();
    await userEvent.click(screen.getByRole("button", { name: "Prepare for online" }));
    expect(await screen.findByText("2 things to fix")).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("Every managed mod was disabled");
    const files = screen.getByRole("listitem", { name: "No other mod files in the game folder" });
    expect(within(files).getByText("Fix")).toBeInTheDocument();
    expect(within(files).getByText("trainer.asi")).toBeInTheDocument();
    expect(within(files).getByText("dinput8.dll (in the baseline)")).toBeInTheDocument();
    expect(within(files).getByRole("link", { name: "Open Review" })).toHaveAttribute("href", "/review");
    const originals = screen.getByRole("listitem", { name: "Original game files are unchanged" });
    expect(within(originals).getByText(/Verify the game files in Steam/)).toBeInTheDocument();
    const unknown = screen.getByRole("listitem", { name: "No unrecognized files" });
    expect(within(unknown).getByText("Review")).toBeInTheDocument();
    expect(m.calls).toContain("prepare_for_online");
  });

  it("check only reports a clear result without claiming online safety", async () => {
    const m = mock();
    m.handlers.check_online_readiness = () => makeResult();
    renderPanel();
    await userEvent.click(screen.getByRole("button", { name: "Check only" }));
    const verdict = await screen.findByLabelText("Online check result");
    expect(verdict).toHaveTextContent("No mod files found");
    expect(verdict).toHaveTextContent("not a guarantee for GTA Online");
    expect(verdict).not.toHaveTextContent(/safe/i);
    expect(m.calls).toEqual(["check_online_readiness"]);
  });

  it("shows why the mods were not disabled and still shows the checks", async () => {
    const m = mock();
    m.handlers.prepare_for_online = () =>
      makeResult({
        verdict: "blocked",
        actionError: { ...APP_ERROR, code: "GAME_RUNNING", message: "GTA V or one of its launchers is running." },
        checks: [{ id: "game_closed", status: "fail", count: 1, items: ["GTA5.exe"] }],
      });
    renderPanel();
    await userEvent.click(screen.getByRole("button", { name: "Prepare for online" }));
    expect(await screen.findByText("The mods were not disabled:")).toBeInTheDocument();
    expect(screen.getByText("GTA V or one of its launchers is running.")).toBeInTheDocument();
    expect(screen.getByText("Running: GTA5.exe.")).toBeInTheDocument();
  });

  it("shows a failed command as an error", async () => {
    const m = mock();
    m.handlers.check_online_readiness = () => {
      throw APP_ERROR;
    };
    renderPanel();
    await userEvent.click(screen.getByRole("button", { name: "Check only" }));
    expect(await screen.findByText(APP_ERROR.message)).toBeInTheDocument();
  });

  it("blocks prepare while writes are blocked but still allows check only", async () => {
    mock();
    renderPanel("GTA V is running.");
    const prepare = screen.getByRole("button", { name: "Prepare for online" });
    expect(prepare).toBeDisabled();
    expect(prepare).toHaveAttribute("title", "GTA V is running.");
    await waitFor(() => expect(screen.getByRole("button", { name: "Check only" })).toBeEnabled());
  });
});

describe("labels", () => {
  it("words each verdict", () => {
    expect(verdictText(makeResult()).title).toBe("No mod files found");
    expect(verdictText(BLOCKED).title).toBe("2 things to fix");
    const review = makeResult({ verdict: "review", checks: [{ id: "leftovers", status: "warn", count: 1, items: ["a"] }] });
    expect(verdictText(review).title).toBe("1 thing to review");
  });

  it("explains a missing baseline and an untrusted one", () => {
    const missing = describeCheck({ id: "originals", status: "warn", count: 0, items: [] }, { baseline: "none" });
    expect(missing.detail).toMatch(/no baseline/);
    expect(missing.link?.to).toBe("/review");
    const untrusted = describeCheck(pass("originals"), { baseline: "untrusted" });
    expect(untrusted.detail).toMatch(/not marked as trusted/);
  });

  it("sends drifted mods to the Mods page", () => {
    const d = describeCheck(
      { id: "managed_mods", status: "fail", count: 1, items: ["Trainer (drifted)"] },
      { baseline: "trusted" },
    );
    expect(d.link?.to).toBe("/mods");
    const plain = describeCheck(
      { id: "managed_mods", status: "fail", count: 2, items: ["A", "B"] },
      { baseline: "trusted" },
    );
    expect(plain.detail).toBe("2 mods have files in the game folder.");
    expect(plain.link).toBeNull();
  });
});
