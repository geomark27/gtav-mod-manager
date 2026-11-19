# AGENTS.md — rules for every coding agent

Read this file completely before writing code. It overrides your defaults.

## 1. Context

GTA V Mod Manager: Windows desktop app (Tauri 2 + React/TypeScript + Rust + SQLite) that safely
toggles story-mode mods for GTA V Legacy and Enhanced. **Safety of the user's files is the
product.** Read `docs/architecture.md` (design), `docs/data-model.md` (schema),
`docs/security.md` (invariants I-01…I-10) and `docs/testing.md` before touching related code.

Project language: **English** for code, comments, identifiers, UI text, docs and reports.

## 2. How work is organized

- You receive one task with an explicit list of files/folders you **own**. Create or modify only
  those. If you need a change elsewhere, do not make it: write it in your report.
- Other agents work in the same folder at the same time on other files. Never reformat, move or
  delete files you do not own. Never run `cargo fmt` on the whole workspace: use
  `cargo fmt -p <your-crate>`.
- The project is a git repo (GitHub, `main` + `develop`, work on `feature|fix|docs|release/*`
  branches). Agents never run git commands that write (`commit`, `push`, `switch`, `branch`,
  `tag`, `merge`, `rebase`, `reset`, `stash`) nor `gh` write commands: the owner and the Tech
  Lead do that (see `docs/release.md`). Read-only git (`status`, `diff`, `log`) is fine.
- Do not edit `docs/devlog/**`: end your report with a `DEVLOG CONTRIBUTION` block (`agent:`,
  `what:`, `why:`, `files:`, `test:`, `follow-up:`); the Tech Lead writes the branch entry.
- Do not edit: `Cargo.toml` (root), `AGENTS.md`, `docs/**`, `.delegar/**`, `opencode.json`.
- Do not install system packages. Rust crates must come from `[workspace.dependencies]` in the
  root `Cargo.toml` (`dep = { workspace = true }`). If you need a crate that is not there, stop
  and report it. npm dependencies are installed by the runner, not by you.
- Never invent requirements. If the task is ambiguous, choose the safest option, implement it,
  and explain the choice in the report.

## 3. Rust conventions

- Edition 2024, workspace lints (`[lints] workspace = true` in every crate manifest, except
  `gmm-platform`, which declares its own lints to allow `unsafe`).
- No `unwrap()`, `expect()`, `panic!`, `todo!`, `unimplemented!` outside tests.
- Errors: one `thiserror` enum per crate (`Error`) and `pub type Result<T> = std::result::Result<T, Error>;`.
- Public items have doc comments (`///`). Modules start with a `//!` summary.
- Prefer small modules: one concept per file. `lib.rs` only declares modules and re-exports.
- Serialize enums with `#[serde(rename_all = "snake_case")]`; every persisted enum has
  `as_str()` and `FromStr` that match the SQL `CHECK` values in `docs/data-model.md` exactly.
- Paths inside an installation are always `gmm_domain::RelPath`, never raw strings.
- Time: `chrono::DateTime<Utc>`; ids: UUID v7 newtypes.
- Verify your crate with:
  ```
  cargo fmt -p <crate> -- --check
  cargo clippy -p <crate> --all-targets -- -D warnings
  cargo test -p <crate>
  ```

## 4. TypeScript / React conventions

- `strict: true`, no `any`, no `@ts-ignore`. Backend types come from `src/bindings/` (generated
  by `ts-rs`); never hand-write a type that mirrors a Rust DTO.
- Function components + hooks. Data from the backend via TanStack Query hooks in
  `src/features/<feature>/`. All Tauri calls go through `src/lib/ipc.ts`.
- Styling: CSS modules + tokens from `src/styles/tokens.css`. No inline styles except dynamic values.
- **No decorative UI.** Do not add a button, switch, menu item or input whose action is not
  implemented end to end. Unbuilt screens show only the “Planned for milestone Mx” notice.
- Tests next to the code: `Component.test.tsx` (Vitest + Testing Library).

## 5. Safety rules (non-negotiable)

- Tests that create, move or delete files do it only under `tempfile::TempDir` (Rust) or
  in-memory mocks (TS). Never reference real game paths such as `C:\Program Files\...`.
- Never write code that launches executables, touches anti-cheat files or edits RPF archives.
- Every task ships with tests for what it adds.

## 6. Report

When you finish, write `.delegar/reports/<task-id>.md` with:

1. Files created/modified.
2. Each verification command and its result (pass/fail, counts).
3. Decisions you made and why.
4. What you did not do, and anything you need from other tasks.
