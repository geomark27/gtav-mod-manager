---
date: 2026-11-21
branch: feature/multi-game
type: feat
author: Marcos Ramos Jacome
agents: claude:orchestrator
pr: pending
version: v0.4.0
---

# Red Dead Redemption 2 in the model, the database and the catalogs

## What changes

Phase 6, step 1: the app can hold Red Dead Redemption 2 installations next to GTA V.

- **Domain.** New `Game` (`gtav`, `rdr2`) and a third edition, `Edition::Rdr2` (`RDR2.exe`).
  Each edition belongs to one game (`Edition::game`, `Game::editions`). The game is derived
  from the edition and never stored.
- **Database, migration 0004.** `installations` is rebuilt so that its `CHECK` accepts `rdr2`.
  The runner now has migrations that rebuild a table. They run with foreign keys off, are
  rolled back if `PRAGMA foreign_key_check` finds a violation, and foreign keys are switched
  back on even when they fail. Existing data is kept as it is: every current installation stays
  `legacy` or `enhanced`.
- **Catalogs.** New `catalog/rdr2.toml`: Steam app id, Rockstar registry key, original files
  (`RDR2.exe`, `PlayRDR2.exe`, `*.rpf`, `x64/**`) and mod patterns (Script Hook RDR2 and its
  .NET version, Lenny's Mod Loader `lml/**` and `ModManager.*`, ASI loader, ReShade).
  `Catalogs` holds one catalog per edition in `Edition::ALL` order. Discovery (Steam, Epic,
  Rockstar), validation and scans pick RDR2 up through it.
- **Signatures scoped to their game.** A signature cannot list editions of two games. The
  matcher never proposes a signature of another game: a GTA V `dinput8.dll` signature is not
  offered on RDR2. The other-edition case (`edition_supported = false`) still applies within a
  game.
- **Guard.** `RDR2.exe` and `PlayRDR2.exe` join the guarded processes. The list is still one
  for both games, so a running RDR2 also blocks GTA V changes, and the other way round
  (conservative, until step 2).
- **DTO and UI.** `InstallationDto.game` is new. Bindings `Game.ts` and `Edition.ts` are
  regenerated, and the edition label "Red Dead Redemption 2" is added.
- Docs: roadmap Phase 6 and assumptions A-09 to A-12; CHANGELOG.

## Why

The owner wants one app for the mods of GTA V and RDR2. The engine (vault, journal,
transactions, profiles, vanilla mode) was already independent of the game, so this step makes
the second game representable without touching it. Each step must leave the app safe to
release, which is why the RDR2 processes are guarded from this first step.

## Files and areas

- `crates/domain/src/{game.rs (new), edition.rs, lib.rs}`
- `crates/store/migrations/0004_rdr2_edition.sql` (new), `crates/store/src/migrations.rs`
- `crates/scanner/catalog/rdr2.toml` (new), `crates/scanner/src/{catalog.rs, processes.rs}`,
  `crates/scanner/src/recognize/{signature.rs, matcher.rs}`
- `crates/testkit/src/{fixtures.rs, install.rs}` (RDR2 base files)
- `crates/app/src/installations.rs`, `apps/desktop/src/bindings/`,
  `apps/desktop/src/features/installations/{labels.ts, testing.tsx}`
- `docs/roadmap.md`, `CHANGELOG.md`

## How to test

- `make check`: 567 Rust tests, 230 UI tests, and clippy of the Tauri shell for Windows.
- New tests:
  - `crates/store/tests/schema.rs`, `migration_from_v3_rebuilds_installations_without_losing_children`:
    data kept, `rdr2` accepted, foreign keys still RESTRICT and CASCADE. If the migration runs
    with foreign keys on, this test fails.
  - `crates/app/tests/rdr2.rs`: register, no GTA V signatures proposed, disable and enable a
    Lenny's Mod Loader mod back to the same bytes, a running `RDR2.exe` blocks changes.
  - Scanner: RDR2 scan with its own catalog, cross-game matcher, RDR2 processes.

## Notes and follow-ups

- Every RDR2 value is an assumption (A-09 to A-12) until checked against the file list of a real
  installation (`dir /s /b /a-d > C:\temp\rdr2-files.txt`).
- Step 2: guard per game, RDR2 signatures, online check rules for Red Dead Online.
- Step 3: UI grouped by game, and texts and app name not tied to GTA V.
