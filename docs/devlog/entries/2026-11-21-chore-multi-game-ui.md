---
date: 2026-11-21
branch: feature/multi-game-ui
type: feat
author: Marcos Ramos Jacome
agents: claude:orchestrator
pr: pending
version: v0.4.0
---

# ModSwitch: UI grouped by game and the new app name

## What changes

Phase 6, step 3.

- **New name: ModSwitch.** The owner left the choice to the agent. The name says what the app is
  for (switching mods off and on quickly, for example before playing online), does not use a
  game or publisher trademark, and fits both games. It appears in:
  - the sidebar, the window title (`tauri.conf.json` `title`) and the HTML title;
  - the installer descriptions;
  - the single-instance message ("ModSwitch is already running.");
  - the GitHub release titles and tags (`release.yml`, `scripts/release.sh`) and the script
    headers;
  - the README, the CHANGELOG intro, the architecture title and the crate doc comments.
- **What keeps the old name, on purpose:**
  - Tauri `productName` (Start menu, install folder, uninstall entry), the installer file name
    `GTAV-Mod-Manager-windows-x64-setup.exe`, and the repository. Renaming `productName` would
    make the NSIS installer of an update install next to the old app instead of replacing it.
  - The identifier `com.kaelence.gtavmodmanager` and the data folder `GtavModManager`: they
    hold the database and the vault.

  Roadmap Phase 6, step 5 records the follow-up.
- **Grouped by game.**
  - The Installations page has one section per game (Grand Theft Auto V, then Red Dead
    Redemption 2).
  - The sidebar selector groups the options under GTA V and Red Dead Redemption 2.
  - The sidebar names the game of the active installation, so every page shows it.
- **Texts.**
  - The persistent notice reads "…does not make an installation safe for GTA Online or Red
    Dead Online."
  - The blocked notice reads "The game or a launcher is running (…)".
  - Discovery reads "No GTA V or Red Dead Redemption 2 installations were detected."
- New `features/games/games.ts`: `APP_NAME`, `GAMES`, `GAME_NAME`, `GAME_SHORT`,
  `ONLINE_NAME`, `ONLINE_NOTICE` and `groupByGame`. The online labels and Settings use it.

## Why

With RDR2 supported (steps 1 and 2), the name "GTA V Mod Manager" and the GTA-only texts were
wrong for half of the installations. A user with both games needs to see at a glance which game
the active installation is.

## Files and areas

- UI:
  - `features/games/` (new);
  - `layout/{AppShell.tsx, AppShell.module.css, InstallationSelector.tsx}`;
  - `pages/InstallationsPage.tsx`, `features/installations/{DiscoverPanel.tsx,
    Installations.module.css}`;
  - `features/{guards/blocking.ts, online/labels.ts, settings/SettingsView.tsx}`;
  - tests that assert these texts.
- `apps/desktop/{index.html, src-tauri/tauri.conf.json, src-tauri/src/main.rs}`
- `crates/app/src/{lock.rs, status.rs, error.rs, lib.rs}`, crate doc comments
- `.github/workflows/release.yml`, `scripts/{release.sh, bump.sh, updater-manifest.sh}`
- `README.md`, `CHANGELOG.md`, `docs/roadmap.md`, `docs/architecture.md`

## How to test

- `make check`.
- New UI tests:
  - `features/games/games.test.ts`: grouping order, and the notice never says that online play
    is safe.
  - `InstallationsPage`: one section per game.
  - `InstallationSelector`: option groups, and the active game is named.

## Notes and follow-ups

- Step 4: verify the RDR2 assumptions A-09 to A-12 with the owner's file list.
- Step 5: rename the install identity with an NSIS hook that removes the old install, and test
  it on Windows before any release that changes `productName`.
