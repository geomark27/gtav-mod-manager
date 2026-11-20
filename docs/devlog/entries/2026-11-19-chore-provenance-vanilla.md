---
date: 2026-11-19
branch: feature/provenance-vanilla
type: feat
author: Marcos Ramos Jacome
agents: claude:orchestrator
pr: pending
version: unreleased
---

# Vanilla mode with restore and mod provenance

## What changes

- **Vanilla mode (Phase 4 R4).** A "Vanilla mode" card on the Dashboard and the Mods page:
  *Disable all mods (vanilla)* saves the current state of every mod as a **restore point**,
  then disables every managed mod in one journaled transaction (kind `profile`, the same guards
  and drift check as any profile). *Restore my mods* applies the restore point in one
  transaction and deletes it. Both dialogs preview the affected mods and repeat the online
  notice. A *Vanilla mode active* badge shows while the restore point exists and no mod is in
  the game folder.
- The card also says how many files the app does **not** manage stay in the game folder
  (candidates, unknown files and modified originals from the last scan), with a link to Review:
  vanilla mode only removes managed mods.
- Pressing vanilla again with nothing enabled keeps the existing restore point. Going vanilla
  again after re-enabling mods replaces it (one per installation). Mods adopted after the restore
  point are left alone by the restore. Drift or a running game blocks before the restore point
  is touched.
- The restore point is listed in Profiles with a *Restore point* badge.
- **Provenance (Phase 4 R5).** Every mod records where it came from: `manual` (adopted by
  hand), `zip_import` (archive file name and SHA-256 of the archive), and, for the coming
  recognition work, `recognized` / `grouped`. The mod details show a "Source:" line ("Imported
  from Mod v1.zip on …", "Adopted by hand on …").
- Schema v3 (`0003_provenance_restore_point.sql`): `mods.source_kind/detail/sha256/version`
  (existing mods become `manual`) and `profiles.kind` (`user` | `restore_point`) with a partial
  unique index (one restore point per installation).
- New commands `vanilla_status`, `enter_vanilla`, `restore_from_vanilla`; `ModDto.source` and
  `ProfileDto.kind` in the bindings.

## Why

First real use of v0.1.0 showed that the owner wants to switch every mod off before playing
online and get them back with one click, and to know where each mod came from. These are the
two Phase 4 steps that do not depend on the signature catalog (R1, blocked on A-08), and R3
needs the provenance columns.

## Files and areas

- `crates/domain`: `ModSource`, `ModSourceKind`, `ProfileKind`.
- `crates/store`: migration 0003, mods repository (source columns), profiles repository
  (`kind`, `restore_point`, `replace_restore_point`).
- `crates/engine`: `AdoptPayload.source` (old journals read as `manual`), `Plan::with_source`.
- `crates/app`: `vanilla.rs` (new), `mods.rs` (`ModSourceDto`), `import.rs` (archive hash),
  `profiles.rs` (`kind`, shared helpers).
- `apps/desktop/src-tauri`: three commands.
- UI: `features/vanilla/` (new), mod details source line, profiles badge, dashboard and mods
  page wiring, `lib/ipc.ts`.
- Docs: `docs/data-model.md`, `docs/roadmap.md`, `CHANGELOG.md`.

## How to test

- `make check` (Rust: 495 tests; UI: 212 tests). New: `crates/app/tests/vanilla.rs` (vanilla →
  restore leaves the game folder byte-identical, replace/keep rules, guards, drift),
  `crates/store/tests/schema.rs` (v2 → v3 backfill, CHECKs, one restore point),
  `features/vanilla/VanillaPanel.test.tsx`.
- On Windows, on a copy of an installation with managed mods: Dashboard → *Disable all mods
  (vanilla)* → mod files gone, badge shown → *Restore my mods* → files back.

## Notes and follow-ups

- The roadmap listed an `installed_at` column; it was left out because it would always equal
  `mods.created_at`.
- The Tauri shell (`gmm-desktop`) is not built on WSL (no webkit2gtk); the three commands follow
  the existing pattern and are compiled by the Windows build.
- `recognized` / `grouped` sources are written by R3; `Recognized as …` currently shows the
  signature id.
