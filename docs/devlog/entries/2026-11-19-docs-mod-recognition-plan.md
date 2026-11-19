---
date: 2026-11-19
branch: docs/mod-recognition-plan
type: docs
author: Marcos Ramos Jacome
agents: claude:orchestrator
pr: pending
version: unreleased
---

# Plan: mod recognition (Phase 4)

## What changes

- `docs/roadmap.md` gains **Phase 4 — Mod recognition** (target v0.2.0), documentation only:
  a signature catalog of known mods (Rampage Trainer, Script Hook V, SHVDN, ASI Loader, Menyoo,
  …) that turns scanned loose files into named mods, grouping for unknown mods, a “Detected
  mods” screen with bulk adoption, one-click vanilla mode with restore, and provenance per mod
  (recognized, grouped, manual or imported from a ZIP). Steps R1–R6 with dependencies and sizes.
- New assumption A-08: real file names of the mods in the signature catalog.

## Why

First real use of v0.1.0 on GTA V Enhanced with mods already installed: the app listed loose
candidate files and the owner had to build each mod by hand. The owner wants the app to
recognize existing mods, show them by name, and switch them off to play online — saved as a
plan before writing code.

## Files and areas

- `docs/roadmap.md`

## How to test

- Read the Phase 4 section of `docs/roadmap.md`.
