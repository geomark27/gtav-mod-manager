---
date: 2026-11-19
branch: feature/mod-recognition
type: feat
author: Marcos Ramos Jacome
agents: claude:orchestrator
pr: pending
version: unreleased
---

# Mod recognition: signatures and grouping

## What changes

- **Known-mod signatures (Phase 4 R1).** `crates/scanner/catalog/mods/*.toml`, one file per
  mod, embedded in the binary: Script Hook V, ASI Loader, Script Hook V .NET, Rampage Trainer,
  Menyoo, Simple Trainer, Native Trainer, NativeUI, LemonUI, ReShade, ENBSeries, OpenIV ASI and
  the OpenIV `mods` folder. Each lists required patterns (`files`), `any_of` groups, `optional`
  patterns, `requires` (other signature ids), supported `editions` and `verified`.
- Loading validates every signature: id equals the file name, unique ids, known `requires`,
  valid globs and categories, and **no pattern may match an original file** of either catalog
  (D-14). A test checks that every TOML in the folder is embedded.
- `gmm_scanner::recognize`: matches the candidate/unknown files of a scan and returns named
  mods (`RecognizedMod`: files, confidence high/medium, missing optional patterns, requirements,
  edition support), ambiguous files and the unrecognized rest. A file belongs to at most one
  mod: the most specific signature wins (more required constraints, then more exact names); a
  tie leaves the file to nobody and reports it as ambiguous. A signature that loses a required
  file is dropped. Original files are never claimed.
- **Grouping (Phase 4 R2).** `gmm_scanner::group_candidates` turns the unrecognized files into
  low-confidence proposals: one per first-level folder under the root, `scripts/`, `plugins/`
  or `mods/`, and stem pairs joined with a folder of the same name (`x.asi` + `x.ini` + `x/`).
  Names are title-cased; the category is the most common one of the catalog mod patterns.
- `Glob::is_literal`.
- Nothing is visible in the app yet: R3 (Detected mods screen and bulk adoption) uses this.

## Why

After a scan the user sees loose candidate files and must build each mod by hand, which is the
hard part. Recognition turns them into proposals by name (“Rampage Trainer — requires Script
Hook V .NET”). Recognition only proposes; nothing is managed until the user adopts it.

## Files and areas

- `crates/scanner/catalog/mods/*.toml` (new, 13 signatures)
- `crates/scanner/src/recognize/` (new: `signature.rs`, `matcher.rs`, `group.rs`)
- `crates/scanner/src/glob.rs`, `crates/scanner/src/lib.rs`
- `crates/scanner/tests/recognize.rs` (new, end to end with `gmm-testkit`)
- `docs/roadmap.md`

## How to test

- `cargo test -p gmm-scanner` (88 unit + 2 integration tests): Rampage setup split into named
  mods, case-insensitive matching, required files, specificity and ambiguity, originals (catalog
  and trusted signature) never proposed, edition support, OpenIV folder as one item, similar
  names that must not match, grouping rules.
- `make check`.

## Notes and follow-ups

- **Every signature is `verified = false`** (A-08): the file names come from public knowledge,
  not from a real installation. Rampage Trainer's patterns are deliberately broad
  (`scripts/Rampage*.dll`). Close A-08 with read-only file lists from the owner's Legacy and
  Enhanced installations before v0.2.0.
- PE metadata rules (`product_name` / `file_description`) from the plan are not implemented:
  `OsProbe` reads only the file version today. Added later if file names prove insufficient.
- `update/**` is original in both catalogs, so DLC packs there are never grouped or adopted.
