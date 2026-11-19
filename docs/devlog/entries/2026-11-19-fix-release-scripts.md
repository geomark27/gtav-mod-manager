---
date: 2026-11-19
branch: fix/release-scripts
type: fix
author: Marcos Ramos Jacome
agents: claude:orchestrator
pr: "#5"
version: unreleased
---

# Fix make finish, make promote and the Tauri version mismatch

## What changes

- `make finish` works when the branch lives in the main folder (no worktree): it switches to
  `develop`, fast-forwards it and deletes the local branch. Before, it tried to remove the main
  working tree as if it were a task worktree (git refused, nothing was lost).
- `make promote` opens the `develop → main` PR when none exists. Before, an empty PR list made
  jq print `null null`, so it reported “PR #null is already open” and created nothing.

- The Windows installer builds again: `tauri build` refused to bundle because the npm packages
  were newer than the Rust crates (`@tauri-apps/api` 2.12 vs `tauri` 2.11, and the dialog,
  updater and process plugins). The npm packages are pinned with `~` to the crates' minor
  versions, and `scripts/check-tauri-versions.sh` now runs in `make check`, in CI (UI job) and in
  the release `prepare` job, so a mismatch fails early instead of at release time.

## Why

All three broke during the first release attempt (v0.1.0): finish and promote had to be done by
hand, and the release build failed, leaving the v0.1.0 release as an unpublished draft.

## Files and areas

- `Makefile` (`finish`, `check`), `scripts/promote.sh`, `scripts/check-tauri-versions.sh`
- `.github/workflows/ci.yml`, `.github/workflows/release.yml`
- `apps/desktop/package.json`, `apps/desktop/package-lock.json`

## How to test

- On a merged work branch in the main folder: `make finish` → back on an up-to-date `develop`,
  branch deleted.
- With no open promotion PR: `make promote` → asks and creates the PR.
- `bash scripts/check-tauri-versions.sh` → four pairs `ok`; bump one npm minor and it fails.
