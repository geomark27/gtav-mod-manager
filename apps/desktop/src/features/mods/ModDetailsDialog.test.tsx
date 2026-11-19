import { clearMocks } from "@tauri-apps/api/mocks";
import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { renderWithProviders } from "../installations/testing";
import { ModsView } from "./ModsView";
import { APP_ERROR, makeMod, mockModsIpc } from "./testing";

afterEach(() => clearMocks());

async function open(name: string) {
  renderWithProviders(<ModsView installationId="inst-1" name="GTA V" />);
  await userEvent.click(await screen.findByRole("button", { name }));
  return screen.findByRole("dialog", { name });
}

describe("mod details", () => {
  it("shows entries with short hashes", async () => {
    mockModsIpc({ mods: [makeMod({ state: "enabled" })] });
    const dialog = await open("Alpha");
    expect(await within(dialog).findByText("scripts/a.asi")).toBeInTheDocument();
    expect(within(dialog).getByText("abcdef01")).toBeInTheDocument();
  });

  it("Save is disabled until something changes, then sends the update", async () => {
    const mock = mockModsIpc({ mods: [makeMod({ state: "enabled" })] });
    const dialog = await open("Alpha");
    const save = await within(dialog).findByRole("button", { name: "Save" });
    expect(save).toBeDisabled();
    const name = within(dialog).getByLabelText("Name");
    await userEvent.clear(name);
    expect(save).toBeDisabled();
    await userEvent.type(name, "Alpha 2");
    await userEvent.selectOptions(within(dialog).getByLabelText("Category"), "vehicles");
    await userEvent.type(within(dialog).getByLabelText("Notes"), "hello");
    expect(save).toBeEnabled();
    await userEvent.click(save);
    await waitFor(() =>
      expect(mock.calls.find((c) => c.cmd === "update_mod")?.args).toEqual({
        modId: "mod-1",
        name: "Alpha 2",
        category: "vehicles",
        notes: "hello",
      }),
    );
  });

  it("shows an update error", async () => {
    const mock = mockModsIpc({ mods: [makeMod({ state: "enabled" })] });
    mock.handlers.update_mod = () => Promise.reject({ ...APP_ERROR, code: "INVALID_INPUT" });
    const dialog = await open("Alpha");
    await userEvent.type(await within(dialog).findByLabelText("Notes"), "x");
    await userEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    expect(await within(dialog).findByText("INVALID_INPUT")).toBeInTheDocument();
  });

  it("Stop managing asks for confirmation, then forgets the mod", async () => {
    const mock = mockModsIpc({ mods: [makeMod({ state: "enabled" })] });
    mock.handlers.forget_mod = () => {
      mock.mods = [];
      return undefined;
    };
    const dialog = await open("Alpha");
    await userEvent.click(await within(dialog).findByRole("button", { name: "Stop managing" }));
    expect(
      within(dialog).getByText(
        "The files stay in the game folder; the app stops tracking them. The vault copy is kept.",
      ),
    ).toBeInTheDocument();
    expect(mock.calls.some((c) => c.cmd === "forget_mod")).toBe(false);
    await userEvent.click(within(dialog).getByRole("button", { name: "Confirm stop managing" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(mock.calls.find((c) => c.cmd === "forget_mod")?.args).toEqual({ modId: "mod-1" });
    expect(await screen.findByText(/No managed mods yet/)).toBeInTheDocument();
  });

  it("Keep managing backs out without a command", async () => {
    const mock = mockModsIpc({ mods: [makeMod({ state: "drifted" })] });
    const dialog = await open("Alpha");
    await userEvent.click(await within(dialog).findByRole("button", { name: "Stop managing" }));
    await userEvent.click(within(dialog).getByRole("button", { name: "Keep managing" }));
    expect(within(dialog).getByRole("button", { name: "Stop managing" })).toBeInTheDocument();
    expect(mock.calls.some((c) => c.cmd === "forget_mod")).toBe(false);
  });

  it("offers no Stop managing for disabled mods", async () => {
    mockModsIpc({ mods: [makeMod({ state: "disabled" })] });
    const dialog = await open("Alpha");
    await within(dialog).findByText("scripts/a.asi");
    expect(within(dialog).queryByRole("button", { name: "Stop managing" })).not.toBeInTheDocument();
    expect(within(dialog).getByText(/Only enabled, drifted or error/)).toBeInTheDocument();
  });

  it("disables Stop managing while a guard blocks writes", async () => {
    mockModsIpc({
      mods: [makeMod({ state: "enabled" })],
      guard: { gameRunning: ["GTA5.exe"], writable: true, recoveryRequired: false, installationMissing: false },
    });
    const dialog = await open("Alpha");
    expect(await within(dialog).findByRole("button", { name: "Stop managing" })).toBeDisabled();
  });

  it("offers Stop managing for error mods", async () => {
    mockModsIpc({ mods: [makeMod({ state: "error" })] });
    const dialog = await open("Alpha");
    expect(await within(dialog).findByRole("button", { name: "Stop managing" })).toBeEnabled();
  });

  it("offers no drift actions unless the mod is drifted", async () => {
    mockModsIpc({ mods: [makeMod({ state: "enabled" })] });
    const dialog = await open("Alpha");
    await within(dialog).findByText("scripts/a.asi");
    expect(
      within(dialog).queryByRole("button", { name: "Restore vault version" }),
    ).not.toBeInTheDocument();
  });

  it("Restore vault version explains, confirms, then calls restore_mod", async () => {
    const mock = mockModsIpc({ mods: [makeMod({ state: "drifted" })] });
    mock.handlers.restore_mod = () => {
      mock.mods = [makeMod({ state: "enabled" })];
      return mock.mods[0];
    };
    const dialog = await open("Alpha");
    await userEvent.click(await within(dialog).findByRole("button", { name: "Restore vault version" }));
    expect(
      within(dialog).getByText(
        "Your current files are kept in the vault, then the managed version is put back.",
      ),
    ).toBeInTheDocument();
    expect(mock.calls.some((c) => c.cmd === "restore_mod")).toBe(false);
    await userEvent.click(within(dialog).getByRole("button", { name: "Confirm restore" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(mock.calls.find((c) => c.cmd === "restore_mod")?.args).toEqual({ modId: "mod-1" });
  });

  it("Keep current files confirms, then calls readopt_mod", async () => {
    const mock = mockModsIpc({ mods: [makeMod({ state: "drifted" })] });
    mock.handlers.readopt_mod = () => {
      mock.mods = [makeMod({ state: "enabled" })];
      return mock.mods[0];
    };
    const dialog = await open("Alpha");
    await userEvent.click(await within(dialog).findByRole("button", { name: "Keep current files" }));
    expect(
      within(dialog).getByText("The current files become the managed version."),
    ).toBeInTheDocument();
    await userEvent.click(within(dialog).getByRole("button", { name: "Confirm keep current files" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(mock.calls.find((c) => c.cmd === "readopt_mod")?.args).toEqual({ modId: "mod-1" });
  });

  it("shows a restore error and stays open; Cancel backs out", async () => {
    const mock = mockModsIpc({ mods: [makeMod({ state: "drifted" })] });
    mock.handlers.restore_mod = () => Promise.reject(APP_ERROR);
    const dialog = await open("Alpha");
    await userEvent.click(await within(dialog).findByRole("button", { name: "Restore vault version" }));
    await userEvent.click(within(dialog).getByRole("button", { name: "Confirm restore" }));
    expect(await within(dialog).findByText("DRIFT_DETECTED")).toBeInTheDocument();
    await userEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(within(dialog).getByRole("button", { name: "Keep current files" })).toBeInTheDocument();
  });

  it("disables drift actions while a guard blocks writes", async () => {
    mockModsIpc({
      mods: [makeMod({ state: "drifted" })],
      guard: { gameRunning: ["GTA5.exe"], writable: true, recoveryRequired: false, installationMissing: false },
    });
    const dialog = await open("Alpha");
    expect(await within(dialog).findByRole("button", { name: "Restore vault version" })).toBeDisabled();
    expect(within(dialog).getByRole("button", { name: "Keep current files" })).toBeDisabled();
  });
});
