---
date: 2026-11-21
branch: feature/rename-install-identity
type: feat
author: Marcos Ramos Jacome
agents: claude:orchestrator
pr: pending
version: unreleased
---

# Windows install renamed to ModSwitch

## What changes

Phase 6, step 5.

- `tauri.conf.json`: `productName` is **ModSwitch**. Windows shows that name in the Start menu,
  installed apps and the install folder (`%LOCALAPPDATA%\ModSwitch`). The release asset is
  `ModSwitch-windows-x64-setup.exe` (`release.yml`, `scripts/updater-manifest.sh`, README,
  `docs/release.md`).
- `windows/hooks.nsh`, new `NSIS_HOOK_POSTINSTALL`, which runs only while the uninstall entry
  of *GTA V Mod Manager* exists. After the new files are installed, it:
  - runs the old `uninstall.exe` silently, without `/UPDATE`, so that the old program files,
    uninstall entry and shortcuts go away;
  - deletes whatever is left: the old uninstall key, `Software\Kaelence\GTA V Mod Manager`, and
    the old `.lnk` files;
  - creates `$SMPROGRAMS\ModSwitch.lnk`, and the desktop shortcut if there was one. The in-app
    update runs the installer with `/UPDATE`, which creates no shortcuts, so without this step
    the app would disappear from the Start menu.

  If the old folder is the new one, it skips the uninstaller. The installer only adds files,
  so the old uninstaller would delete the new executable.
- What keeps the old name, on purpose:
  - the data folder `%LOCALAPPDATA%\GtavModManager`, with the database and the vault;
  - the identifier `com.kaelence.gtavmodmanager`, which also keeps the WebView data;
  - the repository and the updater endpoint.

## Why

The owner saw the app still called *GTA V Mod Manager* in Windows after the rename in v0.4.0.
Changing `productName` alone would install ModSwitch next to the old app, with two entries,
two Start menu shortcuts and the old program left behind.

## Files and areas

- `apps/desktop/src-tauri/{tauri.conf.json, windows/hooks.nsh}`
- `.github/workflows/release.yml`, `scripts/updater-manifest.sh`
- `README.md`, `docs/release.md`, `docs/windows-validation.md` §5.1, `docs/roadmap.md`
  (Phase 6 step 5), `CHANGELOG.md`

## How to test

- Read the Tauri 2 NSIS template (embedded in `@tauri-apps/cli`) for:
  - the uninstall key, install folder and shortcut names, which come from `PRODUCTNAME`;
  - the updater arguments `/P /UPDATE /R`;
  - `CreateOrUpdateStartMenuShortcut`, which returns early in update mode.
- `makensis -WX` compiles the hooks in a harness, with stubs for the template's defines and
  `SetLnkAppUserModelId`. The same harness fails when the hook is broken.
- `make check`.
- Real check on Windows: `docs/windows-validation.md` §5.1. Update from v0.5.0, then make sure
  only ModSwitch is listed, the Start menu shortcut exists and the data is intact.

## Notes and follow-ups

- A taskbar pin of the old app is removed by its uninstaller, and Windows does not let an
  installer pin apps: the owner pins ModSwitch again.
- The hook becomes a no-op once every install has migrated. It can be removed in a later version.
