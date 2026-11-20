import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { AdoptDetectedResultDto } from "../../bindings/AdoptDetectedResultDto";
import type { DetectedModDto } from "../../bindings/DetectedModDto";
import type { DetectedModsDto } from "../../bindings/DetectedModsDto";
import { renderWithProviders } from "../installations/testing";
import { APP_ERROR, makeMod } from "../mods/testing";
import { DetectedModsPanel } from "./DetectedModsPanel";
import { requirementInfo } from "./labels";

afterEach(() => clearMocks());

function makeProposal(over: Partial<DetectedModDto> = {}): DetectedModDto {
  return {
    key: "recognized:script-hook-v",
    sourceKind: "recognized",
    signatureId: "script-hook-v",
    name: "Script Hook V",
    category: "scripts_gameplay",
    confidence: "high",
    files: [{ relPath: "ScriptHookV.dll", size: 1024, classification: "candidate" }],
    totalSize: 1024,
    includesUnknown: false,
    requires: [],
    editionSupported: true,
    nameTaken: false,
    ...over,
  };
}

const SHV = makeProposal();
const RAMPAGE = makeProposal({
  key: "recognized:rampage-trainer",
  signatureId: "rampage-trainer",
  name: "Rampage Trainer",
  confidence: "medium",
  files: [
    { relPath: "scripts/Rampage/config.ini", size: 10, classification: "unknown" },
    { relPath: "scripts/RampageTrainer.dll", size: 2048, classification: "candidate" },
  ],
  totalSize: 2058,
  includesUnknown: true,
  requires: [
    { signatureId: "script-hook-v-dotnet", name: "Script Hook V .NET", status: "missing", modName: null },
  ],
});
const GROUP = makeProposal({
  key: "grouped:fasttravel.asi",
  sourceKind: "grouped",
  signatureId: null,
  name: "FastTravel",
  confidence: "low",
  files: [{ relPath: "FastTravel.asi", size: 5, classification: "candidate" }],
  totalSize: 5,
});

const DATA: DetectedModsDto = {
  scannedAt: "2026-11-19T10:00:00Z",
  proposals: [SHV, RAMPAGE, GROUP],
  ambiguous: [{ relPath: "d3d11.dll", candidates: ["ENBSeries", "ReShade"] }],
};

interface Mock {
  data: DetectedModsDto;
  calls: Array<{ cmd: string; args: Record<string, unknown> }>;
  handlers: Record<string, (args: Record<string, unknown>) => unknown>;
}

function mockDetect(data: DetectedModsDto = DATA): Mock {
  const mock: Mock = { data, calls: [], handlers: {} };
  mockIPC((cmd, payload) => {
    const args = (payload ?? {}) as Record<string, unknown>;
    mock.calls.push({ cmd, args });
    const override = mock.handlers[cmd];
    if (override) return override(args);
    return cmd === "detect_mods" ? mock.data : undefined;
  });
  return mock;
}

function renderPanel(blockedReason: string | null = null) {
  return renderWithProviders(
    <DetectedModsPanel installationId="inst-1" blockedReason={blockedReason} />,
  );
}

const card = (name: string) => screen.findByRole("listitem", { name });

