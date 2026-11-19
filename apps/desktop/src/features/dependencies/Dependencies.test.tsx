import { clearMocks } from "@tauri-apps/api/mocks";
import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ModDetailDto } from "../../bindings/ModDetailDto";
import { renderWithProviders } from "../installations/testing";
import { ModsView } from "../mods/ModsView";
import { APP_ERROR, makeMod, mockModsIpc } from "../mods/testing";
import type { ModsMock } from "../mods/testing";

afterEach(() => clearMocks());

const ALPHA = makeMod({ id: "a", name: "Alpha", state: "enabled" });
const BETA = makeMod({ id: "b", name: "Beta" });
const GAMMA = makeMod({ id: "c", name: "Gamma" });

function renderView() {
  return renderWithProviders(<ModsView installationId="inst-1" name="GTA V" />);
}

async function openDetails(name: string) {
  await userEvent.click(await screen.findByRole("button", { name }));
  return screen.findByRole("dialog", { name });
}

function detailOf(mod: typeof ALPHA, over: Partial<ModDetailDto> = {}): ModDetailDto {
  return { ...mod, entries: [], dependsOn: [], requiredBy: [], ...over };
}

describe("dependencies in the details dialog", () => {
  it("adds a requirement from the other mods and removes it again", async () => {
    let detail = detailOf(ALPHA, { requiredBy: [{ modId: "c", name: "Gamma" }] });
    const mock = mockModsIpc({ mods: [ALPHA, BETA, GAMMA] });
    mock.detail = () => detail;
    mock.handlers.add_dependency = () => {
      detail = { ...detail, dependsOn: [{ modId: "b", name: "Beta" }], dependsOnCount: 1 };
      return detail;
    };
    mock.handlers.remove_dependency = () => {
      detail = { ...detail, dependsOn: [], dependsOnCount: 0 };
      return detail;
    };
    renderView();
    const dialog = await openDetails("Alpha");

    expect(await within(dialog).findByText("This mod does not require other mods.")).toBeInTheDocument();
    // "Required by" is read-only: Gamma is listed but has no button and is not offered as a requirement.
    expect(within(dialog).getByText("Gamma")).toBeInTheDocument();
    const select = within(dialog).getByLabelText("Add requirement");
    expect(within(select).getAllByRole("option").map((o) => o.textContent)).toEqual([
      "Choose a mod…",
      "Beta",
    ]);
    expect(within(dialog).getByRole("button", { name: "Add" })).toBeDisabled();

    await userEvent.selectOptions(select, "b");
    await userEvent.click(within(dialog).getByRole("button", { name: "Add" }));
    await waitFor(() =>
      expect(mock.calls.find((c) => c.cmd === "add_dependency")?.args).toEqual({
        modId: "a",
        dependsOnModId: "b",
      }),
    );
    const remove = await within(dialog).findByRole("button", { name: "Remove requirement Beta" });

    await userEvent.click(remove);
    await waitFor(() =>
      expect(mock.calls.find((c) => c.cmd === "remove_dependency")?.args).toEqual({
        modId: "a",
        dependsOnModId: "b",
      }),
    );
    expect(await within(dialog).findByText("This mod does not require other mods.")).toBeInTheDocument();
  });

  it("shows a dependency error", async () => {
    const mock = mockModsIpc({ mods: [ALPHA, BETA] });
    mock.detail = () => detailOf(ALPHA);
    mock.handlers.add_dependency = () => Promise.reject({ ...APP_ERROR, code: "INVALID_INPUT" });
    renderView();
    const dialog = await openDetails("Alpha");
    await userEvent.selectOptions(await within(dialog).findByLabelText("Add requirement"), "b");
    await userEvent.click(within(dialog).getByRole("button", { name: "Add" }));
    expect(await within(dialog).findByText("INVALID_INPUT")).toBeInTheDocument();
  });

  it("shows the file version column only when a version is known", async () => {
    const mock = mockModsIpc({ mods: [ALPHA] });
    mock.detail = () =>
      detailOf(ALPHA, {
        entries: [
          { relPath: "ScriptHookV.dll", size: 10, sha256: "abcdef0123", fileVersion: "1.0.3179.0" },
          { relPath: "readme.txt", size: 10, sha256: "0123abcdef", fileVersion: null },
        ],
      });
    renderView();
    const dialog = await openDetails("Alpha");
    expect(await within(dialog).findByText("1.0.3179.0")).toBeInTheDocument();
    expect(within(dialog).getByRole("columnheader", { name: "Version" })).toBeInTheDocument();
  });
});

