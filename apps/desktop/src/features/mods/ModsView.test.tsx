import { clearMocks } from "@tauri-apps/api/mocks";
import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ModDto } from "../../bindings/ModDto";
import { deferred } from "../inventory/testing";
import { renderWithProviders } from "../installations/testing";
import { ModsView } from "./ModsView";
import { APP_ERROR, makeMod, mockModsIpc } from "./testing";

const ID = "inst-1";

function renderView() {
  return renderWithProviders(<ModsView installationId={ID} name="GTA V Legacy (Steam)" />);
}

afterEach(() => clearMocks());

function sw(name: string) {
  return screen.findByRole("switch", { name });
}

describe("ModsView switches (D-11)", () => {
  it("reconciles before listing and shows the installation", async () => {
    const mock = mockModsIpc({ mods: [makeMod()] });
    renderView();
    expect(await screen.findByText("Alpha")).toBeInTheDocument();
    expect(screen.getByText("GTA V Legacy (Steam)")).toBeInTheDocument();
    const cmds = mock.calls.map((c) => c.cmd);
    expect(cmds.indexOf("reconcile_installation")).toBeGreaterThanOrEqual(0);
    expect(cmds.indexOf("reconcile_installation")).toBeLessThan(cmds.indexOf("list_mods"));
  });

  it("does not change before the command resolves, is pending meanwhile, then follows the server", async () => {
    const gate = deferred<ModDto>();
    const mock = mockModsIpc({ mods: [makeMod()] });
    mock.handlers.set_mod_enabled = () => gate.promise;
    renderView();
    const toggle = await sw("Alpha");
    expect(toggle).not.toBeChecked();

    await userEvent.click(toggle);
    await waitFor(() => expect(toggle).toBeDisabled());
    expect(toggle).toHaveAttribute("aria-busy", "true");
    expect(toggle).not.toBeChecked();
    expect(mock.calls.filter((c) => c.cmd === "set_mod_enabled")).toHaveLength(1);
    expect(mock.calls.find((c) => c.cmd === "set_mod_enabled")?.args).toEqual({
      modId: "mod-1",
      enabled: true,
    });

    const updated = makeMod({ state: "enabled" });
    mock.mods = [updated];
    gate.resolve(updated);
    await waitFor(() => expect(toggle).toBeChecked());
    expect(toggle).toBeEnabled();
    expect(screen.getByRole("listitem")).toHaveTextContent("Enabled");
  });

  it("follows the returned state even if it differs from the request", async () => {
    const mock = mockModsIpc({ mods: [makeMod({ state: "enabled" })] });
    const returned = makeMod({ state: "enabled" });
    mock.handlers.set_mod_enabled = () => returned;
    renderView();
    const toggle = await sw("Alpha");
    expect(toggle).toBeChecked();
    await userEvent.click(toggle);
    await waitFor(() =>
      expect(mock.calls.some((c) => c.cmd === "set_mod_enabled")).toBe(true),
    );
    await waitFor(() => expect(toggle).toBeEnabled());
    expect(toggle).toBeChecked();
  });

  it("on error returns to the server state and shows code, message and hint", async () => {
    const mock = mockModsIpc({ mods: [makeMod()] });
    mock.handlers.set_mod_enabled = () => Promise.reject(APP_ERROR);
    renderView();
    const toggle = await sw("Alpha");
    await userEvent.click(toggle);
    const alert = await screen.findByRole("alert");
    expect(within(alert).getByText("DRIFT_DETECTED")).toBeInTheDocument();
    expect(within(alert).getByText("Files changed outside the app.")).toBeInTheDocument();
    expect(within(alert).getByText("Check again first.")).toBeInTheDocument();
    expect(toggle).not.toBeChecked();
    expect(toggle).toBeEnabled();
  });

  it("is blocked, with the reason, when the game is running; no command is sent", async () => {
    const mock = mockModsIpc({
      mods: [makeMod()],
      guard: { gameRunning: ["GTA5.exe"], writable: true, recoveryRequired: false, installationMissing: false },
    });
    renderView();
    expect(
      await screen.findByText(
        "The game or a launcher is running (GTA5.exe). Close it to change mods.",
      ),
    ).toBeInTheDocument();
    const toggle = await sw("Alpha");
    expect(toggle).toBeDisabled();
    await userEvent.click(toggle);
    expect(mock.calls.some((c) => c.cmd === "set_mod_enabled")).toBe(false);
    // The group switch is blocked too.
    expect(screen.getByRole("switch", { name: "Scripts & Gameplay (all)" })).toBeDisabled();
  });

  it("shows recovery as a danger alert and blocks", async () => {
    mockModsIpc({
      mods: [makeMod()],
      guard: { gameRunning: [], writable: true, recoveryRequired: true, installationMissing: false },
    });
    renderView();
    expect(await screen.findByRole("alert")).toHaveTextContent(/did not finish cleanly/);
    expect(await sw("Alpha")).toBeDisabled();
  });

  it("shows a not-writable warning", async () => {
    mockModsIpc({
      mods: [makeMod()],
      guard: { gameRunning: [], writable: false, recoveryRequired: false, installationMissing: false },
    });
    renderView();
    expect(await screen.findByRole("status")).toHaveTextContent(/not writable/);
  });

  it("drifted mod shows the explanation and Check again reconciles and refetches", async () => {
    const mock = mockModsIpc({ mods: [makeMod({ state: "drifted" })] });
    renderView();
    expect(await screen.findByText("Files changed outside the app")).toBeInTheDocument();
    const reconciles = () => mock.calls.filter((c) => c.cmd === "reconcile_installation").length;
    const lists = () => mock.calls.filter((c) => c.cmd === "list_mods").length;
    expect(reconciles()).toBe(1);
    const listsBefore = lists();
    mock.mods = [makeMod({ state: "enabled" })];
    await userEvent.click(screen.getByRole("button", { name: "Check again" }));
    await waitFor(() => expect(reconciles()).toBe(2));
    await waitFor(() => expect(lists()).toBeGreaterThan(listsBefore));
    await waitFor(() => expect(screen.queryByText("Files changed outside the app")).toBeNull());
  });

  it("lists conflicts on the conflict icon", async () => {
    mockModsIpc({
      mods: [makeMod({ conflictsWith: [{ modId: "m2", name: "Beta" }, { modId: "m3", name: "Gamma" }] })],
    });
    renderView();
    expect(await screen.findByRole("img", { name: "Conflicts with Beta, Gamma" })).toBeInTheDocument();
  });
});

