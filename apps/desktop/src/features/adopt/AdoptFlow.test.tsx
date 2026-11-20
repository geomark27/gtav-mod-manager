import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { AdoptPreviewDto } from "../../bindings/AdoptPreviewDto";
import type { ModDto } from "../../bindings/ModDto";
import { ReviewPage } from "../../pages/ReviewPage";
import { makeInstallation, renderWithProviders } from "../installations/testing";
import { deferred, makeItem, makeSummary } from "../inventory/testing";
import { makeMod } from "../mods/testing";

vi.mock("@tauri-apps/api/event", () => import("../../lib/eventMock"));

/** Renders the Review page and opens the All files tab. */
async function renderAllFiles() {
  renderWithProviders(<ReviewPage />);
  await userEvent.click(await screen.findByRole("tab", { name: "All files" }));
}

const ITEMS = [
  makeItem({ relPath: "GTA5.exe", classification: "original" }),
  makeItem({ relPath: "scripts/a.asi", classification: "candidate", size: 1024 }),
  makeItem({ relPath: "misc/b.bin", classification: "unknown", size: 2048 }),
  makeItem({ relPath: "mods/c.dll", classification: "managed" }),
  makeItem({ relPath: "x/d.dll", classification: "drifted" }),
];

const PREVIEW: AdoptPreviewDto = {
  files: [{ relPath: "scripts/a.asi", size: 1024, classification: "candidate" }],
  blocked: [],
  totalSize: 1024,
  includesUnknown: false,
  suggestedName: "Suggested Mod",
  suggestedCategory: "scripts_gameplay",
};

interface Setup {
  calls: Array<{ cmd: string; args: Record<string, unknown> }>;
  handlers: Record<string, (args: Record<string, unknown>) => unknown>;
  preview: AdoptPreviewDto;
}

function setup(preview: AdoptPreviewDto = PREVIEW): Setup {
  const s: Setup = { calls: [], handlers: {}, preview };
  mockIPC((cmd, payload) => {
    const args = (payload ?? {}) as Record<string, unknown>;
    s.calls.push({ cmd, args });
    const h = s.handlers[cmd];
    if (h) return h(args);
    if (cmd === "get_selected_installation") return makeInstallation();
    if (cmd === "inventory_summary") return makeSummary();
    if (cmd === "list_inventory") return { items: ITEMS, total: ITEMS.length };
    if (cmd === "preview_adopt") return s.preview;
    return undefined;
  });
  return s;
}

afterEach(() => clearMocks());

