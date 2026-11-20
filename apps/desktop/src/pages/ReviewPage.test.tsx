import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { InventoryFilterDto } from "../bindings/InventoryFilterDto";
import type { InventoryPageDto } from "../bindings/InventoryPageDto";
import { makeInstallation, renderWithProviders } from "../features/installations/testing";
import { makeItem, makeSummary } from "../features/inventory/testing";
import { ReviewPage } from "./ReviewPage";

vi.mock("@tauri-apps/api/event", () => import("../lib/eventMock"));

/** Renders the Review page and opens the All files tab. */
async function renderAllFiles() {
  renderWithProviders(<ReviewPage />);
  await userEvent.click(await screen.findByRole("tab", { name: "All files" }));
}

const INSTALLATION = makeInstallation();

const ITEMS = [
  makeItem(),
  makeItem({
    relPath: "scripts/trainer.asi",
    size: 5 * 1024 * 1024,
    classification: "candidate",
    suggestedCategory: "scripts_gameplay",
    evidence: [{ kind: "mod_pattern", pattern: "ASI plugin" }, { kind: "hard_linked" }],
  }),
  makeItem({
    relPath: "launcher.exe",
    classification: "original",
    evidence: [{ kind: "signed_by", publisher: "Rockstar Games, Inc." }],
  }),
  makeItem({
    relPath: "update/update.rpf",
    classification: "original_modified",
    evidence: [{ kind: "catalog_original" }, { kind: "baseline_mismatch" }],
  }),
];

interface Calls {
  list: Array<{ id: string; filter: InventoryFilterDto }>;
}

function setup(opts: { selected?: boolean; total?: number; items?: typeof ITEMS } = {}) {
  const calls: Calls = { list: [] };
  const items = opts.items ?? ITEMS;
  mockIPC((cmd, args) => {
    if (cmd === "get_selected_installation") return opts.selected === false ? null : INSTALLATION;
    if (cmd === "inventory_summary") {
      return makeSummary({
        counts: [
          { classification: "candidate", count: 1 },
          { classification: "original", count: 2 },
          { classification: "original_modified", count: 1 },
        ],
        lastScan: {
          id: "s",
          startedAt: "2026-01-05T15:04:00Z",
          finishedAt: "2026-01-05T15:05:00Z",
          status: "completed",
          filesSeen: 4,
          error: null,
        },
      });
    }
    if (cmd === "list_inventory") {
      const a = args as { id: string; filter: InventoryFilterDto };
      calls.list.push(a);
      const page: InventoryPageDto = { items, total: opts.total ?? items.length };
      return page;
    }
    return undefined;
  });
  return calls;
}

afterEach(() => clearMocks());

