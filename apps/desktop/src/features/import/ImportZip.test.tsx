import { clearMocks } from "@tauri-apps/api/mocks";
import { open } from "@tauri-apps/plugin-dialog";
import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { Mock } from "vitest";
import type { ImportMappedDto } from "../../bindings/ImportMappedDto";
import type { ImportPreviewDto } from "../../bindings/ImportPreviewDto";
import { emitProgress } from "../../lib/eventMock";
import { renderWithProviders } from "../installations/testing";
import { deferred } from "../inventory/testing";
import { ModsView } from "../mods/ModsView";
import { makeMod, mockModsIpc } from "../mods/testing";
import type { ModsMock } from "../mods/testing";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => import("../../lib/eventMock"));
const openMock = open as unknown as Mock;

afterEach(() => {
  clearMocks();
  openMock.mockReset();
});

function target(over: Partial<ImportMappedDto> = {}): ImportMappedDto {
  return {
    targetPath: "scripts/a.asi",
    size: 1024,
    classification: "absent",
    conflict: false,
    protected: false,
    ...over,
  };
}

function preview(over: Partial<ImportPreviewDto> = {}): ImportPreviewDto {
  return {
    stagingId: "stage-1",
    archiveName: "cool-mod.zip",
    files: [
      { archivePath: "Cool/scripts/a.asi", size: 1024 },
      { archivePath: "Cool/readme.txt", size: 10 },
    ],
    suggestedRoot: "Cool",
    root: "Cool",
    mapped: [target()],
    suggestedName: "cool-mod",
    suggestedCategory: "scripts_gameplay",
    ...over,
  };
}

function setup(first: ImportPreviewDto = preview()): ModsMock {
  const mock = mockModsIpc({ mods: [makeMod()] });
  mock.handlers.stage_import = () => first;
  openMock.mockResolvedValue("/tmp/cool-mod.zip");
  renderWithProviders(<ModsView installationId="inst-1" name="GTA V" />);
  return mock;
}

async function startImport() {
  await userEvent.click(await screen.findByRole("button", { name: "Import ZIP…" }));
  return screen.findByRole("dialog", { name: "Import cool-mod.zip" });
}