describe("selection", () => {
  it("offers checkboxes only on candidate and unknown rows", async () => {
    setup();
    await renderAllFiles();
    await screen.findByText("scripts/a.asi");
    const boxes = screen.getAllByRole("checkbox").map((b) => b.getAttribute("aria-label"));
    expect(boxes).toEqual(["Select scripts/a.asi", "Select misc/b.bin"]);
    expect(screen.queryByRole("button", { name: "Adopt as mod…" })).not.toBeInTheDocument();
  });

  it("shows the count and can clear the selection", async () => {
    setup();
    await renderAllFiles();
    await userEvent.click(await screen.findByRole("checkbox", { name: "Select scripts/a.asi" }));
    await userEvent.click(screen.getByRole("checkbox", { name: "Select misc/b.bin" }));
    expect(screen.getByText("2 selected")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("checkbox", { name: "Select misc/b.bin" }));
    expect(screen.getByText("1 selected")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Clear selection" }));
    expect(screen.queryByText(/selected$/)).not.toBeInTheDocument();
  });
});

describe("adopt dialog", () => {
  async function openDialog() {
    await userEvent.click(await screen.findByRole("checkbox", { name: "Select scripts/a.asi" }));
    await userEvent.click(screen.getByRole("button", { name: "Adopt as mod…" }));
    return screen.findByRole("dialog", { name: "Adopt as mod" });
  }

  it("previews, prefills, adopts, closes, clears the selection and refreshes", async () => {
    const s = setup();
    const gate = deferred<ModDto>();
    s.handlers.adopt_mod = () => gate.promise;
    await renderAllFiles();
    const dialog = await openDialog();
    expect(s.calls.find((c) => c.cmd === "preview_adopt")?.args).toEqual({
      installationId: makeInstallation().id,
      relPaths: ["scripts/a.asi"],
    });
    const name = await within(dialog).findByLabelText("Name");
    expect(name).toHaveValue("Suggested Mod");
    expect(within(dialog).getByLabelText("Category")).toHaveValue("scripts_gameplay");
    expect(within(dialog).getByText("scripts/a.asi")).toBeInTheDocument();
    expect(within(dialog).getByText("1.0 KB")).toBeInTheDocument();
    expect(within(dialog).queryByRole("checkbox")).not.toBeInTheDocument();

    await userEvent.clear(name);
    expect(within(dialog).getByRole("button", { name: "Adopt" })).toBeDisabled();
    await userEvent.type(name, "My Mod");
    await userEvent.selectOptions(within(dialog).getByLabelText("Category"), "vehicles");
    await userEvent.type(within(dialog).getByLabelText("Notes"), "note");
    const listsBefore = s.calls.filter((c) => c.cmd === "list_inventory").length;
    await userEvent.click(within(dialog).getByRole("button", { name: "Adopt" }));

    await within(dialog).findByRole("progressbar", { name: "Adopting" });
    expect(within(dialog).queryByRole("button", { name: "Cancel" })).toBeDisabled();
    expect(s.calls.find((c) => c.cmd === "adopt_mod")?.args).toEqual({
      req: {
        installationId: makeInstallation().id,
        relPaths: ["scripts/a.asi"],
        name: "My Mod",
        category: "vehicles",
        notes: "note",
        confirmUnknown: false,
      },
    });

    gate.resolve(makeMod({ name: "My Mod" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(screen.getByText("Mod adopted: My Mod")).toBeInTheDocument();
    expect(screen.queryByText(/selected$/)).not.toBeInTheDocument();
    await waitFor(() =>
      expect(s.calls.filter((c) => c.cmd === "list_inventory").length).toBeGreaterThan(listsBefore),
    );
  });

  it("requires the unknown-files confirmation", async () => {
    const s = setup({
      ...PREVIEW,
      files: [
        { relPath: "scripts/a.asi", size: 1024, classification: "candidate" },
        { relPath: "misc/b.bin", size: 2048, classification: "unknown" },
      ],
      includesUnknown: true,
    });
    s.handlers.adopt_mod = () => makeMod();
    await renderAllFiles();
    const dialog = await openDialog();
    const adopt = await within(dialog).findByRole("button", { name: "Adopt" });
    expect(adopt).toBeDisabled();
    await userEvent.click(
      within(dialog).getByLabelText("I confirm these unknown files are not part of the game."),
    );
    expect(adopt).toBeEnabled();
    await userEvent.click(adopt);
    await waitFor(() =>
      expect(
        (s.calls.find((c) => c.cmd === "adopt_mod")?.args.req as { confirmUnknown: boolean })
          .confirmUnknown,
      ).toBe(true),
    );
  });

  it("lists blocked files with reasons and adopts only the adoptable ones", async () => {
    const s = setup({
      ...PREVIEW,
      blocked: [{ relPath: "scripts/z.asi", reason: "Already managed by another mod." }],
    });
    s.handlers.adopt_mod = () => makeMod();
    await renderAllFiles();
    const dialog = await openDialog();
    const blocked = await within(dialog).findByRole("list", { name: "Blocked files" });
    expect(within(blocked).getByText("scripts/z.asi")).toBeInTheDocument();
    expect(within(blocked).getByText("Already managed by another mod.")).toBeInTheDocument();
    await userEvent.click(within(dialog).getByRole("button", { name: "Adopt" }));
    await waitFor(() =>
      expect(
        (s.calls.find((c) => c.cmd === "adopt_mod")?.args.req as { relPaths: string[] }).relPaths,
      ).toEqual(["scripts/a.asi"]),
    );
  });

  it("cannot adopt when every file is blocked", async () => {
    setup({
      ...PREVIEW,
      files: [],
      blocked: [{ relPath: "scripts/a.asi", reason: "Is a protected file." }],
      totalSize: 0,
    });
    await renderAllFiles();
    const dialog = await openDialog();
    expect(await within(dialog).findByText("None of the selected files can be adopted.")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Adopt" })).toBeDisabled();
  });

  it("shows an adopt error and keeps the dialog and selection", async () => {
    const s = setup();
    s.handlers.adopt_mod = () =>
      Promise.reject({ code: "GAME_RUNNING", message: "The game is running.", hint: "Close it.", details: null });
    await renderAllFiles();
    const dialog = await openDialog();
    await userEvent.click(await within(dialog).findByRole("button", { name: "Adopt" }));
    expect(await within(dialog).findByText("GAME_RUNNING")).toBeInTheDocument();
    expect(within(dialog).getByText("Close it.")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Adopt" })).toBeEnabled();
    await userEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(screen.getByText("1 selected")).toBeInTheDocument();
  });

  it("shows a preview error", async () => {
    const s = setup();
    s.handlers.preview_adopt = () =>
      Promise.reject({ code: "INVALID_INPUT", message: "Nothing selected.", hint: null, details: null });
    await renderAllFiles();
    const dialog = await openDialog();
    expect(await within(dialog).findByText("Nothing selected.")).toBeInTheDocument();
    expect(within(dialog).queryByRole("button", { name: "Adopt" })).not.toBeInTheDocument();
  });
});
