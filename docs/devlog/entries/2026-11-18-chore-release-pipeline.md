---
date: 2026-11-18
branch: feature/release-pipeline
type: chore
author: Marcos Ramos Jacome
agents: claude:orchestrator, opencode:deepseek-v4.1-flash-free (t26 release tooling), opencode:deepseek-v4.1-flash-free (t27 in-app updates)
pr: "#1"
version: v0.1.0
---

# Release pipeline, CI and Windows installer

## What changes

- Every push and pull request to `main` and `develop` runs CI on GitHub Actions: Rust format,
  clippy and tests on Linux; UI typecheck, tests and build; the full Rust test suite and the
  Tauri shell clippy on `windows-latest` (the `#[cfg(windows)]` tests run for the first time);
  and, on pull requests to `develop`, this devlog check.
- Pushing a `vX.Y.Z` tag builds the Windows installer (NSIS, per-user, no admin rights), signs
  it for the updater, attaches `GTAV-Mod-Manager-windows-x64-setup.exe`, `SHA256SUMS.txt` and
  `latest.json` to a draft release and publishes it when the build succeeds. A manual
  `workflow_dispatch` builds a test installer as a workflow artifact without releasing.
- Controlled release flow through the `Makefile`: `make start` → `make check` → `make ship` →
  `make promote` (develop → main PR) → `make bump` on a `release/*` branch (versions,
  CHANGELOG and devlog closing) → `make release` (tag on `origin/main`). See `docs/release.md`.
- The installed app checks GitHub Releases for a newer signed version (10 s after start and
  every 6 hours; never in dev builds) and shows a banner; installing is blocked while a scan,
  transaction or import is running. Settings shows the installed version and a manual check.
- Uninstalling removes only the WebView cache; the database, the vault with the mod backups
  and the logs in `%LOCALAPPDATA%\GtavModManager` stay.
- Versions aligned at `0.1.0` in `Cargo.toml`, `tauri.conf.json` and `package.json`;
  `CHANGELOG.md` starts with everything the product does today under `[Unreleased]`.
- Devlog set up for the project (English, vendored skill); `opencode.json` is no longer tracked.
- Rust pinned to 1.97.1 (`rust-toolchain.toml`) so local, CI and release builds format and lint
  the same; `.gitattributes` forces LF so embedded catalogs and scripts are identical on Windows
  (the first Windows CI run failed on CRLF and on a test that creates names Windows forbids,
  now Unix-only).

## Why

The project had no way to build, version or ship the Windows `.exe`, and nothing verified it on
Windows. The owner wants the same controlled release process as signal-aduana: releases only
from `main`, from a tag, with notes from the CHANGELOG and signed automatic updates.

## Files and areas

- `.github/workflows/ci.yml`, `.github/workflows/release.yml`
- `Makefile`, `scripts/{bump,release,promote,merge,changelog-notes,updater-key,updater-manifest}.sh`
- `apps/desktop/src-tauri/tauri.conf.json`, `tauri.release.conf.json`, `windows/hooks.nsh`,
  `capabilities/default.json`, `src/{lib,commands}.rs`, `Cargo.toml`
- `crates/app/src/operation.rs` (`operation_status`)
- `apps/desktop/src/features/updates/**`, `layout/AppShell.tsx`, `features/settings/**`,
  `lib/ipc.ts`
- `CHANGELOG.md`, `docs/release.md`, `CLAUDE.md`, `README.md`, `AGENTS.md`, `.devlog.conf`,
  `.claude/skills/devlog/`

## How to test

- `make check` (format, clippy, tests, UI typecheck/tests/build, devlog).
- `make version`, `make notes`, `DRY_RUN=1 make bump ver=0.1.0`.
- Open the PR to `develop` and watch the CI jobs (`gh pr checks`).
- `gh workflow run release.yml -f ref=<branch>` and download the `gtav-mod-manager-installer`
  artifact to try the `.exe` on Windows without releasing.

## Notes and follow-ups

- Before the first release the owner runs `make updater-key` once on a `feature/*` or
  `release/*` branch (creates `~/.tauri/gtav-mod-manager.key`, stores it as the
  `TAURI_SIGNING_PRIVATE_KEY` repo secret, writes the public key) and backs the key up. Until
  then the release workflow stops at “prepare” on purpose.
- No Windows code-signing certificate: SmartScreen warns on first run (documented).
- `shellcheck` and a YAML linter were not available locally; CI is the first real run of the
  workflows.
