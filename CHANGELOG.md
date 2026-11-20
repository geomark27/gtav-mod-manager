# Changelog

Everything users of GTA V Mod Manager notice is recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the versions follow
[Semantic Versioning](https://semver.org/): `MAJOR.MINOR.PATCH`.

Every branch that changes something visible adds its line under **[Unreleased]**, in *Added*,
*Changed*, *Fixed* or *Removed*. The technical detail of each task lives in the devlog
(`docs/devlog/`). To publish, `make bump` turns [Unreleased] into the version's section and
`make release` uses that section as the GitHub release notes (see `docs/release.md`).

## [Unreleased]

## [0.2.0] - 2026-11-19

### Added

- Vanilla mode: one button disables every managed mod after saving their states as a restore point, and "Restore my mods" brings them back in one step. The app says how many files it does not manage are still in the game folder, and never claims that playing online is safe.
- Detected mods: after a scan, the Review page names the mods it recognizes (Script Hook V, Script Hook V .NET, Rampage Trainer, Menyoo, ReShade, OpenIV and others), groups the remaining loose files, shows what each mod requires, and adopts the selected ones in one step with their dependencies linked.
- Mod source: every mod shows where it came from — recognized, grouped, adopted by hand or imported from a ZIP (with the archive name and hash).

## [0.1.0] - 2026-11-18

### Added

- Installations: finds GTA V Legacy and Enhanced installed through Steam, Epic or Rockstar — or takes a folder by hand — validates the structure and shows the edition, the game version and whether the folder can be written.
- Scan and review: walks an installation and classifies every file as original, managed or candidate, with the evidence behind it, so no game file is ever mistaken for a mod.
- Safe engine: every enable, disable and adopt is journaled with SHA-256 in two phases, and files taken out of the game go to a vault; if the app or the machine stops mid-operation, the next start detects it and finishes or rolls it back.
- Mods: adopt, enable and disable with verified switches; the screen only shows the state the engine committed to the database, never an optimistic guess.
- Group switches: enable or disable a whole category of mods in one transaction, or nothing at all.
- Conflicts and guards: mods that share paths are reported, collisions and protected or original files are refused, and switches are blocked while the game is running.
- Drift: detects files changed outside the app and lets you keep them, re-adopt them or restore the original.
- Profiles: save a named set of mods, preview the change and apply it all-or-nothing; a built-in Vanilla profile disables everything.
- History: a record of every operation with its result and the files it touched.
- Dashboard: installations, mods and vault usage at a glance.
- ZIP import: add a mod from a ZIP into a safe staging area, map it to the installation root and import it disabled.
- Dependencies: a mod can require other mods; they are enabled together in a single transaction.
- Vault maintenance: garbage collection removes the vault copies no mod points to any more.
- Windows installer and automatic updates: a per-user NSIS installer in English and Spanish that needs no administrator rights, and signed updates downloaded from GitHub releases. The installer has no code-signing certificate, so SmartScreen shows a warning the first time.

[Unreleased]: https://github.com/geomark27/gtav-mod-manager/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/geomark27/gtav-mod-manager/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/geomark27/gtav-mod-manager/releases/tag/v0.1.0
