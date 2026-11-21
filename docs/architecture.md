# Architecture — ModSwitch (formerly GTA V Mod Manager)

Status: **v0.1 — approved baseline for Phase 1** · Last update: 2026-11-18 · Owner: Tech Lead

Labels used in this document:

- **FACT** — verified.
- **ASSUMPTION** — must be verified on a real Windows installation before the milestone that depends on it ships.
- **DECISION** — chosen design; change only through the Tech Lead.

---

## 1. Purpose

A Windows desktop application that detects, inventories and safely toggles **story-mode**
modifications for Grand Theft Auto V **Legacy** and **Enhanced**, without ever modifying,
overwriting or deleting original game files.

The product promise is _reversibility_: every change the app makes to a game folder can be
undone exactly, even after a crash or power loss.

## 2. Requirements

### 2.1 Functional (MVP)

| ID    | Requirement                                                                                                 |
| ----- | ----------------------------------------------------------------------------------------------------------- |
| FR-01 | Detect Legacy and Enhanced installations (Steam, Epic, Rockstar Games Launcher) and allow manual selection. |
| FR-02 | Validate an installation structurally (executable + base archives present) and read the executable version. |
| FR-03 | Support several installations at once; each one has its own inventory, mods, profiles and history.         |
| FR-04 | Scan an installation and classify every file (see §6).                                                    |
| FR-05 | Let the user confirm ownership of candidate files and group them into a **mod** with a category.            |
| FR-06 | Enable / disable a mod with a switch; the switch reflects disk state only after verification.               |
| FR-07 | Enable / disable every mod of a category in one transaction (group switch).                                 |
| FR-08 | Detect basic conflicts: two managed mods claiming the same path.                                            |
| FR-09 | Keep a verifiable backup (vault) of every managed mod file.                                                 |
| FR-10 | Journal every operation; roll back on error; recover after an unexpected shutdown.                          |
| FR-11 | Detect external changes (drift) to managed files and to the installation.                                   |
| FR-12 | Basic profiles per installation: save current mod states, preview, apply.                                  |
| FR-13 | Operation history (from the journal).                                                                       |
| FR-14 | Dashboard with statistics per installation.                                                                 |
| FR-15 | Search and filters over mods and inventory.                                                                 |

### 2.2 Non-functional

| ID     | Requirement                                                                                              |
| ------ | -------------------------------------------------------------------------------------------------------- |
| NFR-01 | Zero writes outside: the app data dir, the vault, and the root of a registered installation.             |
| NFR-02 | 100 % of interrupted transactions recovered to a consistent state in fault-injection tests.              |
| NFR-03 | Scan of 20 000 files with a warm hash cache in < 5 s on an SSD (cold scan does not hash; see §6.4).      |
| NFR-04 | UI stays responsive (< 100 ms input latency) during scans and transactions (all I/O off the UI thread).  |
| NFR-05 | Works as a standard (non-elevated) user; never requests elevation silently.                              |
| NFR-06 | Structured logs (JSON lines) with a transaction id on every engine event.                                |
| NFR-07 | Every feature ships with automated tests; destructive tests run only in temporary directories.           |

### 2.3 Out of scope (MVP)

- Editing RPF archives. Installing mods from archives (ZIP — Phase 2). Downloading mods.
- GTA Online support of any kind. Interacting with BattlEye or any anti-cheat.
- Launching the game or any mod tool.
- Elevated operation (installations under protected folders are reported as _not writable_).

## 3. Key decisions