describe("ReviewPage", () => {
  it("opens on the Detected mods tab", async () => {
    setup();
    renderWithProviders(<ReviewPage />);
    const tab = await screen.findByRole("tab", { name: "Detected mods" });
    expect(tab).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tabpanel", { name: "Detected mods" })).toBeInTheDocument();
  });

  it("links to Installations when there is no active installation", async () => {
    setup({ selected: false });
    renderWithProviders(<ReviewPage />);
    const link = await screen.findByRole("link", { name: "Go to Installations" });
    expect(link).toHaveAttribute("href", "/installations");
    expect(screen.queryByRole("button", { name: "Scan now" })).not.toBeInTheDocument();
    expect(screen.queryByText(/Planned for milestone/)).not.toBeInTheDocument();
  });

  it("shows the installation, controls, rows with evidence", async () => {
    setup();
    await renderAllFiles();
    expect(await screen.findByText("GTA V Legacy (Steam)")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Scan now" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Capture baseline" })).toBeInTheDocument();
    expect(await screen.findByText("scripts/trainer.asi")).toBeInTheDocument();

    const row = screen.getByText("scripts/trainer.asi").closest("tr");
    expect(row).not.toBeNull();
    const r = within(row as HTMLElement);
    expect(r.getByText("Candidate")).toBeInTheDocument();
    expect(r.getByText("Scripts & Gameplay")).toBeInTheDocument();
    expect(r.getByText("5.0 MB")).toBeInTheDocument();
    expect(r.getByText("Matches: ASI plugin · Hard-linked")).toBeInTheDocument();

    expect(screen.getByText("In game catalog")).toBeInTheDocument();
    expect(screen.getByText("Signed by Rockstar Games, Inc.")).toBeInTheDocument();
    expect(screen.getByText("In game catalog · Differs from baseline")).toBeInTheDocument();
    expect(screen.queryByText(/arrives in milestone/)).not.toBeInTheDocument();
  });

  it("shows one chip per classification with its count", async () => {
    setup();
    await renderAllFiles();
    expect(await screen.findByRole("button", { name: "Original 2" })).toBeInTheDocument();
    for (const name of [
      "Modified original 1",
      "Candidate 1",
      "Unknown 0",
      "Managed 0",
      "Drifted 0",
      "Engine file 0",
    ]) {
      expect(screen.getByRole("button", { name })).toBeInTheDocument();
    }
  });

  it("sends the selected classifications and resets the page", async () => {
    const calls = setup({ total: 250 });
    await renderAllFiles();
    await screen.findByText("scripts/trainer.asi");
    expect(calls.list[0]?.filter).toEqual({
      classifications: [],
      search: null,
      limit: 100,
      offset: 0,
    });

    await userEvent.click(screen.getByRole("button", { name: "Next" }));
    await waitFor(() => expect(calls.list.at(-1)?.filter.offset).toBe(100));

    await userEvent.click(screen.getByRole("button", { name: "Candidate 1" }));
    await waitFor(() =>
      expect(calls.list.at(-1)?.filter).toEqual({
        classifications: ["candidate"],
        search: null,
        limit: 100,
        offset: 0,
      }),
    );
    expect(screen.getByRole("button", { name: "Candidate 1" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );

    await userEvent.click(screen.getByRole("button", { name: "Original 2" }));
    await waitFor(() =>
      expect(calls.list.at(-1)?.filter.classifications).toEqual(["candidate", "original"]),
    );
    await userEvent.click(screen.getByRole("button", { name: "Candidate 1" }));
    await waitFor(() => expect(calls.list.at(-1)?.filter.classifications).toEqual(["original"]));
  });

  it("debounces the search before querying", async () => {
    const calls = setup();
    await renderAllFiles();
    await screen.findByText("scripts/trainer.asi");
    const before = calls.list.length;
    await userEvent.type(screen.getByRole("searchbox", { name: "Search files" }), "trainer");
    // Typing is faster than the debounce: no query per keystroke.
    expect(calls.list.length).toBe(before);
    await waitFor(() => expect(calls.list.at(-1)?.filter.search).toBe("trainer"));
    expect(calls.list.length).toBe(before + 1);
  });

  it("paginates 100 rows per page", async () => {
    setup({ total: 250 });
    await renderAllFiles();
    expect(await screen.findByText("Showing 1–4 of 250")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Previous" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Next" })).toBeEnabled();
  });

  it("shows an empty state before the first scan", async () => {
    mockIPC((cmd) => {
      if (cmd === "get_selected_installation") return INSTALLATION;
      if (cmd === "inventory_summary") return makeSummary();
      if (cmd === "list_inventory") return { items: [], total: 0 };
      return undefined;
    });
    await renderAllFiles();
    expect(
      await screen.findByText("No scan yet. Scan the installation to see its files."),
    ).toBeInTheDocument();
  });

  it("says when filters match nothing", async () => {
    setup({ items: [] });
    await renderAllFiles();
    await userEvent.click(await screen.findByRole("button", { name: "Candidate 1" }));
    expect(await screen.findByText("No files match the current filters.")).toBeInTheDocument();
  });
});
