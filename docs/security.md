# Security and safety strategy

Status: v0.1 · 2026-11-18

The asset we protect is **the user's game installation**. The worst failure is losing or
corrupting an original file or a mod the user cannot get back. Every rule below exists to make
that impossible or detectable.

## Invariants

Each invariant is enforced in code **and** covered by at least one automated test
(see [testing.md](testing.md)). A change that weakens an invariant needs a Tech Lead decision.

| ID   | Invariant                                                                                                              | Enforced in              |
| ---- | ---------------------------------------------------------------------------------------------------------------------- | ------------------------ |
| I-01 | The engine never creates, renames or deletes a path classified `Original`, `OriginalModified` or listed in the catalog as original. | engine + app (Ownership guard) |
| I-02 | A game file is never deleted unless a vault object with the same SHA-256 exists and was verified in the same transaction. | engine (`TombstoneGameFile`) |
| I-03 | Every file-system mutation is preceded by a persisted journal step (`Started`).                                        | engine                   |
| I-04 | No transaction executes while a guarded process runs.                                                                  | app (Process guard)      |
| I-05 | Every resolved path stays inside the installation root or the vault; `..`, absolute paths, reparse points and hard links are rejected. | domain (`RelPath`) + platform + engine |
| I-06 | Renames inside game folders never overwrite.                                                                           | platform (`rename_no_replace`) |
| I-07 | The UI never shows a state that the database has not committed after verification.                                     | app + UI                 |
| I-08 | Files the engine did not record are never deleted automatically, including files that look like engine artifacts.     | engine (recovery)        |
| I-09 | Tests never touch a real installation: all destructive tests run inside `tempfile` directories.                        | testkit + review         |
| I-10 | The app never launches the game, mod tools or any executable from a game folder, and never reads or writes anti-cheat files. | review                |

## Threats and mitigations

| #    | Threat                                                                 | Mitigation                                                                                           |
| ---- | ---------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| T-01 | A game DLL is mistaken for a mod and removed.                          | Evidence-based classification; app acts only on user-confirmed mods; I-01; trusted publisher list.   |
| T-02 | Crash or power loss mid-operation leaves the folder half-changed.      | Two-phase transactions, write-ahead journal with `synchronous=FULL`, recovery before any action.     |
| T-03 | Vault copy is corrupt, so disabling loses the mod.                     | Hash verification of the vault object before tombstoning (I-02); objects read-only.                  |
| T-04 | A file changes outside the app (game update, launcher verify, user).   | Reconciliation before every transaction; `Drifted` blocks actions until the user decides.            |
| T-05 | Game/launcher holds files open or rewrites them during an operation.   | Process guard before planning and before executing.                                                  |
| T-06 | Path traversal or symlink trick makes the engine write elsewhere.      | `RelPath` validation, root confinement after resolution, reparse points and hard links rejected.      |
| T-07 | Two mods (or a mod and an original) target the same path.              | `PATH_CONFLICT` on enable; no-replace renames; enabled-path exclusivity check.                       |
| T-08 | Antivirus locks or quarantines a file mid-transaction.                 | Treated as I/O error → rollback; quarantined file appears as drift on next reconciliation.           |
| T-09 | Disk full during copy.                                                 | Space guard with margin; copy to temp name first, so failure leaves no partial target.               |
| T-10 | Two app instances operate on the same data.                            | Single-instance lock in the data dir; one transaction at a time.                                     |
| T-11 | User believes the game is safe for GTA Online after disabling mods.    | Persistent notice; no "online-safe" wording anywhere in the product.                                 |
| T-12 | Elevation misuse.                                                      | App runs as invoker; protected installations reported as `NOT_WRITABLE`; no silent elevation.        |

## Out of bounds (never implemented)

- Reading, modifying or bypassing BattlEye or any anti-cheat or DRM component.
- Modifying game executables or RPF archives in place.
- Network access for mods (no downloads in the MVP; Phase 3 integrations will be reviewed separately).