| ID   | Decision                                                                                                                                                                                                                                                                                             |
| ---- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D-01 | **Stack:** Tauri 2 shell, React + TypeScript UI, Rust core split into crates, SQLite (rusqlite, bundled). Project language: English.                                                                                                                                                                  |
| D-02 | **Original-file identification is evidence-based** (§6): catalog of known originals per edition, Authenticode publisher for PE files, and an optional _trusted baseline_. No single signal (extension, name) decides.                                                                               |
| D-03 | **The app only acts on user-confirmed mods.** Unknown files are never touched. Ownership confirmation is an explicit UI step with its own audit record.                                                                                                                                              |
| D-04 | **A mod is a manifest** of file entries relative to the installation root (§5.4). Directories are expanded to files at adoption time.                                                                                                                                                               |
| D-05 | **Content-addressed vault**: every managed file is stored once by SHA-256 outside the game folder. The vault is the source of truth for mod content and the backup.                                                                                                                                  |
| D-06 | **Two-phase transactions with a write-ahead journal in SQLite** (§7). Prepare phase is fully reversible (no deletions); commit phase only deletes tombstones whose content is already in the vault. Recovery: _Open → roll back_, _Committing → roll forward_.                                       |
| D-07 | **Renames never overwrite.** All renames inside the game folder use a no-replace primitive.                                                                                                                                                                                                         |
| D-08 | **Group switch and profile apply are one transaction, all-or-nothing**, always preceded by a preview (dry-run plan).                                                                                                                                                                               |
| D-09 | **Conservative guards** block any transaction while the game or a launcher runs, when the installation is not writable, when disk space is insufficient, or when drift is unresolved.                                                                                                              |
| D-10 | **Never elevate in the MVP.** Protected installations are detected and explained.                                                                                                                                                                                                                  |
| D-11 | **The UI is never optimistic.** It renders state read back from the database after the engine commits.                                                                                                                                                                                              |
| D-12 | **Development on WSL2, product on Windows.** Core crates are platform-agnostic and fully tested on Linux; Windows-only code lives behind traits in `gmm-platform` with `#[cfg(windows)]` implementations. The Tauri shell is built and run on Windows.                                             |
| D-13 | **Typed IPC**: Rust DTOs are exported to TypeScript with `ts-rs`; the UI never hand-writes backend types.                                                                                                                                                                                          |
| D-14 | **Catalog originals win over the baseline.** A path matching a catalog original (file or glob) is `Original` even when a trusted baseline does not list it. Misclassifying a mod as original only makes it non-adoptable (safe); the reverse could expose a game file to operations. The engine independently refuses protected paths. |

## 4. System overview

```
┌──────────────────────────── apps/desktop (Windows) ─────────────────────────────┐
│  React + TS UI  ──invoke/events──►  src-tauri (gmm-desktop): thin command layer │
└──────────────────────────────────────────────┬──────────────────────────────────┘
                                               │
                                     ┌─────────▼─────────┐
                                     │      gmm-app      │  use cases, guards,
                                     │  (application)    │  error mapping, DTOs
                                     └──┬──────┬──────┬──┘
                         ┌──────────────┘      │      └──────────────┐
                ┌────────▼───────┐   ┌─────────▼────────┐   ┌────────▼───────┐
                │  gmm-scanner   │   │    gmm-engine    │   │   gmm-store    │
                │ detect, scan,  │   │ vault, journal,  │   │ SQLite schema, │
                │ classify       │   │ transactions,    │   │ migrations,    │
                │                │   │ recovery         │   │ repositories   │
                └────────┬───────┘   └─────────┬────────┘   └────────┬───────┘
                         └──────────────┬──────┴─────────────────────┘
                              ┌─────────▼─────────┐     ┌──────────────────┐
                              │   gmm-platform    │     │    gmm-domain    │
                              │ Fs, hashing, OS   │     │ pure types, rules│
                              │ probes (Windows)  │     │ (used by all)    │
                              └───────────────────┘     └──────────────────┘
                 gmm-testkit (dev-only): fake installations, fault plans, fixtures
```

### 4.1 Crates

| Crate          | Path                       | Responsibility                                                                                         | May depend on                       |
| -------------- | -------------------------- | ------------------------------------------------------------------------------------------------------ | ----------------------------------- |
| `gmm-domain`   | `crates/domain`            | Entities, value objects (`RelPath`, `Sha256`), enums, state machines, domain errors. **No I/O.**      | —                                   |
| `gmm-platform` | `crates/platform`          | `Fs` trait (+ `StdFs`, `FaultyFs`), streaming SHA-256, no-replace rename, process probe, Authenticode, registry, data dirs. | —                                   |
| `gmm-store`    | `crates/store`             | SQLite connection, pragmas, migrations, repositories.                                                  | domain                              |
| `gmm-scanner`  | `crates/scanner`           | Installation discovery & validation, file walking, classification pipeline, catalogs.                  | domain, platform                    |
| `gmm-engine`   | `crates/engine`            | Vault, transaction planner, executor, journal, recovery, reconciliation.                               | domain, platform, store             |
| `gmm-app`      | `crates/app`               | Use-case services, guards, single-writer lock, error mapping, DTOs (`ts-rs`), progress events.         | domain, platform, store, scanner, engine |
| `gmm-testkit`  | `crates/testkit`           | Fake installation builder, fixtures, helpers. **dev-dependency only.**                                 | domain, platform                    |
| `gmm-desktop`  | `apps/desktop/src-tauri`   | Tauri 2 shell: commands map 1:1 to `gmm-app` services. No business logic.                              | app                                 |

