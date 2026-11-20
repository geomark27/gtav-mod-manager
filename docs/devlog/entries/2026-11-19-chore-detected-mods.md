---
date: 2026-11-19
branch: feature/detected-mods
type: feat
author: Marcos Ramos Jacome
agents: claude:orchestrator
pr: pending
version: v0.2.0
---

# Detected mods: review and bulk adoption

## What changes

- **Review page tabs (Phase 4 R3).** Review opens on **Detected mods**. The previous per-file
  list, with manual selection and "Adopt as mod…", is now **All files**.
- Detected mods shows one card per proposal from the last scan:
  - known mods by name (badge *Known mod* or *Likely match*) and loose files grouped by folder
    or name (*Grouped*);
  - an editable name and category, the file list (unknown files marked), and each requirement
    as *managed as "X"*, *detected here* or *not found*;
  - warnings when the mod is not made for this edition or when its name is already taken.
  Known mods start selected; grouped proposals and taken names start unselected. Files claimed
  equally by two known mods are listed apart, to adopt by hand.
- **Adopt selected** adopts every chosen proposal:
  - one transaction per mod, requirements first, each recorded as `recognized` (with the
    signature id) or `grouped`;
  - dependency links created from the signatures (also to a mod adopted earlier by hand whose
    files match the required signature);
  - one rescan at the end, not one per mod.
  A proposal that fails is listed under *Not adopted* with its reason and the others still go
  in. An empty request, a file in two mods, an unknown signature, or a running game are refused
  before anything is adopted. Unknown files need the explicit confirmation, as in the manual
  flow.
- Backend: `App::detect_mods`, `App::adopt_detected`; Tauri commands `detect_mods` and
  `adopt_detected`; `ModSource::recognized/grouped`; `Signature::is_satisfied_by`;
  `commit_plan_with` (commit without the automatic rescan).

## Why

This is the screen the recognition engine (R1/R2) was built for: a user who installed
Rampage Trainer by hand gets "Rampage Trainer — requires Script Hook V .NET" and adopts
everything in one action, instead of picking loose files.

## Files and areas

- `crates/app/src/detect.rs` (new), `adopt.rs` (provenance and rescan options), `txn.rs`,
  `app.rs` (signatures loaded at start), `lib.rs`
- `crates/domain/src/mods.rs`, `crates/scanner/src/recognize/{signature,matcher}.rs`
- `apps/desktop/src-tauri/src/{commands,lib}.rs`
- UI: `features/detect/` (new), `pages/ReviewPage.tsx` (tabs), `lib/ipc.ts`, query
  invalidation after scans and adoptions
- Docs: `docs/roadmap.md`, `docs/windows-validation.md` §6, `CHANGELOG.md`

## How to test

- `make check` (Rust 538 tests, UI 221 tests). New: `crates/app/tests/detect.rs` (proposals,
  adopt all with provenance and dependencies, game folder untouched, managed requirements,
  partial failure, validation before adopting, unknown confirmation),
  `features/detect/DetectedModsPanel.test.tsx`, Review tab test.
- On Windows, follow `docs/windows-validation.md` §6 on a copy.

## Notes and follow-ups

- Every signature is still `verified = false` (A-08). Closing A-08 and the Windows run of §6
  are what is left of Phase 4 (R6) before `make bump bump=minor` → v0.2.0.
- Adopt never writes to the game folder, so the "game running" guard is the only write guard
  checked up front; each transaction still runs the usual guards.
