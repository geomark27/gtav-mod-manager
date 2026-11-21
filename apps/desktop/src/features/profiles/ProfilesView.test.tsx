import { clearMocks } from "@tauri-apps/api/mocks";
import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ProfileDto } from "../../bindings/ProfileDto";
import { renderWithProviders } from "../installations/testing";
import { deferred } from "../inventory/testing";
import { APP_ERROR } from "../mods/testing";
import { ProfilesView } from "./ProfilesView";
import { makeProfile, mockProfilesIpc, VANILLA } from "./testing";

afterEach(() => clearMocks());

function renderView() {
  return renderWithProviders(<ProfilesView installationId="inst-1" />);
}

function item(name: string) {
  return screen.findByRole("listitem", { name });
}

describe("ProfilesView", () => {
  it("lists Vanilla first as built-in with only Preview & apply", async () => {
    mockProfilesIpc();
    renderView();
    const vanilla = await item("Vanilla");
    expect(within(vanilla).getByText("Built-in")).toBeInTheDocument();
    expect(within(vanilla).getByRole("button", { name: "Preview & apply" })).toBeInTheDocument();
    for (const name of ["Edit", "Rename", "Delete"]) {
      expect(within(vanilla).queryByRole("button", { name })).not.toBeInTheDocument();
    }
    const story = await item("Story");
    for (const name of ["Preview & apply", "Edit", "Rename", "Delete"]) {
      expect(within(story).getByRole("button", { name })).toBeInTheDocument();
    }
    expect(screen.getAllByRole("listitem")[0]).toBe(vanilla);
  });

  it("marks the restore point saved by vanilla mode", async () => {
    mockProfilesIpc({
      profiles: [VANILLA, makeProfile({ id: "rp", name: "Before vanilla", kind: "restore_point" })],
    });
    renderView();
    const rp = await item("Before vanilla");
    expect(within(rp).getByText("Restore point")).toBeInTheDocument();
    expect(within(await item("Vanilla")).queryByText("Restore point")).not.toBeInTheDocument();
  });

  it("saves the current state with a suggested name", async () => {
    const mock = mockProfilesIpc();
    mock.handlers.create_profile_from_current = (args) => {
      mock.profiles = [...mock.profiles, makeProfile({ id: "p2", name: String(args.name) })];
      return mock.profiles[mock.profiles.length - 1];
    };
    renderView();
    await userEvent.click(
      await screen.findByRole("button", { name: "Save current state as profile…" }),
    );
    const dialog = await screen.findByRole("dialog");
    const save = within(dialog).getByRole("button", { name: "Save profile" });
    expect(save).toBeDisabled();
    await userEvent.click(await within(dialog).findByRole("button", { name: "Clean" }));
    expect(within(dialog).getByLabelText("Profile name")).toHaveValue("Clean");
    await userEvent.click(save);
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(mock.calls.find((c) => c.cmd === "create_profile_from_current")?.args).toEqual({
      installationId: "inst-1",
      name: "Clean",
    });
    expect(await item("Clean")).toBeInTheDocument();
  });

  it("shows a save error", async () => {
    const mock = mockProfilesIpc();
    mock.handlers.create_profile_from_current = () =>
      Promise.reject({ ...APP_ERROR, code: "INVALID_INPUT" });
    renderView();
    await userEvent.click(
      await screen.findByRole("button", { name: "Save current state as profile…" }),
    );
    const dialog = await screen.findByRole("dialog");
    await userEvent.type(within(dialog).getByLabelText("Profile name"), "Story");
    await userEvent.click(within(dialog).getByRole("button", { name: "Save profile" }));
    expect(await within(dialog).findByText("INVALID_INPUT")).toBeInTheDocument();
  });

  it("previews and applies a profile, pending while it runs", async () => {
    const gate = deferred<unknown>();
    const mock = mockProfilesIpc();
    mock.handlers.apply_profile = () => gate.promise;
    renderView();
    await userEvent.click(
      within(await item("Story")).getByRole("button", { name: "Preview & apply" }),
    );
    const dialog = await screen.findByRole("dialog");
    expect(await within(dialog).findByText("Alpha: disabled → enabled")).toBeInTheDocument();
    expect(within(dialog).getByText(/1 mod affected · 2.0 KB to copy/)).toBeInTheDocument();
    expect(mock.calls.find((c) => c.cmd === "preview_apply_profile")?.args).toEqual({
      installationId: "inst-1",
      profileId: "p1",
    });
    const apply = within(dialog).getByRole("button", { name: "Apply" });
    await waitFor(() => expect(apply).toBeEnabled());
    await userEvent.click(apply);
    await waitFor(() => expect(apply).toBeDisabled());
    expect(apply).toHaveAttribute("aria-busy", "true");
    gate.resolve([]);
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(mock.calls.find((c) => c.cmd === "apply_profile")?.args).toEqual({
      installationId: "inst-1",
      profileId: "p1",
    });
  });

  it("previews Vanilla with its id and shows an apply error", async () => {
    const mock = mockProfilesIpc();
    mock.handlers.apply_profile = () => Promise.reject(APP_ERROR);
    renderView();
    await userEvent.click(
      within(await item("Vanilla")).getByRole("button", { name: "Preview & apply" }),
    );
    const dialog = await screen.findByRole("dialog");
    const apply = await within(dialog).findByRole("button", { name: "Apply" });
    await waitFor(() => expect(apply).toBeEnabled());
    await userEvent.click(apply);
    expect(await within(dialog).findByText("DRIFT_DETECTED")).toBeInTheDocument();
    expect(mock.calls.find((c) => c.cmd === "preview_apply_profile")?.args).toEqual({
      installationId: "inst-1",
      profileId: "vanilla",
    });
  });

  it("disables Apply when nothing would change, or when a guard blocks writes", async () => {
    mockProfilesIpc({ preview: { modsAffected: [], actions: [], bytesToCopy: 0 } });
    renderView();
    await userEvent.click(
      within(await item("Story")).getByRole("button", { name: "Preview & apply" }),
    );
    const dialog = await screen.findByRole("dialog");
    expect(await within(dialog).findByText(/Nothing to change/)).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Apply" })).toBeDisabled();
  });

  it("blocks Apply and shows the banner while the game is running", async () => {
    mockProfilesIpc({
      guard: { gameRunning: ["GTA5.exe"], writable: true, recoveryRequired: false, installationMissing: false },
    });
    renderView();
    expect(await screen.findByText(/The game or a launcher is running/)).toBeInTheDocument();
    await userEvent.click(
      within(await item("Story")).getByRole("button", { name: "Preview & apply" }),
    );
    const dialog = await screen.findByRole("dialog");
    await within(dialog).findByText("Alpha: disabled → enabled");
    expect(within(dialog).getByRole("button", { name: "Apply" })).toBeDisabled();
  });

  it("shows a preview error", async () => {
    const mock = mockProfilesIpc();
    mock.handlers.preview_apply_profile = () => Promise.reject(APP_ERROR);
    renderView();
    await userEvent.click(
      within(await item("Story")).getByRole("button", { name: "Preview & apply" }),
    );
    const dialog = await screen.findByRole("dialog");
    expect(await within(dialog).findByText("DRIFT_DETECTED")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Apply" })).toBeDisabled();
  });

  it("lists every mod with three choices and saves only the included ones", async () => {
    const mock = mockProfilesIpc();
    mock.handlers.update_profile_entries = () => mock.detail;
    renderView();
    await userEvent.click(within(await item("Story")).getByRole("button", { name: "Edit" }));
    const dialog = await screen.findByRole("dialog");
    const alpha = await within(dialog).findByRole("combobox", { name: "Alpha in this profile" });
    const beta = within(dialog).getByRole("combobox", { name: "Beta in this profile" });
    const gamma = within(dialog).getByRole("combobox", { name: "Gamma in this profile" });
    expect(alpha).toHaveValue("enabled");
    expect(beta).toHaveValue("disabled");
    // A mod added after the profile was saved defaults to "Not in profile".
    expect(gamma).toHaveValue("none");
    expect(within(gamma).getAllByRole("option").map((o) => o.textContent)).toEqual([
      "Enabled",
      "Disabled",
      "Not in profile",
    ]);
    const save = within(dialog).getByRole("button", { name: "Save" });
    expect(save).toBeDisabled();
    await userEvent.selectOptions(alpha, "none");
    await userEvent.selectOptions(beta, "enabled");
    await userEvent.selectOptions(gamma, "disabled");
    await userEvent.click(save);
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(mock.calls.find((c) => c.cmd === "update_profile_entries")?.args).toEqual({
      profileId: "p1",
      entries: [
        { modId: "m2", desired: "enabled" },
        { modId: "m3", desired: "disabled" },
      ],
    });
  });

  it("shows an edit error", async () => {
    const mock = mockProfilesIpc();
    mock.handlers.update_profile_entries = () => Promise.reject(APP_ERROR);
    renderView();
    await userEvent.click(within(await item("Story")).getByRole("button", { name: "Edit" }));
    const dialog = await screen.findByRole("dialog");
    await userEvent.selectOptions(
      await within(dialog).findByRole("combobox", { name: "Alpha in this profile" }),
      "disabled",
    );
    await userEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    expect(await within(dialog).findByText("DRIFT_DETECTED")).toBeInTheDocument();
  });

  it("renames a profile", async () => {
    const mock = mockProfilesIpc();
    mock.handlers.rename_profile = (args) => {
      const renamed: ProfileDto = makeProfile({ name: String(args.name) });
      mock.profiles = [VANILLA, renamed];
      return renamed;
    };
    renderView();
    await userEvent.click(within(await item("Story")).getByRole("button", { name: "Rename" }));
    const dialog = await screen.findByRole("dialog");
    const rename = within(dialog).getByRole("button", { name: "Rename" });
    expect(rename).toBeDisabled();
    const input = within(dialog).getByLabelText("New name");
    await userEvent.clear(input);
    await userEvent.type(input, "Main");
    await userEvent.click(rename);
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(mock.calls.find((c) => c.cmd === "rename_profile")?.args).toEqual({
      profileId: "p1",
      name: "Main",
    });
    expect(await item("Main")).toBeInTheDocument();
  });

  it("asks for confirmation before deleting", async () => {
    const mock = mockProfilesIpc();
    mock.handlers.delete_profile = () => {
      mock.profiles = [VANILLA];
      return undefined;
    };
    renderView();
    await userEvent.click(within(await item("Story")).getByRole("button", { name: "Delete" }));
    const dialog = await screen.findByRole("dialog");
    expect(mock.calls.some((c) => c.cmd === "delete_profile")).toBe(false);
    await userEvent.click(within(dialog).getByRole("button", { name: "Delete profile" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(mock.calls.find((c) => c.cmd === "delete_profile")?.args).toEqual({ profileId: "p1" });
    await waitFor(() => expect(screen.queryByRole("listitem", { name: "Story" })).not.toBeInTheDocument());
  });
});
