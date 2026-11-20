---
date: 2026-11-19
branch: fix/ci-once-per-release
type: chore
author: Marcos Ramos Jacome
agents: claude:orchestrator
pr: pending
version: v0.2.0
---

# CI once per release

## What changes

- CI runs once per release: on the promotion PR to `main` (or by hand from the Actions tab),
  no longer on every push and PR to `develop` and `main`. The devlog check stays local in
  `make check`. The release workflow is unchanged.

## Why

The owner is the only user for now and wants the tests to run once before each release, not a
full CI (with a 5–12 minute Windows job) on every change.

## Files and areas

- `.github/workflows/ci.yml`, `CLAUDE.md`

## How to test

- A PR to `develop` shows no CI checks; the promotion PR to `main` runs Rust, UI and Windows.