Rules:

1. Dependencies point downward only (diagram above). `gmm-domain` and `gmm-platform` never depend on another `gmm-*` crate.
2. Only `gmm-engine` mutates game folders. Only `gmm-store` touches SQLite.
3. `unsafe` is allowed only in `gmm-platform`, each block with a `// SAFETY:` comment.
4. The workspace `default-members` excludes `gmm-desktop`, so `cargo test` runs on WSL without WebKitGTK.

## 5. Domain model

### 5.1 Value objects

- `RelPath` — path relative to an installation root. Stored with `/` separators. Rejects: empty, absolute, drive letters, UNC, `..`, `.` components, empty components, NUL, trailing dot/space in a component (Windows), reserved device names (`CON`, `NUL`, `COM1`…). `key()` returns the lowercase comparison key (Windows paths are case-insensitive).
- `Sha256` — 64 lowercase hex chars.
- Ids — UUID v7 newtypes: `InstallationId`, `ModId`, `ProfileId`, `TransactionId`, `ScanId`.

### 5.2 Installation

```
Installation { id, edition: Legacy|Enhanced, platform: Steam|Epic|Rockstar|Unknown,
               root: absolute path, root_key, exe_version: Option<String>,
               status: Ok|Missing|Incomplete|NotWritable,
               baseline: None|Untrusted|Trusted, created_at, updated_at }
```

- Edition is decided by the executable found at the root: `GTA5.exe` → Legacy, `GTA5_Enhanced.exe` → Enhanced. **ASSUMPTION** (verify exact names on both editions).
- Two installations of the same edition are allowed (e.g. Steam + Epic). The same `root_key` cannot be registered twice.

### 5.3 Inventory and classification

Each scanned file becomes an `InventoryItem { rel_path, size, mtime, sha256?, class, evidence[] }`.

| Classification     | Meaning                                                                                   | Engine may act? |
| ------------------ | ----------------------------------------------------------------------------------------- | --------------- |
| `Original`         | Part of the game (catalog, trusted baseline, or trusted publisher signature).             | Never           |
| `OriginalModified` | Original path whose content differs from the trusted baseline (e.g. edited RPF).          | Never           |
| `Candidate`        | Looks like a mod (pattern evidence), not yet confirmed by the user.                       | No (adopt only) |
| `Unknown`          | No evidence either way.                                                                   | No (adopt only, extra confirmation) |
| `Managed`          | Entry of a confirmed mod, content matches the vault.                                      | Yes             |
| `Drifted`          | Entry of a confirmed mod, content changed outside the app.                                | Blocked until resolved |
| `EngineArtifact`   | Temp file or tombstone created by the engine (`*.gmm-tmp-*`, `*.gmm-tomb-*`; `<tx8>` = last 8 hex chars of the transaction UUID).             | Recovery only   |

`Evidence` is a list of reasons (`CatalogOriginal`, `BaselineMatch`, `BaselineMismatch`, `SignedBy(publisher)`, `Unsigned`, `ModPattern(name)`, `ReparsePoint`, `HardLinked`, `ManagedBy(mod_id)`), persisted as JSON and shown in the UI so the user understands every classification.

### 5.4 Mod

```
Mod      { id, installation_id, name, category, state, mechanism: LooseFiles, notes, created_at, updated_at }
ModEntry { id, mod_id, rel_path, path_key, sha256, size }
ModState = Enabled | Disabled | Drifted | Error
Category = GraphicsTextures | Vehicles | ScriptsGameplay | MapsBuildings
         | CharactersClothing | SoundsMusic | Uncategorized
```