describe("DetectedModsPanel", () => {
  it("shows each proposal with confidence, requirements and files", async () => {
    mockDetect();
    renderPanel();
    const shv = await card("Script Hook V");
    expect(within(shv).getByText("Known mod")).toBeInTheDocument();
    expect(within(shv).getByRole("checkbox", { name: "Include Script Hook V" })).toBeChecked();
    const rampage = await card("Rampage Trainer");
    expect(within(rampage).getByText("Likely match")).toBeInTheDocument();
    expect(within(rampage).getByText("Requires Script Hook V .NET: not found")).toBeInTheDocument();
    expect(within(rampage).getByText("scripts/RampageTrainer.dll")).toBeInTheDocument();
    expect(within(rampage).getByText("Unknown")).toBeInTheDocument();
    const group = await card("FastTravel");
    expect(within(group).getByText("Grouped")).toBeInTheDocument();
    expect(within(group).getByRole("checkbox", { name: "Include FastTravel" })).not.toBeChecked();
    expect(screen.getByText("d3d11.dll")).toBeInTheDocument();
    expect(screen.getByText("(ENBSeries, ReShade)")).toBeInTheDocument();
  });

  it("asks for the unknown-files confirmation, then adopts the edited selection", async () => {
    const mock = mockDetect();
    const result: AdoptDetectedResultDto = {
      adopted: [makeMod({ id: "m1", name: "SHV" }), makeMod({ id: "m2", name: "Rampage Trainer" })],
      skipped: [],
      dependenciesLinked: 1,
    };
    mock.handlers.adopt_detected = () => {
      mock.data = { ...DATA, proposals: [GROUP] };
      return result;
    };
    renderPanel();
    const adopt = await screen.findByRole("button", { name: "Adopt selected (2)" });
    expect(adopt).toBeDisabled();
    await userEvent.click(screen.getByRole("checkbox", { name: /are mine to adopt/ }));
    expect(adopt).toBeEnabled();

    const shv = await card("Script Hook V");
    const name = within(shv).getByLabelText("Name");
    await userEvent.clear(name);
    await userEvent.type(name, "SHV");
    await userEvent.click(adopt);

    await waitFor(() =>
      expect(mock.calls.find((c) => c.cmd === "adopt_detected")?.args).toEqual({
        req: {
          installationId: "inst-1",
          confirmUnknown: true,
          items: [
            {
              signatureId: "script-hook-v",
              name: "SHV",
              category: "scripts_gameplay",
              relPaths: ["ScriptHookV.dll"],
            },
            {
              signatureId: "rampage-trainer",
              name: "Rampage Trainer",
              category: "scripts_gameplay",
              relPaths: ["scripts/Rampage/config.ini", "scripts/RampageTrainer.dll"],
            },
          ],
        },
      }),
    );
    const status = await screen.findByRole("status");
    expect(status).toHaveTextContent("Adopted 2 mods: SHV, Rampage Trainer · 1 dependency linked");
    // The refreshed proposals replace the adopted ones.
    await waitFor(() =>
      expect(screen.queryByRole("listitem", { name: "Script Hook V" })).not.toBeInTheDocument(),
    );
    expect(await card("FastTravel")).toBeInTheDocument();
  });

  it("reports the proposals that were skipped", async () => {
    const mock = mockDetect({ ...DATA, proposals: [SHV] });
    mock.handlers.adopt_detected = () =>
      ({
        adopted: [],
        skipped: [{ name: "Script Hook V", error: { ...APP_ERROR, message: "Name taken." } }],
        dependenciesLinked: 0,
      }) satisfies AdoptDetectedResultDto;
    renderPanel();
    await userEvent.click(await screen.findByRole("button", { name: "Adopt selected (1)" }));
    const status = await screen.findByRole("status");
    expect(status).toHaveTextContent("Not adopted:");
    expect(status).toHaveTextContent("Script Hook V: Name taken. Check again first.");
  });

  it("refuses empty, duplicate or blocked selections", async () => {
    mockDetect({ ...DATA, proposals: [SHV, GROUP] });
    renderPanel();
    const adopt = await screen.findByRole("button", { name: "Adopt selected (1)" });
    await userEvent.click(screen.getByRole("checkbox", { name: "Include Script Hook V" }));
    expect(screen.getByRole("button", { name: "Adopt selected (0)" })).toBeDisabled();
    await userEvent.click(screen.getByRole("checkbox", { name: "Include Script Hook V" }));
    await userEvent.click(screen.getByRole("checkbox", { name: "Include FastTravel" }));
    const group = await card("FastTravel");
    const name = within(group).getByLabelText("Name");
    await userEvent.clear(name);
    await userEvent.type(name, "script hook v");
    expect(screen.getByRole("button", { name: "Adopt selected (2)" })).toHaveAttribute(
      "title",
      "Two selected mods have the same name.",
    );
    await userEvent.clear(name);
    expect(screen.getByRole("button", { name: "Adopt selected (2)" })).toBeDisabled();
    expect(adopt).toBeDefined();
  });

  it("is blocked while the game runs", async () => {
    mockDetect({ ...DATA, proposals: [SHV] });
    renderPanel("GTA V is running.");
    const adopt = await screen.findByRole("button", { name: "Adopt selected (1)" });
    expect(adopt).toBeDisabled();
    expect(adopt).toHaveAttribute("title", "GTA V is running.");
  });

  it("shows warnings and leaves taken names unselected", async () => {
    mockDetect({
      ...DATA,
      proposals: [makeProposal({ editionSupported: false, nameTaken: true })],
    });
    renderPanel();
    const shv = await card("Script Hook V");
    expect(within(shv).getByText(/not made for this edition/)).toBeInTheDocument();
    expect(within(shv).getByText(/already has this name/)).toBeInTheDocument();
    expect(within(shv).getByRole("checkbox", { name: "Include Script Hook V" })).not.toBeChecked();
  });

  it("explains the empty states", async () => {
    mockDetect({ scannedAt: null, proposals: [], ambiguous: [] });
    const { unmount } = renderPanel();
    expect(await screen.findByText(/Scan the installation to detect/)).toBeInTheDocument();
    unmount();
    clearMocks();
    mockDetect({ scannedAt: "2026-11-19T10:00:00Z", proposals: [], ambiguous: [] });
    renderPanel();
    expect(await screen.findByText(/No unmanaged mod files/)).toBeInTheDocument();
  });
});

describe("requirementInfo", () => {
  it("describes each status", () => {
    const base = { signatureId: "x", name: "X", modName: null };
    expect(requirementInfo({ ...base, status: "managed", modName: "My X" })).toEqual({
      text: "X: managed as “My X”",
      tone: "success",
    });
    expect(requirementInfo({ ...base, status: "detected" }).tone).toBe("info");
    expect(requirementInfo({ ...base, status: "missing" }).text).toBe("X: not found");
  });
});
