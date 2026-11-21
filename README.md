# ModSwitch

Windows desktop app to detect, inventory and safely enable/disable **story-mode** mods for
**GTA V** (Legacy and Enhanced) and **Red Dead Redemption 2** — without ever modifying original
game files. Every change is journaled, verified with SHA-256 and reversible, even after a crash.

ModSwitch was called *GTA V Mod Manager* until Red Dead Redemption 2 support. Since v0.6.0 the
Windows install (Start menu, install folder, uninstall entry, installer file) is named
ModSwitch too; the first ModSwitch installer removes the old *GTA V Mod Manager* install. The
repository and the data folder (`%LOCALAPPDATA%\GtavModManager`) keep the old name on purpose:
the data folder holds the database and the vault with the mods.

> Mods managed here are for story mode. Disabling mods does not make an installation safe for
> GTA Online or Red Dead Online.

## Stack

Tauri 2 · React + TypeScript (Vite) · Rust (workspace of crates) · SQLite.

## Layout

| Path                      | What                                                   |
| ------------------------- | ------------------------------------------------------ |
| `crates/domain`           | Domain model (no I/O)                                  |
| `crates/platform`         | File system trait, fault injection, hashing, OS probes |
| `crates/store`            | SQLite schema, migrations, repositories                |
| `crates/scanner`          | Installation discovery, scanning, classification (M2+) |
| `crates/engine`           | Vault, journal, transactions, recovery (M4)            |
| `crates/app`              | Use cases, guards, DTOs                                |
| `crates/testkit`          | Fake installations for tests                           |
| `apps/desktop`            | React UI + Tauri shell (`src-tauri`)                   |
| `docs/`                   | Architecture, data model, security, testing, roadmap   |

## Download & release

Windows users download the installer from the
[latest release](https://github.com/geomark27/gtav-mod-manager/releases/latest/download/ModSwitch-windows-x64-setup.exe)
(`ModSwitch-windows-x64-setup.exe`, `SHA256SUMS.txt` next to it), and the app updates
itself from then on. Releases are built and signed by GitHub Actions; the installer has no
code-signing certificate, so SmartScreen shows “Windows protected your PC” the first time
(More info → Run anyway).

How a version is cut, tagged and published: [docs/release.md](docs/release.md).

## Development

- Core (WSL/Linux or Windows): `cargo test` · `cargo clippy --all-targets -- -D warnings`
- Windows target check from WSL: `cargo check --target x86_64-pc-windows-gnu` (once: `rustup target add x86_64-pc-windows-gnu`; the toolchain is pinned in `rust-toolchain.toml`)
- UI: `cd apps/desktop && npm install && npm test && npm run build`
- Desktop app (Windows): `cd apps/desktop && npm run tauri dev`

Start with [docs/architecture.md](docs/architecture.md) and [AGENTS.md](AGENTS.md).
