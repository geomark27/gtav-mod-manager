# Windows validation (M7)

Status: v0.1 · 2026-11-18 · Owner: Tech Lead. Checklist for the project owner on a real
Windows machine. Everything here is **read-only for the game** except section 4, which uses a
throw-away copy.

## 1. Toolchain (once)

1. Rust via <https://rustup.rs> (default host `x86_64-pc-windows-msvc`).
2. Node.js LTS (≥ 22.12).
3. Visual Studio Build Tools — workload “Desktop development with C++”.
4. WebView2 Runtime (preinstalled on Windows 11).

From WSL: `scripts/sync-to-windows.sh /mnt/c/dev/gtav-mod-manager`, then in a Windows terminal:

```
cd C:\dev\gtav-mod-manager
cargo test
cd apps\desktop
npm install
npm test
npm run tauri dev
```

`cargo test` on Windows also runs the `#[cfg(windows)]` tests (MoveFileExW no-replace,
hard-link count, Authenticode, version resource).

## 2. Resolve assumptions (no changes to the game)

| ID   | How to verify                                                                                           | Where to fix                         |
| ---- | ------------------------------------------------------------------------------------------------------- | ------------------------------------ |
| A-01 | Executable names in each edition's folder.                                                               | `crates/domain/src/edition.rs`, catalogs |
| A-02 | Steam: `steamapps\appmanifest_<id>.acf` names in the library.                                            | `crates/scanner/catalog/*.toml`      |
| A-03 | Epic: `C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests\*.item` has `InstallLocation`.               | `crates/scanner/src/discovery/epic.rs` |
| A-04 | Rockstar: `reg query "HKLM\SOFTWARE\WOW6432Node\Rockstar Games" /s` — note the key names for both editions. | catalogs `rockstar_registry_keys`    |
| A-05 | `dir /b` of each game root and of `update\` — compare with `required_files` / `originals`.               | catalogs                             |
| A-06 | In the app's Review page after a scan: which DLLs show “Signed by …”; add missing trusted publishers.     | catalogs `trusted_publishers`        |
| A-07 | Task Manager → Details while the game and launchers run: exact process names.                           | `crates/scanner/src/processes.rs`    |

## 3. Read-only app checks on the real installation

1. Installations → **Detect installations**: both editions found (or added manually).
2. Status **Ready**; version shown.
3. Review → **Scan now**: originals classified `Original`; no game DLL listed as Candidate
   (false positives → catalog fix).
4. Start the game: Mods page shows the “game running” banner and switches are blocked.

## 4. Destructive checks — on a COPY only

Copy a small subset that is structurally valid (or use `gmm-testkit`-style fake folder) to
`C:\gmm-test\GTAV` and register **that** folder:

1. Put a test `.asi` + `.ini` in it, scan, adopt, disable, enable — files byte-identical
   (compare with `certutil -hashfile <file> SHA256`).
2. Kill the app (Task Manager) during a large enable; restart → the Settings page shows the
   recovery report; folder consistent.
3. Put the copy under `C:\Program Files\gmm-test` → status **Read-only**; switches blocked.

## 5. Installer

`cd apps\desktop && npm run tauri build` → NSIS installer in
`apps\desktop\src-tauri\target\release\bundle\nsis\` (or the workspace `target\`). Install,
run, uninstall; the vault and database stay in `%LOCALAPPDATA%\GtavModManager` (by design —
removing them is a manual decision).

### 5.1 From GTA V Mod Manager to ModSwitch (v0.6.0)

The first ModSwitch installer removes the old *GTA V Mod Manager* install
(`apps/desktop/src-tauri/windows/hooks.nsh`). Before updating, copy
`%LOCALAPPDATA%\GtavModManager` somewhere safe (the hook never touches it, the copy is for peace
of mind).

1. With v0.5.0 installed and in use, accept the update banner (or run the v0.6.0 installer).
2. The app restarts as **ModSwitch**: window title, tray tooltip and sidebar.
3. Start menu: **ModSwitch** is there, *GTA V Mod Manager* is gone. If there was a desktop
   shortcut, it is now *ModSwitch*. A taskbar pin of the old app is gone: pin ModSwitch again.
4. Settings → Apps → Installed apps: only **ModSwitch** (version 0.6.0).
5. `%LOCALAPPDATA%\GTA V Mod Manager` no longer exists; `%LOCALAPPDATA%\ModSwitch` holds the
   program.
6. Installations, mods, profiles and Settings → Vault are exactly as before.
7. Run the v0.6.0 installer again: it updates in place (no second entry, no prompt about the old
   app).

## 6. Phase 4 — mod recognition (v0.2.0)

**A-08 (read-only, real installation).** In a Windows terminal, from each game root (Legacy
and Enhanced) — this only lists names, it changes nothing:

```
dir /s /b /a-d > C:\temp\gta-<edition>-files.txt
```

Send the lists (they contain no personal data beyond folder names). For every mod you have
installed, the Tech Lead compares the real names with `crates/scanner/catalog/mods/<id>.toml`,
fixes the patterns and sets `verified = true`.

**Flow on a COPY** (section 4 rules) with Script Hook V, Script Hook V .NET and Rampage
Trainer installed by hand:

1. Record hashes: `certutil -hashfile <file> SHA256` for every mod file (or keep a zip).
2. Review → scan → **Detected mods**: each mod shown by name with its requirements;
   loose files grouped; nothing from the game listed.
3. **Adopt selected**: one history entry per mod; Mods page shows the dependencies
   (Rampage → Script Hook V .NET → Script Hook V / ASI Loader) and “Source: Recognized as …”.
4. Dashboard → **Disable all mods (vanilla)**: mod files gone from the folder, *Vanilla mode
   active*, restore point listed in Profiles.
5. **Restore my mods**: every file back and byte-identical to step 1.

## 7. Phase 5 — before playing online (v0.3.0)

On the same COPY as §6, with a trusted baseline captured right after "verify files":

1. With mods enabled and one loose `.asi` not adopted: Dashboard → **Check only** → *things to
   fix* lists the managed mods and the loose file; nothing in the folder changed.
2. **Prepare for online** → managed mods disabled (one history entry), restore point saved; the
   loose `.asi` is still listed under "No other mod files".
3. Adopt it in Review → Detected mods, **Prepare for online** again → *No mod files found*.
4. Open GTA V, press **Prepare for online** → "The mods were not disabled" with the running
   process; close the game.
5. **Restore my mods** → every file back and byte-identical.

## 8. Delete a mod and the tray (v0.5.0)

The tray cannot be tested from WSL: only its texts (`crates/app/src/tray.rs`) and a Windows
type check (`make shell`) run there. On a COPY (section 4 rules):

**Delete a mod**

1. Mods → open an enabled mod → **Delete mod** → the text says its files leave the game folder
   and that this cannot be undone → **Confirm delete**: the files are gone from the folder, the
   mod is gone from the list, Settings → Vault shows no unused copy for it.
2. Same with a disabled mod: nothing in the game folder changes.
3. A mod another one requires: refused with "Other mods require …" and their names.
4. With the game open: the button is disabled with the reason.

**Tray**

1. Start the app: the ModSwitch icon appears next to the clock. Left click opens the window.
2. Right click: "Open ModSwitch", then one block per installation (GTA V first) with
   "Prepare for online" and "Restore my mods", then "Quit ModSwitch".
3. Close the window (X): it hides, a notification says ModSwitch is still running (only the
   first time). Start the app again from the Start menu: the same window comes back, no second
   window and no "already running" error.
4. Tray → **Prepare for online** with the window hidden: a notification with the result (never
   "safe"); open the window: Dashboard and Mods already show the mods disabled.
5. Open the game, tray → **Prepare for online**: notification "mods not disabled" naming the
   game; close the game.
6. Tray → **Restore my mods**: notification "N mods are back"; files byte-identical.
7. Register or rename an installation: the tray menu shows the new name.
8. **Quit ModSwitch**: the icon disappears and the process ends.
