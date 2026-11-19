# Roadmap

Status: v0.1 · 2026-11-18 · Maintained by the Tech Lead only (agents never edit this file).

## Team model

| Role        | Who                                    | Does                                                              |
| ----------- | -------------------------------------- | ----------------------------------------------------------------- |
| Product     | Project owner                          | Approves milestones, confirms merges, decides product questions.  |
| Tech Lead   | Claude Opus 5.5 (high)                 | Architecture, roadmap, task specs, review, integration.           |
| Developers  | Claude Sonnet 5.5 (medium) subagents   | Write code and tests for one task; light verification of their own crate/module. |
| Runners     | opencode `deepseek-v4.1-flash-free`    | Dependency installs, full builds, full test/lint suites, reports. Never edit code. |

Working copy is local only (no git for now). Tasks in the same wave never share files.

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
| Mod metadata recognition | Seeded: PE file version of `.dll`/`.asi`/`.exe` entries (Windows) — T24 |
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
