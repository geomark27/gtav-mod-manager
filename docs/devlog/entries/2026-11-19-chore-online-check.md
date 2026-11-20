---
date: 2026-11-19
branch: feature/online-check
type: feat
author: Marcos Ramos Jacome
agents: claude:orchestrator
pr: pending
version: unreleased
---

# Before playing online: prepare and check

## What changes

- New **Before playing online** card on the Dashboard (Phase 5). It has two buttons:
  - **Prepare for online**, in one operation:
    1. reconciles the disk with the database and scans the folder;
    2. applies vanilla mode, which saves a restore point and disables every managed mod in one
       transaction;
    3. runs the checks.
    If the mods cannot be disabled (game running, drift), the checks still run and the card
    says why the mods were not disabled.
  - **Check only**: the same scan and checks, read-only for the game folder.
- Seven checks, each with a status (OK, Review or Fix), what it found (up to 50 items) and how to
  fix it, linking to Review or Mods when that helps:
  1. game and launchers closed;
  2. no recovery pending;
  3. no managed mod in the game folder;
  4. no other mod files;
  5. unrecognized files (warning);
  6. originals unchanged against the baseline (warning without one);
  7. no leftovers of interrupted operations (warning).
- Check 4 also reports files that a mod pattern matches but that count as original only because
  a trusted baseline captured them while mods were installed. They are marked "in the
  baseline".
- Verdict: *No mod files found*, *N things to review* or *N things to fix*. It never says that
  playing online is safe, the online notice is always visible, and original files are only
  reported, never repaired.
- Backend: `App::check_online_readiness`, `App::prepare_for_online` (`crates/app/src/online.rs`),
  `enter_vanilla_inner` so vanilla mode runs inside another operation. Tauri commands
  `check_online_readiness` and `prepare_for_online`.

## Why

The owner's goal is a fast way from story mode with mods to GTA Online without mods. Vanilla
mode only removes managed mods. Prepare for online also shows what vanilla mode cannot remove
(loose loaders, `.asi` files, modified originals), so nothing is left unnoticed.

## Files and areas

- `crates/app/src/online.rs` (new), `vanilla.rs` (inner step), `lib.rs`
- `apps/desktop/src-tauri/src/{commands,lib}.rs`, `apps/desktop/src/lib/ipc.ts`
- UI: `features/online/` (new), `features/dashboard/DashboardView.tsx`
- Docs: `docs/roadmap.md` (Phase 5), `docs/windows-validation.md` §7, `CHANGELOG.md`

## How to test

- `make check` (Rust 554 tests, UI 230 tests). New: `crates/app/tests/online.rs` (9 cases:
  read-only check, prepare with unmanaged leftovers, clear result, modified original never
  repaired, unknown files and leftovers only warn, running game, drift, mod files inside a
  baseline captured with mods, unknown installation), `features/online/OnlineCheckPanel.test.tsx`.
- On Windows: `docs/windows-validation.md` §7 on a copy.

## Notes and follow-ups

- Not done: "adopt and disable" directly from the check (it links to Detected mods), keeping
  the last result across restarts, and checks outside the game folder.
- Unrecognized files are a warning rather than a failure: while the catalog of original files
  is incomplete (A-05), some game files can show up as unrecognized.