describe("ModsView grouping and filters", () => {
  const mods = [
    makeMod({ id: "a", name: "Alpha", category: "scripts_gameplay", state: "enabled" }),
    makeMod({ id: "b", name: "Bravo", category: "vehicles", state: "disabled" }),
    makeMod({ id: "c", name: "Charlie", category: "vehicles", state: "drifted" }),
  ];

  it("groups by non-empty category with enabled/total counts", async () => {
    mockModsIpc({ mods });
    renderView();
    const vehicles = await screen.findByRole("region", { name: "Vehicles" });
    expect(within(vehicles).getByText("0/2 enabled")).toBeInTheDocument();
    const scripts = screen.getByRole("region", { name: "Scripts & Gameplay" });
    expect(within(scripts).getByText("1/1 enabled")).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Sounds & Music" })).not.toBeInTheDocument();
  });

  it("filters by search, state and category", async () => {
    mockModsIpc({ mods });
    renderView();
    await screen.findByText("Alpha");

    await userEvent.type(screen.getByRole("searchbox", { name: "Search mods" }), "brav");
    expect(screen.queryByText("Alpha")).not.toBeInTheDocument();
    expect(screen.getByText("Bravo")).toBeInTheDocument();
    await userEvent.clear(screen.getByRole("searchbox", { name: "Search mods" }));

    await userEvent.selectOptions(screen.getByLabelText("State"), "attention");
    expect(screen.getByText("Charlie")).toBeInTheDocument();
    expect(screen.queryByText("Bravo")).not.toBeInTheDocument();

    await userEvent.selectOptions(screen.getByLabelText("State"), "enabled");
    expect(screen.getByText("Alpha")).toBeInTheDocument();
    expect(screen.queryByText("Charlie")).not.toBeInTheDocument();

    await userEvent.selectOptions(screen.getByLabelText("State"), "all");
    await userEvent.selectOptions(screen.getByLabelText("Category"), "vehicles");
    expect(screen.queryByText("Alpha")).not.toBeInTheDocument();
    expect(screen.getByText("Bravo")).toBeInTheDocument();

    await userEvent.type(screen.getByRole("searchbox", { name: "Search mods" }), "zzz");
    expect(screen.getByText("No mods match the current filters.")).toBeInTheDocument();
  });

  it("explains an empty installation", async () => {
    mockModsIpc({ mods: [] });
    renderView();
    expect(await screen.findByText(/No managed mods yet/)).toBeInTheDocument();
  });
});

