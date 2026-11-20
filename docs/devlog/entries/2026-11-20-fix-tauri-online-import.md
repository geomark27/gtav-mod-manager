---
date: 2026-11-20
branch: fix/tauri-online-import
type: fix
author: Marcos Ramos Jacome
agents: claude:orchestrator
pr: pending
version: v0.3.0
---

# Tauri shell: missing import and a check from WSL

## What changes

- `apps/desktop/src-tauri/src/commands.rs` imports `OnlineCheckDto`. Without it, the
  `check_online_readiness` and `prepare_for_online` commands of `feature/online-check` did not
  compile. CI failed on the promotion PR `develop` → `main` with four `E0425` errors.
- New `make shell`: clippy of the Tauri shell (`gmm-desktop`) for `x86_64-pc-windows-gnu`. It
  runs from WSL, where the Linux build of the shell is impossible (no webkit2gtk). It is part of
  `make check` and of the checks of `make release bump=…`.

## Why

The shell was only compiled in CI on Windows, so a broken command reached `develop` unnoticed.
Checking it for the Windows target from WSL catches these errors before a commit.

## Files and areas

- `apps/desktop/src-tauri/src/commands.rs`, `Makefile`, `scripts/release.sh`,
  `docs/release.md`

## How to test

- `make shell` passes. With the import removed, it reports the same four `E0425` errors as CI.
- `make check`.

## Notes and follow-ups

- `make shell` needs the Rust target once: `rustup target add x86_64-pc-windows-gnu`. It only
  type-checks; linking and the Windows tests still run in CI.
