# Roadmap

Status: v0.1 · 2026-11-18 · Maintained by the Tech Lead only (agents never edit this file).

## Team model

| Role        | Who                                    | Does                                                              |
| ----------- | -------------------------------------- | ----------------------------------------------------------------- |
| Product     | Project owner                          | Approves milestones, confirms merges, decides product questions.  |
| Tech Lead   | Claude Opus 5.5 (high)                 | Architecture, roadmap, task specs, review, integration.           |
| Developers  | Claude Sonnet 5.5 (medium) subagents   | Write code and tests for one task; light verification of their own crate/module. |
| Runners     | opencode `deepseek-v4.1-flash-free`    | Dependency installs, full builds, full test/lint suites, reports. Never edit code. |

Git flow: `feature|fix|docs|release/*` branches into `develop`, promoted to `main` (see `docs/release.md`). Tasks in the same wave never share files.

## Phase 1 — MVP

| Milestone | Goal                                                                                         | Exit criteria                                                                                     | Status      |
| --------- | -------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- | ----------- |
| **M1**    | **Skeleton**: workspace, crates, schema v1, fault-injectable FS, testkit, app context, Tauri shell, UI shell with one real end-to-end command. | `cargo test` + clippy green on WSL; core crates `cargo check` for `x86_64-pc-windows-gnu`; UI typecheck/test/build green; Tauri app opens on Windows and shows real status data. | Done (W01 Windows run pending) |
| M2        | Installations: discovery (Steam/Epic/Rockstar/manual), structural validation, version, registration, writable check, selector UI. | Detection tests for both editions with fake installs; missing/incomplete cases; UI flow.          | Done |
| M3        | Scanning & classification: walker, catalog v0, evidence, hash cache, untrusted/trusted baseline, inventory UI (read-only). | False-positive suite; NFR-03 benchmark; inventory filters.                                        | Done |
| M4        | Operations engine: vault, journal, adopt/enable/disable, two-phase commit, recovery, guards. | Exhaustive crash tests green; invariants I-01…I-08 tested.                                        | Done |
| M5        | Mods UI: review & adopt flow, mods table, real switches with pending state, group switch, conflicts, search/filters. | UI tests incl. "switch never shows unverified state".                                             | Done |
| M6        | Reconciliation & drift, basic profiles with preview, activity history, dashboard stats.      | Drift scenarios; profile apply all-or-nothing tests.                                             | Done |
| M7        | Windows hardening: Authenticode, registry discovery, process probe, read-only validation on real installs, all ASSUMPTIONs resolved, installer. | Windows test run; assumptions table closed; signed-off installer build.                           | Partial — Windows run pending |

## Phase 2 — status (2026-11-18)