describe("switches with dependencies", () => {
  function setup(mods = [makeMod({ id: "a", name: "Alpha", dependsOnCount: 1 }), BETA]): ModsMock {
    return mockModsIpc({ mods });
  }

  it("shows a small 'requires N' label", async () => {
    setup();
    renderView();
    expect(await screen.findByText("requires 1")).toBeInTheDocument();
    expect(screen.getAllByText(/^requires/)).toHaveLength(1);
  });

  it("previews first and sends directly when only that mod is affected", async () => {
    const mock = setup();
    renderView();
    await userEvent.click(await screen.findByRole("switch", { name: "Alpha" }));
    await waitFor(() =>
      expect(mock.calls.find((c) => c.cmd === "set_mod_enabled")?.args).toEqual({
        modId: "a",
        enabled: true,
      }),
    );
    const cmds = mock.calls.map((c) => c.cmd);
    expect(cmds.indexOf("preview_set_mod_enabled")).toBeLessThan(cmds.indexOf("set_mod_enabled"));
    expect(screen.queryByText(/Also enable/)).not.toBeInTheDocument();
  });

  it("asks for confirmation when more than one mod is affected and sends withDependencies", async () => {
    const mock = setup();
    mock.handlers.preview_set_mod_enabled = () => ({
      modsAffected: [
        { modId: "b", name: "Beta", from: "disabled", to: "enabled" },
        { modId: "a", name: "Alpha", from: "disabled", to: "enabled" },
      ],
      actions: [],
      bytesToCopy: 0,
    });
    mock.handlers.set_mod_enabled = () => makeMod({ id: "a", name: "Alpha", state: "enabled" });
    renderView();
    await userEvent.click(await screen.findByRole("switch", { name: "Alpha" }));
    const dialog = await screen.findByRole("dialog", { name: "Enable Alpha" });
    expect(within(dialog).getByText("Also enable:")).toBeInTheDocument();
    expect(within(dialog).getByText("Beta")).toBeInTheDocument();
    expect(mock.calls.some((c) => c.cmd === "set_mod_enabled")).toBe(false);

    await userEvent.click(within(dialog).getByRole("button", { name: "Apply to all" }));
    await waitFor(() =>
      expect(mock.calls.find((c) => c.cmd === "set_mod_enabled")?.args).toEqual({
        modId: "a",
        enabled: true,
        withDependencies: true,
      }),
    );
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("cancelling the confirmation changes nothing; disabling says 'Also disable'", async () => {
    const mock = setup([makeMod({ id: "a", name: "Alpha", state: "enabled" }), makeMod({ id: "b", name: "Beta", state: "enabled" })]);
    mock.handlers.preview_set_mod_enabled = () => ({
      modsAffected: [
        { modId: "a", name: "Alpha", from: "enabled", to: "disabled" },
        { modId: "b", name: "Beta", from: "enabled", to: "disabled" },
      ],
      actions: [],
      bytesToCopy: 0,
    });
    renderView();
    await userEvent.click(await screen.findByRole("switch", { name: "Alpha" }));
    const dialog = await screen.findByRole("dialog", { name: "Disable Alpha" });
    expect(within(dialog).getByText("Also disable:")).toBeInTheDocument();
    await userEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(mock.calls.some((c) => c.cmd === "set_mod_enabled")).toBe(false);
    expect(screen.getByRole("switch", { name: "Alpha" })).toBeChecked();
  });

  it("shows DEPENDENCY_BLOCKED like any other error", async () => {
    const mock = setup();
    mock.handlers.set_mod_enabled = () =>
      Promise.reject({
        code: "DEPENDENCY_BLOCKED",
        message: '"Alpha" requires "Beta", which would not be enabled.',
        hint: null,
        details: null,
      });
    renderView();
    await userEvent.click(await screen.findByRole("switch", { name: "Alpha" }));
    expect(await screen.findByText("DEPENDENCY_BLOCKED")).toBeInTheDocument();
    expect(screen.getByText(/requires "Beta"/)).toBeInTheDocument();
  });
});
