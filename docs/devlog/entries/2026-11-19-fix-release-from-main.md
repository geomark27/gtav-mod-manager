---
date: 2026-11-19
branch: fix/release-from-main
type: fix
author: Marcos Ramos Jacome
agents: claude:orchestrator
pr: pending
version: v0.3.0
---

# Release from main in one command

## What changes

- `make release bump=patch|minor|major` (or `ver=X.Y.Z`) on `main` now does the whole release:
  - checks the code (fmt, clippy, Rust tests and UI);
  - raises the version: the version files, the lockfiles, the `CHANGELOG.md` section and the
    devlog card;
  - commits `release: vX.Y.Z` on `main` and pushes it;
  - tags and publishes;
  - fast-forwards `develop` to `main`.
  It asks once, `yes=1` skips the question and `DRY_RUN=1` simulates.
- `make release` with no arguments still publishes the version already in `main`.
- `make bump` is only for work branches again. On `main` or `develop` it points to
  `make release bump=…`. It no longer creates `release/*` branches: that path, added in
  `fix/bump-release-branch`, is removed.
- `docs/release.md` §3–4, `CLAUDE.md` and `make help` describe the new flow.

## Why

The owner wants releases made from `main` with a single command, with no release branch, PR or
promotion. The previous step (`make bump` creating `release/vX-Y-Z`) confused things: run again
after a release, it created `release/v0-3-0`. Its `[Unreleased]` was empty, so the branch was
left with nothing in it. Running the checks inside `make release` replaces the CI run on the
promotion PR, which no longer happens before a release.

## Files and areas

- `scripts/release.sh`, `scripts/bump.sh`, `Makefile`, `docs/release.md`, `CLAUDE.md`

## How to test

Tested in a throwaway clone with a local bare `origin` and a fake `gh`:
- `DRY_RUN=1 make release bump=patch` showed 0.2.0 → 0.2.1 and changed nothing.
- `yes=1 make release bump=patch` passed the checks (~30 s with a warm build cache). It wrote
  0.2.1 everywhere, closed the CHANGELOG and the devlog, pushed `release: v0.2.1` to `main`,
  pushed tag v0.2.1, called `gh release create`, and left `develop` equal to `main` with a clean
  tree.
- A fmt error stopped it at the checks: no commit, no tag, no changes.
- It refused: `make bump` on `main` and `develop`, `make release bump=…` off `main`, and a tag
  that already exists.

## Notes and follow-ups

- The Windows tests and the Tauri shell build still only run in CI (`release.yml` builds the
  shell on Windows). A failure there stops the release run, after the tag already exists.
- A release made this way needs no devlog entry of its own (there is no release branch). That
  ends the problem of the release branch's entry slipping into the next version.