- `mechanism` is the extension point for future install mechanisms (`OpenIvModsFolder`, `ArchiveImport`, `ExternalTool`). The MVP implements only `LooseFiles`: files placed directly in the installation tree.
- A path may belong to several mods (alternative versions), but **at most one of them may be enabled**. Enabling a mod whose path is occupied fails with `PATH_CONFLICT`.

### 5.5 Profile

`Profile { id, installation_id, name }` + `ProfileEntry { profile_id, mod_id, desired: Enabled|Disabled }`.
Suggested names: Vanilla, Visual Overhaul, Vehicle Collection, Immersive Gameplay, World Expansion, Complete Experience. _Vanilla_ is computed (all managed mods disabled), not stored. Applying a profile builds one transaction (D-08).

### 5.6 State machines

Transaction:

```
Open ──(all steps prepared)──► Committing ──(all tombstones deleted)──► Committed
  │                                  │
  └──(error)──► RollingBack ──► RolledBack        (crash in Committing ⇒ recovery rolls forward)
                    │
                    └──(rollback step fails)──► NeedsAttention
```

Step: `Pending → Started → Done → (Undone)`. A step is persisted as `Started` **before** its file-system effect and as `Done` **after** the effect is verified.

## 6. Classification pipeline

### 6.1 Evidence sources

1. **Catalog** (`crates/scanner/catalog/{legacy,enhanced}.toml`): known original paths and glob patterns per edition, plus known mod patterns with a suggested category. Ships with the app, versioned. Initial content is conservative and **ASSUMPTION**-marked; it grows from trusted baselines captured on real installations.
2. **Authenticode** (Windows only): for PE files (`.exe`, `.dll`, `.asi`), the signer subject. Publishers on the trusted list (initially `Rockstar Games, Inc.`, `Take-Two Interactive Software, Inc.`, plus signed third-party runtimes shipped with the game — **ASSUMPTION**) count as original evidence. Unsigned is _not_ evidence of a mod.
3. **Baseline**: a snapshot (path, size, SHA-256) taken when an installation is registered.
   - `Trusted` only if the user declares they just verified the game files with the launcher (UI explains how). Then any non-baseline file is a mod/unknown, and a baseline file with a different hash is `OriginalModified`.
   - `Untrusted` otherwise: used only for change detection, never as proof of originality.

### 6.2 Rules (first match wins)

1. Engine artifact name pattern → `EngineArtifact`.
2. Path belongs to a mod entry → `Managed` if hash matches, else `Drifted`.
3. Path in catalog originals or trusted baseline → `OriginalModified` if trusted-baseline hash differs, else `Original`.
4. PE file signed by a trusted publisher → `Original`.
5. Matches a catalog mod pattern → `Candidate` (+ suggested category).
6. Otherwise → `Unknown`.

Reparse points (symlinks, junctions) and hard-linked files (link count > 1) are recorded with evidence and **can never be adopted** (ownership is ambiguous).

### 6.3 Adoption (ownership confirmation)

User selects candidate/unknown items → app shows a manifest preview (paths, sizes, total vault usage) → user confirms (Unknown items need an extra explicit checkbox) → engine copies each file into the vault and verifies it. **The game folder is not modified by adoption.** The new mod is `Enabled`.

Grouping helper (suggestion only): files sharing a stem (`foo.asi` + `foo.ini`), or under the same first-level folder of `scripts/`, `plugins/`, `mods/`.

### 6.4 Hashing strategy

- Streaming SHA-256, 1 MiB buffer, off the UI thread, cancellable.
- Hash cache keyed by `(path_key, size, mtime)`; a scan only hashes when the key changed **and** the hash is needed (managed entries, baseline comparison, adoption).
- Large files (> 256 MiB, typical for RPF copies under `mods/`) show progress and require confirmation before adoption because of vault size.

## 7. Operations engine

### 7.1 Vault

- Location: `%LOCALAPPDATA%\GtavModManager\vault` by default (dev: XDG data dir), configurable.
- Layout: `objects/<first 2 hex>/<sha256>`. Written to a temp name, flushed, hash-verified, then renamed; marked read-only.
- One vault for all installations (deduplicated by content). An object is garbage only when no `mod_entries` row references it — GC is manual (Phase 2).
- The vault is outside every game folder; volume does not matter because atomic steps happen inside the game folder (see 7.2).

