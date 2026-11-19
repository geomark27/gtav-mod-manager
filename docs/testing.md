# Testing strategy

Status: v0.1 · 2026-11-18

## Rules

1. Every feature ships with tests in the same task. No test, no merge.
2. Destructive tests run only inside `tempfile::TempDir`. Hard-coded paths to real installations are forbidden in tests (I-09).
3. Fake installations are built with `gmm-testkit`, never copied from a real game.
4. Tests are deterministic: no sleeps, no network, no dependence on the host's processes or registry (those go through traits with fakes).

## Levels

| Level              | Where                         | Tooling                                         |
| ------------------ | ----------------------------- | ----------------------------------------------- |
| Unit               | each crate, `#[cfg(test)]`    | `cargo test`                                    |
| Integration        | `crates/*/tests/*.rs`         | `cargo test`, `gmm-testkit`, `tempfile`         |
| Fault injection    | `crates/engine/tests/`        | `FaultyFs` with `FaultPlan` (error or crash at the N-th matching operation) |
| Windows compile    | core crates                   | `cargo check --target x86_64-pc-windows-gnu`    |
| Windows runtime    | `#[cfg(windows)]` tests       | `cargo test` on Windows (milestone M7)          |
| Frontend           | `apps/desktop/src/**/*.test.tsx` | Vitest + Testing Library + `mockIPC`         |

## Fault injection model

`FaultyFs` wraps a real `StdFs` rooted in a temp dir. A `FaultPlan` says _which_ operation kind
(`copy`, `rename`, `remove`, `write`, `create_dir`, `any`) fails at _which_ occurrence, and
_how_:

- `Error` — returns an I/O error; the engine must roll back and leave the disk exactly as before.
- `Crash` — returns `SimulatedCrash`; the engine must stop immediately **without** rolling back
  (simulating power loss). The test then builds a fresh engine on the same database and
  directories, runs recovery, and asserts the final disk state.

For every transaction kind, tests iterate the crash point over **every** file-system operation
the transaction performs (exhaustive crash testing), asserting after recovery that the
installation equals either the pre-transaction or the post-transaction snapshot (never
something in between), and that I-01/I-02 hold.

## Mandatory scenarios → owning milestone

| Scenario                                   | Milestone | Main crate          |
| ------------------------------------------ | --------- | ------------------- |
| Detection of both editions                 | M2        | scanner             |
| Missing / incomplete installations         | M2        | scanner, app        |
| Mod detection (candidates)                 | M3        | scanner             |
| False positives (signed DLLs, catalog originals, look-alike names) | M3 | scanner |
| Duplicate files (same content, several paths/mods) | M3–M4 | scanner, engine |
| Activation / deactivation                  | M4        | engine              |
| SHA-256 integrity (vault and game copies)  | M4        | engine, platform    |
| Conflicts (same path, enable while occupied) | M4      | engine              |
| Interruption during operations             | M4        | engine (fault injection) |
| Recovery after unexpected shutdown         | M4        | engine (crash mode) |
| Protection of original files               | M4        | engine, app         |
| Externally modified files (drift)          | M6        | engine, app         |
| Switch never shows unverified state        | M5        | frontend            |