describe("group switch", () => {
  const vehicles = [
    makeMod({ id: "b", name: "Bravo", category: "vehicles", state: "disabled" }),
    makeMod({ id: "d", name: "Delta", category: "vehicles", state: "disabled" }),
  ];
  const preview = {
    modsAffected: [
      { modId: "b", name: "Bravo", from: "disabled" as const, to: "enabled" as const },
      { modId: "d", name: "Delta", from: "disabled" as const, to: "enabled" as const },
    ],
    actions: [
      { kind: "place_from_vault" as const, relPath: "x", modName: "Bravo" },
      { kind: "place_from_vault" as const, relPath: "y", modName: "Delta" },
      { kind: "place_from_vault" as const, relPath: "z", modName: "Delta" },
    ],
    bytesToCopy: 3 * 1024 * 1024,
  };

  it("opens a confirmation with the preview; Cancel sends nothing", async () => {
    const mock = mockModsIpc({ mods: vehicles, preview });
    renderView();
    await userEvent.click(await sw("Vehicles (all)"));
    const dialog = await screen.findByRole("dialog", { name: "Enable Vehicles" });
    expect(await within(dialog).findByText(/2 mods affected · 3 file actions · 3\.0 MB to copy/)).toBeInTheDocument();
    expect(within(dialog).getByText("Bravo")).toBeInTheDocument();
    expect(mock.calls.find((c) => c.cmd === "preview_set_category")?.args).toEqual({
      installationId: ID,
      category: "vehicles",
      enabled: true,
    });
    await userEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(mock.calls.some((c) => c.cmd === "set_category_enabled")).toBe(false);
    expect(screen.getByRole("switch", { name: "Vehicles (all)" })).not.toBeChecked();
  });

  it("Apply keeps the switch pending until the command resolves, then follows the server", async () => {
    const gate = deferred<ModDto[]>();
    const mock = mockModsIpc({ mods: vehicles, preview });
    mock.handlers.set_category_enabled = () => gate.promise;
    renderView();
    const group = await sw("Vehicles (all)");
    await userEvent.click(group);
    const dialog = await screen.findByRole("dialog");
    const apply = await within(dialog).findByRole("button", { name: "Apply" });
    await waitFor(() => expect(apply).toBeEnabled());
    await userEvent.click(apply);
    await waitFor(() => expect(group).toBeDisabled());
    expect(group).not.toBeChecked();

    const after = vehicles.map((m) => ({ ...m, state: "enabled" as const }));
    mock.mods = after;
    gate.resolve(after);
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    await waitFor(() => expect(group).toBeChecked());
    expect(screen.getByText("2/2 enabled")).toBeInTheDocument();
  });

  it("shows the AppError in the dialog when applying fails", async () => {
    const mock = mockModsIpc({ mods: vehicles, preview });
    mock.handlers.set_category_enabled = () => Promise.reject(APP_ERROR);
    renderView();
    await userEvent.click(await sw("Vehicles (all)"));
    const dialog = await screen.findByRole("dialog");
    const apply = await within(dialog).findByRole("button", { name: "Apply" });
    await waitFor(() => expect(apply).toBeEnabled());
    await userEvent.click(apply);
    expect(await within(dialog).findByText("DRIFT_DETECTED")).toBeInTheDocument();
    expect(within(dialog).getByText("Check again first.")).toBeInTheDocument();
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });

  it("disables Apply when nothing would change", async () => {
    mockModsIpc({ mods: vehicles, preview: { modsAffected: [], actions: [], bytesToCopy: 0 } });
    renderView();
    await userEvent.click(await sw("Vehicles (all)"));
    const dialog = await screen.findByRole("dialog");
    expect(await within(dialog).findByText(/Nothing to change/)).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Apply" })).toBeDisabled();
  });
});
