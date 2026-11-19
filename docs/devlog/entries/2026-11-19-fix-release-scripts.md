---
date: 2026-11-19
branch: fix/release-scripts
type: fix
author: Marcos Ramos Jacome
agents: claude:orchestrator
pr: pending
version: unreleased
---

# Fix make finish and make promote

## What changes

- `make finish` works when the branch lives in the main folder (no worktree): it switches to
  `develop`, fast-forwards it and deletes the local branch. Before, it tried to remove the main
  working tree as if it were a task worktree (git refused, nothing was lost).
- `make promote` opens the `develop → main` PR when none exists. Before, an empty PR list made
  jq print `null null`, so it reported “PR #null is already open” and created nothing.

## Why

Both broke during the first release (v0.1.0); the steps had to be done by hand.

## Files and areas

- `Makefile` (`finish`), `scripts/promote.sh`

## How to test

- On a merged work branch in the main folder: `make finish` → back on an up-to-date `develop`,
  branch deleted.
- With no open promotion PR: `make promote` → asks and creates the PR.