| Item | Status |
| ---- | ------ |
| ZIP import (safe staging, install-root mapping, imported as disabled) | Done — T23, T24, T25 |
| Mod dependencies (schema v2, closure as one transaction) | Done — T22, T24, T25 |
| Advanced profiles (three-way edit, preview, Vanilla built-in) | Done — T19, T20, T25 |
| Conflict detection (paths shared between mods, enable-time conflicts) | Done — T13, T15, T16 |
| Operation history | Done — T19, T20 |
| Vault garbage collection | Done — T22, T24, T25 |
| Unregister installation flow | Not done — needs a product decision (what happens to disabled mods' vault copies and history) |
| Elevated helper for protected folders | Not done — deliberately out of scope (D-10) |

## Phase 3 — status

| Item | Status |
| ---- | ------ |
| Mod metadata recognition | Seeded: PE file version of `.dll`/`.asi`/`.exe` entries (Windows) — T24. Full plan: **Phase 4 — Mod recognition** below |
| Broader installation types | Steam / Epic / Rockstar / manual + junctioned roots — T07, T09 |
| Integration with install tools (OpenIV, etc.) | Not started — requires those tools and their formats; `mods/` folder is already classified |
| Update detection | Not started — requires an online catalog/API (network access and a provider decision) |

---

## M1 — Skeleton: task board

| Task | Wave | Owner (files)                                   | Summary                                                              | Model        | Rounds | Status  |
| ---- | ---- | ----------------------------------------------- | -------------------------------------------------------------------- | ------------ | ------ | ------- |
| T00  | 0    | TL — root `Cargo.toml`, configs, docs           | Workspace manifest, lint/format config, documentation.               | Opus         | —      | Done    |
| T01  | 1    | `crates/domain/**`                              | Domain value objects, enums, entities, state machines + tests.       | Sonnet 5.5 medium | 1 | Done — 61 tests |
| T02  | 1    | `crates/platform/**`                            | `Fs` trait, `StdFs`, `FaultyFs`, SHA-256 streaming, no-replace rename, data dirs + tests. | Sonnet 5.5 medium | 1 | Done — 25 tests |
| T03  | 1    | `crates/store/**`                               | SQLite connection, pragmas, migration `0001_initial.sql` + tests.    | Sonnet 5.5 medium | 1 | Done — 10 tests |
| T04  | 1    | `apps/desktop/**` except `src-tauri/`           | React + TS + Vite UI shell: theme, layout, routes, primitives + tests. | Sonnet 5.5 medium | 1 + TL fix (jsdom ^29 for Node 24.14) | Done — 17 tests |
| R01  | 1    | — (runs commands only)                          | npm install, full Rust + UI verification, report.                    | opencode deepseek-v4.1-flash-free | 1 | Done — 8/8 PASS |
| T05  | 2    | `crates/testkit/**`                             | Fake installation builder + fixtures + tests.                        | Sonnet 5.5 medium | 1 | Done — 16 tests |
| T06  | 2    | `crates/app/**`, `apps/desktop/src-tauri/**`, `apps/desktop/src/features/status/**`, `apps/desktop/src/lib/ipc.ts` | App context, logging, lock, `AppError`, `get_app_status`, Tauri shell, status card wired end to end. | Sonnet 5.5 medium | 1 (also fixed AppShell test: QueryClientProvider) | Done — 13 Rust + 6 UI tests |
| R02  | 2    | — (runs commands only)                          | Full verification + Windows target check, report.                    | opencode deepseek-v4.1-flash-free | 1 | Done — 9/9 PASS; Windows has no Rust/Node yet |
| W01  | 2    | Project owner on Windows                        | `npm run tauri dev` on Windows; confirm the status card shows real data. | —         |        | Pending — install toolchain on Windows |

## M2–M7 task board

All code tasks by Sonnet 5.5 medium (1 round each unless noted); runners by opencode
`deepseek-v4.1-flash-free`.

| Task | Milestone | Owner (files) | Summary | Status |
| ---- | --------- | ------------- | ------- | ------ |
| T07 | M2 | `crates/scanner` | Catalogs, `OsProbe` (registry, PE version), Steam/Epic/Rockstar discovery, VDF parser, structural validation. | Done — 34 tests |
| T08 | M2 | `crates/domain`, `crates/store` | Domain `ts` feature (TS export of enums); installations + settings repositories. | Done — 75 + 23 tests |
| T09 | M2 | `crates/app`, `src-tauri`, UI installations | Discover/register (root canonicalized)/refresh/rename/select; dialog plugin; Installations page; sidebar selector. | Done — 34 Rust + 48 UI tests |
| R03 | M2 | runner | `@tauri-apps/plugin-dialog` install. | Done |
| T10 | M3 | `crates/scanner` | Anchored glob, catalog originals/mod patterns, scan + classification + evidence + hash cache, baseline capture, process probe. | Done — 66 tests |
| T11 | M3 | `crates/store` | Scans, inventory (bulk replace keeping first-seen), baseline, managed entries. | Done — 34 tests |
| T14 | M3 | `crates/app`, `src-tauri`, UI review | Operation slot + cancel, throttled progress events, scan/inventory/baseline services, Review page. | Done — 61 Rust + 91 UI tests |
| T12 | M4 | `crates/store` | Vault objects, mods, transactions journal, atomic `commit_transaction`. | Done — 40 tests |
| T13 | M4 | `crates/engine` | Vault, plans, two-phase execute, undo, recovery, reconcile. 96 fault-injection cases (48 error + 48 crash). | Done — 26 tests |
| R04 | M5 | runner | Radix Dialog + Checkbox install. | Done |
| T15 | M4/M5 | `crates/app`, `src-tauri`, `ipc.ts` | Startup recovery, guards, mod services (adopt/enable/disable/category/forget/reconcile), commands. | Done — 104 Rust tests |
| T16 | M5 | UI mods/adopt/guards | Mods page with real switches, group switch with preview, adopt flow from Review. | Done — 135 UI tests |
| T17 | M6 | `crates/store` | Profiles repository. | Done — 8 tests |
| T18 | M6 | `crates/engine`, `crates/store` | Drift resolution: restore (keeps user's version in vault) and re-adopt; 44 more fault cases. | Done |
| T19 | M6 | `crates/app`, `src-tauri`, `ipc.ts` | Profiles, history, dashboard, restore/re-adopt commands; managed-context fix. | Done — 130 Rust tests |
| T20 | M6 | UI dashboard/profiles/activity/settings | Remaining screens. | Done — 168 UI tests |
| T22 | P2 | `crates/domain`, `crates/store`, `crates/engine` | Schema v2 dependencies, imported (disabled) mods, vault GC (+ crash loop). | Done |
| T23 | P2 | `crates/archive` | Safe ZIP staging (zip-slip, links, bombs, encryption), root suggestion. | Done — 20 tests |
| T24 | P2 | `crates/app`, `src-tauri`, `ipc.ts` | Import, dependencies, vault, file versions. | Done — 157 Rust tests |
| T25 | P2 | UI import/dependencies/vault/profiles | Phase 2 screens. | Done — 194 UI tests |
| T21 | M7 | `crates/scanner` | Authenticode publisher via WinVerifyTrust (embedded signatures). | Done (Windows run pending) |
| R05 | all | runner | Full verification of the workspace. | Done — all PASS after TL fix (cfg(unix) on one app test) |

## Phase 4 — Mod recognition (planned, target v0.2.0)

Status: **code complete** (2026-11-19) — R1–R5 done (PRs #9, #10 and `feature/detected-mods`). Open: R6 (A-08 file lists, Windows run on a copy, release v0.2.0). Decided with the owner after the first use of
v0.1.0 on a real GTA V Enhanced installation that already had mods (Rampage Trainer).

### Problem

A user who already installed mods by hand sees, after a scan, a list of loose files marked
*Candidate* (“ASI plugin”, “Scripts folder”…). The app does not know that `ScriptHookV.dll`,
`dinput8.dll`, `ScriptHookVDotNet*` and `scripts/Rampage…` form **Rampage Trainer** and its
requirements, so the user must pick files and build each mod by hand. That is exactly the hard
part for a normal user.

### Goal

After a scan, the app proposes **named mods** (“Rampage Trainer — 8 files — requires Script
Hook V .NET”), the user adopts them with one action, and from then on every mod shows where it
came from and can be switched on/off — including a one-click **vanilla mode** to remove every
mod before playing online and a one-click **restore**.

### Principles (unchanged)

- Recognition only **proposes**; nothing is managed until the user adopts it (D-03).
- Adoption never moves files: they stay in place, a verified copy goes to the vault.
- Catalog originals always win (D-14): a signature can never claim a game file.
- No network: the signature catalog ships with the app and grows with releases.
- **Vanilla mode removes mod files from the game folder; it does not guarantee that playing
  online is safe.** The persistent notice stays; no screen may say “safe for online”.

### R1 — Known-mod signature catalog and matcher (`gmm-scanner`)

- Signature files `crates/scanner/catalog/mods/*.toml`, one per known mod:
  `id`, `name`, `category`, `editions` (legacy/enhanced/both), `requires` (other signature
  ids), match rules: `files` (required paths/globs, anchored like the catalog globs), `optional`
  files/folders, `any_of` groups, and optional PE metadata (`product_name` / `file_description`
  substrings from the version resource, Windows only).
- Matcher: input = scan inventory (candidate/unknown items); output =
  `RecognizedMod { signature_id, name, files, confidence: high|medium, missing_optional, requires }`.
  A file belongs to at most one recognition (most specific signature wins; ties → none,
  reported as ambiguous). Never matches `Original`/`OriginalModified` items.
- Initial signatures (names and paths verified on real installs — assumption **A-08**):
  Script Hook V, Script Hook V .NET (v2/v3/nightly), Ultimate ASI Loader / `dinput8.dll`,
  Rampage Trainer, Menyoo, Simple Trainer, Native Trainer, NativeUI, LemonUI, ReShade, ENB,
  OpenIV ASI (`OpenIV.asi`) and the OpenIV `mods/` folder (as one “OpenIV mods folder” item).
- Tests: fixtures built with `gmm-testkit` for each signature, overlap/ambiguity cases, false
  positives (game files with similar names), Legacy vs Enhanced.

### R2 — Grouping heuristics for unrecognized candidates (`gmm-scanner`)

- Group remaining candidates into proposed mods: one per first-level folder under `scripts/`,
  `plugins/`, `mods/`; stem pairs (`x.asi` + `x.ini` + `x/` folder); loose files alone.
- Suggested name from folder/stem (title-cased), suggested category from the catalog pattern.
- Confidence `low`; always editable before adopting.

### R3 — “Detected mods” review and bulk adoption (`gmm-app`, `gmm-engine`, UI)

- New Review tab **Detected mods**: cards per proposed mod (name, confidence badge, files,
  requirements, edition warning if the signature does not support this edition), editable name
  and category, include/exclude checkboxes, plus the existing per-file list for manual work.
- **Adopt all selected** (game and launchers closed): one transaction per mod, run in sequence
  with a summary (adopted / skipped with reason); requirements adopted first.
- Dependency links (`mod_dependencies`, Phase 2) created automatically from `requires` when both
  mods are managed.
- After adoption the scan re-runs and the recognized files show as *Managed by “Rampage
  Trainer”*.

### R4 — Vanilla mode and restore (`gmm-app`, UI)

- Dashboard and Mods page: **Disable all mods (vanilla)** → snapshots the current states into
  an automatic profile “Before vanilla — <date>” and applies Vanilla in one transaction (guards
  as usual: game closed, no drift). A persistent indicator **Vanilla mode active**.
- **Restore my mods** → applies that snapshot in one transaction. Only the latest snapshot is
  kept as the restore point; it is also listed in Profiles.
- Confirmation dialog repeats the online notice.

### R5 — Provenance (“where did this mod come from”) (`gmm-store` migration 0003, app, UI)

- Columns on `mods`: `source_kind` (`recognized` | `grouped` | `manual` | `zip_import`),
  `source_detail` (signature id or archive file name), `source_sha256` (archive hash for ZIP
  imports), `source_version` (signature/PE version when known), `installed_at`.
- Backfill existing mods as `manual`; ZIP imports record the archive name and hash.
- UI: “Source” line in mod rows/details (“Recognized as Rampage Trainer”, “Imported from
  RampageTrainer_v4.zip on 2026-11-19”, “Adopted by hand”).

### R6 — Validation and release v0.2.0

- Read-only collection of real file lists from the owner's installations (Legacy/Enhanced) to
  close **A-08**; never modify the real folder during development.
- Windows validation on a copy (docs/windows-validation.md §4) with Rampage: recognize → adopt
  all → vanilla → restore → files byte-identical.
- CHANGELOG `[Unreleased]` lines per item; `make bump bump=minor` → v0.2.0.

### Order and size

| Step | Depends on | Size | Owner |
| ---- | ---------- | ---- | ----- |
| R1 | — | M | scanner — **Done** (13 signatures, all `verified = false` until A-08; PE metadata rules deferred) |
| R2 | R1 | S | scanner — **Done** (bases: root, `scripts`, `plugins`, `mods`; `update/**` is original) |
| R5 | — | S | store + app — **Done** (schema v3; `installed_at` dropped: same as `created_at`) |
| R3 | R1, R2, R5 | M | app + UI — **Done** (Review tab, one transaction per mod, dependencies linked, one rescan) |
| R4 | — | S | app + UI — **Done** (restore point = `profiles.kind = 'restore_point'`) |
| R6 | all | S | owner + TL — checklist in `docs/windows-validation.md` §6 |

R1, R4 and R5 can run in parallel (different crates/screens).

### Out of scope (still)

Downloading mods or signatures from the internet, update detection for mods, OpenIV package
(`.oiv`) installation, anything that touches anti-cheat or promises online safety.

---

## Assumptions to verify (closed in M7 at the latest)

| ID   | Assumption                                                                        | Needed by |
| ---- | --------------------------------------------------------------------------------- | --------- |
| A-01 | Executable names: `GTA5.exe` (Legacy), `GTA5_Enhanced.exe` (Enhanced).           | M2        |
| A-02 | Steam app ids: Legacy `271590`, Enhanced `3240220`.                               | M2        |
| A-03 | Epic manifests location and `InstallLocation` field.                              | M2        |
| A-04 | Rockstar Games Launcher registry keys for both editions.                          | M2        |
| A-05 | Required base files per edition for structural validation.                       | M2        |
| A-06 | Trusted publisher subjects for Authenticode (Rockstar and shipped runtimes). Only embedded signatures are read (catalog-signed files return none). | M3 / M7   |
| A-07 | Process names of the game, launchers and Rockstar services.                      | M4 / M7   |
| A-08 | Real file names and paths of the mods in the Phase 4 signature catalog (Rampage Trainer, Script Hook V, SHVDN, ASI Loader, Menyoo, …) for Legacy and Enhanced. | Phase 4 R1 |
