---
date: 2026-11-21
branch: feature/delete-mod-and-tray
type: feat
author: Marcos Ramos Jacome
agents: claude:orchestrator
pr: pending
version: unreleased
---

# Delete a mod, and the tray icon

## What changes

**Delete a mod**

- `App::delete_mod` (new `crates/app/src/remove.rs`) and the Tauri command `delete_mod`, which
  returns `DeleteModResultDto` with the name, files removed and vault bytes freed.
  - An enabled mod is first disabled by the usual journaled transaction, so its files leave the
    game folder. Then the mod and its entries, dependency links and profile entries are deleted.
  - Its vault objects that nothing else references are freed with the existing `vault_gc`.
    This is skipped while a transaction is unresolved, as in Settings → Vault.
  - Original game files are never touched.
  - Refused when another mod requires it (`DEPENDENCY_BLOCKED`, with their names), when it
    drifted, when its stored copy is damaged, and by the guards (game running, recovery).
- UI: in the mod details, **Delete mod** asks for confirmation with a text that says where the
  files go and that the deletion cannot be undone. It is available for enabled and disabled
  mods. Drifted and damaged mods say why they cannot be deleted. "Stop managing" is unchanged.

**Tray**

- A tray icon (Tauri `tray-icon` feature):
  - left click opens the window;
  - the right-click menu has "Open ModSwitch", one block per installation (GTA V first) with
    **Prepare for online** and **Restore my mods**, and "Quit ModSwitch".
- The actions run the same use cases as the window (`prepare_for_online`,
  `restore_from_vanilla`) on the blocking pool and end with a Windows notification
  (`tauri-plugin-notification`).
  - The wording lives in `crates/app/src/tray.rs` and is tested on Linux: the verdict, how many
    things to fix or review, the error and its hint. It never says "safe".
  - The UI refreshes every query on the new `gmm://data-changed` event.
- Closing the window hides it to the tray. The first time per session, a notification says the
  app is still running.
- `tauri-plugin-single-instance`: launching the app again shows the running window instead of
  failing with "already running". The second process logs that at info level, not as an error.
- The tray menu is rebuilt after an installation is registered or renamed.

## Why

- The owner imported Red Dead Redemption 2 mods into the GTA V installation by mistake and had
  no way to remove them: "Stop managing" leaves the files in the game folder.
- The tray is the quick switch the owner asked for: preparing for online or getting the mods
  back without opening the window.

## Files and areas

- `crates/app/src/{remove.rs, tray.rs}` (new), `crates/app/src/lib.rs`,
  `crates/app/tests/delete.rs` (new)
- `apps/desktop/src-tauri/`:
  - `src/{tray.rs (new), lib.rs, commands.rs}`;
  - `Cargo.toml`: `tray-icon` feature and two plugins. They are Rust only, so no npm package.
- `Cargo.toml` (workspace), `Cargo.lock`
- UI:
  - `features/mods/{ModDetailsDialog.tsx, ModDetailsDialog.test.tsx, useMods.ts}`;
  - `lib/{ipc.ts, dataChanged.ts, dataChanged.test.tsx}`, `App.tsx`;
  - `bindings/DeleteModResultDto.ts`.
- `docs/windows-validation.md` §8, `CHANGELOG.md`

## How to test

- `make check`.
- `crates/app/tests/delete.rs` (7 tests):
  - an enabled mod leaves the folder and the vault, and other mods stay;
  - a disabled mod is deleted without folder changes;
  - a mod that others require is refused;
  - a running game and a drifted mod are refused without changes;
  - shared vault content is kept;
  - the owner's case: RDR2 files in a GTA V install.
- `crates/app/src/tray.rs` (5 tests) for the notification texts.
- UI tests for the dialog and the refresh event.
- The tray itself only type-checks from WSL (`make shell`). Follow
  `docs/windows-validation.md` §8 on Windows.

## Notes and follow-ups

- Hide-to-tray is the only close behaviour. If it bothers the owner, add a setting.
- The tray acts on every installation, not only the active one, so both games are one click
  away.
