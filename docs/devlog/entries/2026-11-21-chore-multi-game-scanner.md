---
date: 2026-11-21
branch: feature/multi-game-scanner
type: feat
author: Marcos Ramos Jacome
agents: claude:orchestrator
pr: pending
version: unreleased
---

# Red Dead Redemption 2: guard per game, signatures and online check

## What changes

Phase 6, step 2.

- **Guard per game.** `running_guarded(os, game)` replaces the single list. The lists are
  `GTAV_PROCESSES` (GTA V, both editions and their launchers), `RDR2_PROCESSES` (`RDR2.exe`,
  `PlayRDR2.exe`) and `ROCKSTAR_PROCESSES` (the launcher and its services, which block every
  game). A running GTA V no longer blocks changes to RDR2, and the other way round. The
  `GameRunning` error names the game, for example "Red Dead Redemption 2 or one of its
  launchers is running."
- **RDR2 signatures** (`catalog/mods/rdr2-*.toml`, all `verified = false`, A-12):
  - Script Hook RDR2 and its .NET port.
  - The ASI loader (`dinput8.dll`) and the Native Trainer, which requires both.
  - Lenny's Mod Loader (`vfs.asi`, `ModManager.*`, the loose files in `lml/`).
  - ReShade.

  Ids carry an `rdr2-` prefix because a signature cannot span two games.
- **Grouping.** `lml` is a grouping base: every `lml/<Mod>/` folder becomes one proposal, and
  the loader signature does not claim them.
- **Online check.** `OnlineCheckDto.game` is new. The UI words the result for the game: "GTA
  Online" or "Red Dead Online", and "GTA V" or "Red Dead Redemption 2 and its launchers are
  closed". The checks are the same for both games, with each game's own catalog and processes.
- **Registration messages.** They name the three executables (`GTA5.exe`,
  `GTA5_Enhanced.exe`, `RDR2.exe`) instead of GTA V only.

## Why

In step 1 the process list was shared by both games on purpose, as the conservative choice. It
was annoying, though: preparing GTA V while RDR2 was open was refused. Recognition is what makes
RDR2 useful beyond loose files: with named mods and their requirements, the vanilla mode and
the online check do the same job they do for GTA V.

## Files and areas

- `crates/scanner/src/{processes.rs, lib.rs, validation.rs}`,
  `crates/scanner/src/recognize/{signature.rs, matcher.rs, group.rs}`,
  `crates/scanner/catalog/mods/rdr2-*.toml` (6 new)
- `crates/app/src/{guards.rs, online.rs, installations.rs}`, `crates/app/tests/rdr2.rs`
- UI: `features/online/{labels.ts, OnlineCheckPanel.tsx, OnlineCheckPanel.test.tsx}`,
  `features/installations/AddManualButton.test.tsx`, `bindings/OnlineCheckDto.ts`
- `docs/roadmap.md` (Phase 6 steps), `CHANGELOG.md`

## How to test

- `make check`.
- New tests:
  - Scanner:
    - each game blocks only itself and the Rockstar launcher blocks both;
    - RDR2 mods recognized, with the `lml/` mod left for grouping;
    - no cross-game proposals;
    - `lml/<Mod>/` grouping.
  - `crates/app/tests/rdr2.rs`:
    - proposals by name and adopting them all links the 2 dependencies, with the game folder
      untouched;
    - GTA V running does not block RDR2, and `RockstarService.exe` does;
    - prepare for online returns `game = rdr2`, ignores `GTA5.exe`, and lists `RDR2.exe` when
      RDR2 is running.
  - UI: verdict and check titles for `rdr2`.

## Notes and follow-ups

- The names in the RDR2 signatures and the process names are still assumptions (A-10, A-12)
  until the owner's file list of a real install is available.
- Step 3: the UI grouped by game, and the app name and texts that still say GTA V (window
  title, single-instance lock message, help texts).