### 7.2 Steps

| Step                 | Prepare (reversible)                                                                                          | Undo                                         | Commit                 |
| -------------------- | ------------------------------------------------------------------------------------------------------------- | -------------------------------------------- | ---------------------- |
| `CaptureToVault`     | Copy game file → vault object (if absent); verify hash.                                                       | Delete object only if this step created it.  | —                      |
| `EnsureDir`          | Create missing parent dir inside the root; record that it was created.                                        | Remove it if this step created it and empty. | —                      |
| `PlaceFromVault`     | Verify object hash → copy to `<target>.gmm-tmp-<tx8>` → flush → verify → **no-replace** rename to `<target>`. | Verify hash, delete `<target>`.              | —                      |
| `TombstoneGameFile`  | Verify `<target>` hash == expected **and** vault object verified → no-replace rename to `<target>.gmm-tomb-<tx8>`. | No-replace rename tombstone back.      | Delete tombstone.      |

- **Enable mod** = `EnsureDir*` + `PlaceFromVault` per entry (fails with `PATH_CONFLICT` if the target exists).
- **Disable mod** = `TombstoneGameFile` per entry. Commit deletes tombstones.
- **Adopt** = `CaptureToVault` per entry.
- Group switch / profile = concatenation of the above for every affected mod, in one transaction.

### 7.3 Journal and recovery

- `transactions` and `transaction_steps` tables (see [data-model.md](data-model.md)); SQLite with `journal_mode=WAL`, `synchronous=FULL`.
- Before the UI enables any action at startup, `RecoveryService` runs:
  - `Open` / `RollingBack` → undo `Done` and `Started` steps in reverse order (each undo checks real disk state first, so it is idempotent) → `RolledBack`.
  - `Committing` → delete remaining tombstones → `Committed`.
  - Any undo that cannot be proven safe → `NeedsAttention`; actions for that installation stay blocked and the UI explains what to do.
- Files matching engine artifact patterns that are **not** referenced by the journal are reported, never deleted (D-03, uncertainty ⇒ block).

### 7.4 Concurrency

One transaction at a time per app instance (async mutex in `gmm-app`). A single-instance lock file in the data dir prevents two app instances from sharing the database.

### 7.5 Reconciliation (drift)

On startup, after recovery, and before every transaction, for each managed entry of the affected installation: expected present (enabled) / absent (disabled). Fast check by size + mtime; hash on mismatch. Mismatch marks the mod `Drifted`; options offered to the user: _restore from vault_, _re-adopt current content_, _forget mod_ (removes management, leaves files as they are).

## 8. Installation discovery

