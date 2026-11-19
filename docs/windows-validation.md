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