describe("Import ZIP", () => {
  it("opens a ZIP-filtered picker, stages the archive and preselects the suggested root", async () => {
    const mock = setup();
    const dialog = await startImport();
    expect(openMock).toHaveBeenCalledWith({
      multiple: false,
      directory: false,
      filters: [{ name: "ZIP archive", extensions: ["zip"] }],
    });
    expect(mock.calls.find((c) => c.cmd === "stage_import")?.args).toEqual({
      installationId: "inst-1",
      zipPath: "/tmp/cool-mod.zip",
    });
    const root = within(dialog).getByLabelText("Install root");
    expect(root).toHaveValue("Cool");
    expect(within(root).getAllByRole("option").map((o) => o.textContent)).toEqual([
      "(archive root)",
      "Cool",
      "Cool/scripts",
    ]);
    expect(within(dialog).getByLabelText("Name")).toHaveValue("cool-mod");
    expect(within(dialog).getByText("New")).toBeInTheDocument();
    expect(within(dialog).getByRole("checkbox", { name: "Enable after import" })).toBeChecked();
  });

  it("does nothing when the picker is cancelled", async () => {
    const mock = mockModsIpc({ mods: [makeMod()] });
    openMock.mockResolvedValue(null);
    renderWithProviders(<ModsView installationId="inst-1" name="GTA V" />);
    await userEvent.click(await screen.findByRole("button", { name: "Import ZIP…" }));
    await waitFor(() => expect(openMock).toHaveBeenCalled());
    expect(mock.calls.some((c) => c.cmd === "stage_import")).toBe(false);
  });

  it("remaps when the root changes, shows conflict badges and unchecks Enable after import", async () => {
    const mock = setup();
    mock.handlers.remap_import = () =>
      preview({
        root: null,
        mapped: [
          target({ targetPath: "Cool/scripts/a.asi", conflict: true, classification: "unknown" }),
          target({ targetPath: "Cool/readme.txt" }),
        ],
      });
    const dialog = await startImport();
    await userEvent.selectOptions(within(dialog).getByLabelText("Install root"), "");
    await waitFor(() =>
      expect(mock.calls.find((c) => c.cmd === "remap_import")?.args).toEqual({
        stagingId: "stage-1",
        root: null,
      }),
    );
    expect(await within(dialog).findByText("Conflict")).toBeInTheDocument();
    expect(within(dialog).getByText("New")).toBeInTheDocument();
    expect(within(dialog).getByRole("checkbox", { name: "Enable after import" })).not.toBeChecked();
    expect(within(dialog).getByRole("button", { name: "Import" })).toBeEnabled();
  });

  it("blocks Import while a target is a protected game file", async () => {
    const mock = setup(
      preview({ mapped: [target({ targetPath: "GTA5.exe", protected: true, classification: "original" })] }),
    );
    const dialog = await startImport();
    expect(within(dialog).getByText("Game file — blocked")).toBeInTheDocument();
    expect(within(dialog).getByRole("alert")).toHaveTextContent(/cannot be imported/);
    expect(within(dialog).getByRole("button", { name: "Import" })).toBeDisabled();
    expect(mock.calls.some((c) => c.cmd === "confirm_import")).toBe(false);
  });

  it("imports with the edited fields and enableNow, then closes and refreshes the list", async () => {
    const mock = setup();
    mock.handlers.confirm_import = () => makeMod({ id: "new", name: "Cool Mod" });
    const dialog = await startImport();
    const name = within(dialog).getByLabelText("Name");
    await userEvent.clear(name);
    await userEvent.type(name, "Cool Mod");
    await userEvent.selectOptions(within(dialog).getByLabelText("Category"), "vehicles");
    await userEvent.type(within(dialog).getByLabelText("Notes"), "from forum");
    const listCalls = mock.calls.filter((c) => c.cmd === "list_mods").length;
    await userEvent.click(within(dialog).getByRole("button", { name: "Import" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(mock.calls.find((c) => c.cmd === "confirm_import")?.args).toEqual({
      req: {
        stagingId: "stage-1",
        root: "Cool",
        name: "Cool Mod",
        category: "vehicles",
        notes: "from forum",
        enableNow: true,
      },
    });
    await waitFor(() =>
      expect(mock.calls.filter((c) => c.cmd === "list_mods").length).toBeGreaterThan(listCalls),
    );
    expect(mock.calls.some((c) => c.cmd === "discard_import")).toBe(false);
  });

  it("sends enableNow false when the user unchecks it", async () => {
    const mock = setup();
    mock.handlers.confirm_import = () => makeMod({ id: "new" });
    const dialog = await startImport();
    await userEvent.click(within(dialog).getByRole("checkbox", { name: "Enable after import" }));
    await userEvent.click(within(dialog).getByRole("button", { name: "Import" }));
    await waitFor(() => expect(mock.calls.some((c) => c.cmd === "confirm_import")).toBe(true));
    const req = mock.calls.find((c) => c.cmd === "confirm_import")?.args.req as { enableNow: boolean };
    expect(req.enableNow).toBe(false);
  });

  it("Cancel discards the staged archive", async () => {
    const mock = setup();
    const dialog = await startImport();
    await userEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(mock.calls.find((c) => c.cmd === "discard_import")?.args).toEqual({
      stagingId: "stage-1",
    });
    expect(mock.calls.some((c) => c.cmd === "confirm_import")).toBe(false);
  });

  it("shows code, message and hint when the import fails and keeps the dialog", async () => {
    const mock = setup();
    mock.handlers.confirm_import = () =>
      Promise.reject({
        code: "INVALID_INPUT",
        message: "A mod with this name already exists.",
        hint: "Choose another name.",
        details: null,
      });
    const dialog = await startImport();
    await userEvent.click(within(dialog).getByRole("button", { name: "Import" }));
    expect(await within(dialog).findByText("INVALID_INPUT")).toBeInTheDocument();
    expect(within(dialog).getByText("A mod with this name already exists.")).toBeInTheDocument();
    expect(within(dialog).getByText("Choose another name.")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Import" })).toBeEnabled();
  });

  it("offers only Close after the mod was imported but not enabled", async () => {
    const mock = setup();
    mock.handlers.confirm_import = () =>
      Promise.reject({
        code: "DRIFT_DETECTED",
        message: "Files changed.",
        hint: "The mod was imported but not enabled.",
        details: null,
      });
    const dialog = await startImport();
    await userEvent.click(within(dialog).getByRole("button", { name: "Import" }));
    expect(await within(dialog).findByText("DRIFT_DETECTED")).toBeInTheDocument();
    expect(within(dialog).queryByRole("button", { name: "Import" })).not.toBeInTheDocument();
    await userEvent.click(within(dialog).getByRole("button", { name: "Close" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(mock.calls.some((c) => c.cmd === "discard_import")).toBe(false);
  });

  it("shows progress while staging and Cancel stops it without an error", async () => {
    const gate = deferred<ImportPreviewDto>();
    const mock = mockModsIpc({ mods: [makeMod()] });
    mock.handlers.stage_import = () => gate.promise;
    mock.handlers.cancel_operation = () => {
      gate.reject({ code: "CANCELLED", message: "Cancelled.", hint: null, details: null });
    };
    openMock.mockResolvedValue("/tmp/big.zip");
    renderWithProviders(<ModsView installationId="inst-1" name="GTA V" />);
    await userEvent.click(await screen.findByRole("button", { name: "Import ZIP…" }));
    const dialog = await screen.findByRole("dialog", { name: "Reading archive" });
    act(() =>
      emitProgress({
        operationId: "op",
        kind: "import",
        filesSeen: 42,
        bytesHashed: 0,
        current: null,
        done: false,
      }),
    );
    expect(await within(dialog).findByText("42 files read…")).toBeInTheDocument();
    await userEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(mock.calls.some((c) => c.cmd === "cancel_operation")).toBe(true);
    expect(screen.queryByText("CANCELLED")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Import ZIP…" })).toBeEnabled();
  });

  it("shows a staging error with code and message", async () => {
    const mock = mockModsIpc({ mods: [makeMod()] });
    mock.handlers.stage_import = () =>
      Promise.reject({ code: "INVALID_INPUT", message: "Not a valid ZIP.", hint: null, details: null });
    openMock.mockResolvedValue("/tmp/bad.zip");
    renderWithProviders(<ModsView installationId="inst-1" name="GTA V" />);
    await userEvent.click(await screen.findByRole("button", { name: "Import ZIP…" }));
    expect(await screen.findByText("Not a valid ZIP.")).toBeInTheDocument();
  });
});
