---
date: 2026-11-19
branch: fix/bump-release-branch
type: fix
author: Marcos Ramos Jacome
agents: claude:orchestrator
pr: pending
version: unreleased
---

# One-step bump from develop or main

## What changes

- `make bump bump=minor` (or `ver=X.Y.Z`) run from `develop` or `main` now does the whole
  preparation: it fast-forwards `develop`, computes the version from `develop`, creates
  `release/vX-Y-Z` from it (through `make start`) and bumps there. Before, it stopped with a hint
  to create the branch by hand, and the suggested name (`release-X-Y-Z`) was rejected by
  `make start`.
- `DRY_RUN=1 make bump …` from `develop` or `main` shows the branch it would create. From
  `develop` it also shows the notes and the devlog count.
- New refusals: a release branch for that version that already exists (with the command to
  continue there), and a detached HEAD. On a work branch the behaviour does not change.
- `make help`, the `make release` hint and `docs/release.md` §3 describe the new flow.

## Why

Releasing took two commands and a branch name typed by hand. The guard keeps the rule that
`main` and `develop` only receive merges, but the bump does not need the user to type the
branch.

## Files and areas

- `scripts/bump.sh`, `Makefile`, `docs/release.md`

## How to test

Tested in a throwaway clone with a local bare `origin`:
- On `main`: `DRY_RUN=1 make bump bump=minor` printed `release/v0-2-0` and 0.2.0, with nothing
  created.
- On `main`: `make bump bump=minor` created `release/v0-2-0` from `develop` and wrote 0.2.0 to
  `Cargo.toml`, `package.json`, `tauri.conf.json` and `package-lock.json`. It also added
  `## [0.2.0]` to the CHANGELOG and wrote the devlog card.
- Run again from `develop`, it refused because the branch exists. It also refused with a dirty
  tree, and with `bump=` and `ver=` together.

## Notes and follow-ups

- `make bump` runs the script of the branch you are on. From `main`, the new behaviour only
  applies once this fix has been promoted to `main`. Until then, run it from `develop`.
- The devlog entry of a release branch (for example `release/v0-1-0`) is written after the
  version is closed, so it stays `unreleased` and goes out with the next version. This is a
  separate fix.