| Source          | Where                                                                                                                                         | Status         |
| --------------- | --------------------------------------------------------------------------------------------------------------------------------------------- | -------------- |
| Steam           | `HKCU\Software\Valve\Steam\SteamPath` → `steamapps/libraryfolders.vdf` → `appmanifest_<appid>.acf`. App ids: Legacy `271590`, Enhanced `3240220`. | ASSUMPTION (Enhanced app id) |
| Epic            | `%ProgramData%\Epic\EpicGamesLauncher\Data\Manifests\*.item` (JSON, `InstallLocation`).                                                        | ASSUMPTION     |
| Rockstar        | Registry keys under `HKLM\SOFTWARE\WOW6432Node\Rockstar Games\` (`InstallFolder`).                                                              | ASSUMPTION     |
| Manual          | Folder picker.                                                                                                                                | —              |

Structural validation (not a DRM check): executable present, required base files present per the catalog (e.g. `common.rpf`, `x64a.rpf`, `update/update.rpf` — **ASSUMPTION**, especially for Enhanced), version read from the PE version resource. Result: `Ok`, `Incomplete(missing list)`, `Missing`, `NotWritable`.

## 9. Guards

Evaluated in `gmm-app` before planning and again right before executing a transaction.

| Guard            | Blocks when                                                                                                         | Error code              |
| ---------------- | ------------------------------------------------------------------------------------------------------------------- | ----------------------- |
| Process          | Any of: `GTA5.exe`, `GTA5_Enhanced.exe`, `PlayGTAV.exe`, `GTAVLauncher.exe`, Rockstar launcher processes (**ASSUMPTION**: exact names). | `GAME_RUNNING`          |
| Writable         | Probe file cannot be created/deleted in the installation root.                                                      | `INSTALL_NOT_WRITABLE`  |
| Space            | Free space < required bytes + 512 MiB margin (game volume for enable, vault volume for adopt).                       | `INSUFFICIENT_SPACE`    |
| Drift            | Any affected mod is `Drifted`.                                                                                      | `DRIFT_DETECTED`        |
| Recovery         | The installation has a `NeedsAttention` transaction.                                                                 | `RECOVERY_REQUIRED`     |
| Ownership        | A target path is `Original`, `OriginalModified`, a reparse point or hard-linked.                                    | `OWNERSHIP_UNCERTAIN`   |
| Confinement      | A resolved path escapes the installation root.                                                                      | `PATH_OUTSIDE_ROOT`     |

## 10. Errors and logging

- Each crate defines its own `thiserror` enum. `gmm-app` maps them to `AppError { code, message, hint?, details? }`, serialized to the UI. Codes are stable `SCREAMING_SNAKE_CASE` strings (§9 plus `PATH_CONFLICT`, `HASH_MISMATCH`, `NOT_FOUND`, `INVALID_INPUT`, `IO`, `DATABASE`, `INTERNAL`).
- No `unwrap`/`expect` outside tests. Panics are bugs.
- `tracing` with JSON lines to `<data>/logs/gmm.<date>.jsonl` (daily rotation, 14 files kept) plus a human console layer in dev. Engine spans carry `tx_id`, `installation_id`, `step`.

## 11. IPC contract

- Tauri commands are `snake_case` and map 1:1 to `gmm-app` services; every command returns `Result<Dto, AppError>`.
- DTOs use `#[serde(rename_all = "camelCase")]` and `#[derive(ts_rs::TS)]`, exported to `apps/desktop/src/bindings/`.
- Long operations (scan, adopt, transactions) emit progress events `gmm://progress` `{ operationId, phase, done, total }`; the command resolves only when the operation is finished and verified.

## 12. Frontend

- React + TypeScript (strict) + Vite. Routing: `react-router` (hash router). Server state: TanStack Query (invalidated after every command). No global client store unless proven necessary.
- Styling: CSS custom properties (dark theme tokens) + CSS modules. Accessible primitives from Radix UI (Switch, Dialog, Dropdown, Tooltip). Icons: `lucide-react`.
- Screens: Dashboard · Mods · Review (candidates/unknown → adopt) · Profiles · Activity (history) · Installations · Settings. Installation selector in the sidebar header.
- **Switch UX**: `off | on | pending | blocked | error`. A click sets `pending`; the final state comes from the command result. Blocked switches show the reason in a tooltip.
- Persistent notice: _“Mods managed here are for story mode. Disabling mods does not make an installation safe for GTA Online.”_
- **No decorative UI**: a control exists only when its feature works end to end. Screens of unbuilt milestones show a plain “Planned for milestone Mx” notice with no controls.
- Tests: Vitest + Testing Library; backend mocked with `@tauri-apps/api/mocks` (`mockIPC`).

## 13. Phase 2 additions

- **ZIP import** (`gmm-archive`): archives are staged into `<data>/staging` with zip-slip, link,
  encryption, size and compression-ratio checks; the user picks the install root inside the
  archive; files go to the vault and the mod is registered **Disabled** (no game-folder change);
  enabling uses the normal journaled path with conflict checks.
- **Dependencies**: `mod_dependencies` table; plans validate final states; the app offers the
  dependency closure (“also enable/disable …”) as one transaction.
- **Vault GC**: deletes only objects unreferenced by mod entries and by unresolved
  transactions; stray files are reported, never deleted.
- **Entry metadata**: PE file version of `.dll`/`.asi`/`.exe` entries (Windows).

## 14. Development environment

- Dev host: WSL2 (Linux). Core crates: `cargo test` on Linux, `cargo check --target x86_64-pc-windows-gnu` to keep `#[cfg(windows)]` code compiling.
- Desktop shell: built and run on Windows (Rust MSVC toolchain, Node LTS, Visual Studio Build Tools, WebView2).
- Package manager: npm.
