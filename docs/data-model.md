# Data model — SQLite schema v1

Status: **v3 — migrations `0001_initial.sql` (schema v1), `0002_dependencies.sql` (v2) and `0003_provenance_restore_point.sql` (v3) in `crates/store/migrations/`.**
Any change after Phase 1 ships is a new numbered migration; never edit an applied one.

## Conventions

- Database file: `<app data dir>/gmm.sqlite3`. Schema version tracked with `PRAGMA user_version`.
- Connection pragmas (every connection): `journal_mode = WAL`, `synchronous = FULL`, `foreign_keys = ON`, `busy_timeout = 5000`.
- Ids: UUID v7 as `TEXT` (36 chars, lowercase, hyphenated).
- Timestamps: `TEXT` RFC 3339 UTC with milliseconds, e.g. `2026-11-18T21:06:00.000Z`.
- Enums: `TEXT` constrained with `CHECK (... IN (...))`, values in `snake_case`, matching the Rust `as_str()` of each enum in `gmm-domain`.
- Paths: `rel_path` keeps original case with `/` separators; `path_key` is `lower(rel_path)` computed in Rust (`RelPath::key()`), used for uniqueness and joins.
- Hashes: `sha256` as 64 lowercase hex chars.

## Schema

```sql
-- 0001_initial.sql

CREATE TABLE installations (
    id            TEXT PRIMARY KEY,
    edition       TEXT NOT NULL CHECK (edition IN ('legacy', 'enhanced')),
    platform      TEXT NOT NULL CHECK (platform IN ('steam', 'epic', 'rockstar', 'unknown')),
    display_name  TEXT NOT NULL,
    root_path     TEXT NOT NULL,
    root_key      TEXT NOT NULL UNIQUE,
    exe_version   TEXT,
    status        TEXT NOT NULL CHECK (status IN ('ok', 'missing', 'incomplete', 'not_writable')),
    baseline      TEXT NOT NULL DEFAULT 'none' CHECK (baseline IN ('none', 'untrusted', 'trusted')),
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE TABLE scans (
    id               TEXT PRIMARY KEY,
    installation_id  TEXT NOT NULL REFERENCES installations(id) ON DELETE CASCADE,
    started_at       TEXT NOT NULL,
    finished_at      TEXT,
    status           TEXT NOT NULL CHECK (status IN ('running', 'completed', 'failed', 'cancelled')),
    files_seen       INTEGER NOT NULL DEFAULT 0 CHECK (files_seen >= 0),
    error            TEXT
);

CREATE TABLE inventory_items (
    installation_id  TEXT NOT NULL REFERENCES installations(id) ON DELETE CASCADE,
    path_key         TEXT NOT NULL,
    rel_path         TEXT NOT NULL,
    size             INTEGER NOT NULL CHECK (size >= 0),
    mtime_ns         INTEGER NOT NULL,
    sha256           TEXT CHECK (sha256 IS NULL OR length(sha256) = 64),
    classification   TEXT NOT NULL CHECK (classification IN
                        ('original', 'original_modified', 'candidate', 'unknown',
                         'managed', 'drifted', 'engine_artifact')),
    suggested_category TEXT,
    evidence_json    TEXT NOT NULL DEFAULT '[]',
    first_seen_at    TEXT NOT NULL,
    last_seen_at     TEXT NOT NULL,
    last_scan_id     TEXT REFERENCES scans(id) ON DELETE SET NULL,
    PRIMARY KEY (installation_id, path_key)
) WITHOUT ROWID;

CREATE INDEX inventory_items_by_class ON inventory_items (installation_id, classification);

CREATE TABLE baseline_entries (
    installation_id  TEXT NOT NULL REFERENCES installations(id) ON DELETE CASCADE,
    path_key         TEXT NOT NULL,
    rel_path         TEXT NOT NULL,
    size             INTEGER NOT NULL CHECK (size >= 0),
    sha256           TEXT NOT NULL CHECK (length(sha256) = 64),
    captured_at      TEXT NOT NULL,
    PRIMARY KEY (installation_id, path_key)
) WITHOUT ROWID;

CREATE TABLE vault_objects (
    sha256       TEXT PRIMARY KEY CHECK (length(sha256) = 64),
    size         INTEGER NOT NULL CHECK (size >= 0),
    created_at   TEXT NOT NULL,
    verified_at  TEXT NOT NULL
) WITHOUT ROWID;

CREATE TABLE mods (
    id               TEXT PRIMARY KEY,
    installation_id  TEXT NOT NULL REFERENCES installations(id) ON DELETE CASCADE,
    name             TEXT NOT NULL CHECK (length(trim(name)) > 0),
    category         TEXT NOT NULL CHECK (category IN
                        ('graphics_textures', 'vehicles', 'scripts_gameplay', 'maps_buildings',
                         'characters_clothing', 'sounds_music', 'uncategorized')),
    state            TEXT NOT NULL CHECK (state IN ('enabled', 'disabled', 'drifted', 'error')),
    mechanism        TEXT NOT NULL DEFAULT 'loose_files' CHECK (mechanism IN ('loose_files')),
    notes            TEXT,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL,
    UNIQUE (installation_id, name)
);

CREATE TABLE mod_entries (
    id        TEXT PRIMARY KEY,
    mod_id    TEXT NOT NULL REFERENCES mods(id) ON DELETE CASCADE,
    rel_path  TEXT NOT NULL,
    path_key  TEXT NOT NULL,
    sha256    TEXT NOT NULL REFERENCES vault_objects(sha256),
    size      INTEGER NOT NULL CHECK (size >= 0),
    UNIQUE (mod_id, path_key)
);

CREATE INDEX mod_entries_by_path ON mod_entries (path_key);

CREATE TABLE ownership_confirmations (
    id               TEXT PRIMARY KEY,
    mod_id           TEXT NOT NULL REFERENCES mods(id) ON DELETE CASCADE,
    confirmed_at     TEXT NOT NULL,
    included_unknown INTEGER NOT NULL CHECK (included_unknown IN (0, 1)),
    manifest_json    TEXT NOT NULL
);

CREATE TABLE profiles (
    id               TEXT PRIMARY KEY,
    installation_id  TEXT NOT NULL REFERENCES installations(id) ON DELETE CASCADE,
    name             TEXT NOT NULL CHECK (length(trim(name)) > 0),
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL,
    UNIQUE (installation_id, name)
);

CREATE TABLE profile_entries (
    profile_id  TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    mod_id      TEXT NOT NULL REFERENCES mods(id) ON DELETE CASCADE,
    desired     TEXT NOT NULL CHECK (desired IN ('enabled', 'disabled')),
    PRIMARY KEY (profile_id, mod_id)
) WITHOUT ROWID;

CREATE TABLE transactions (
    id               TEXT PRIMARY KEY,
    installation_id  TEXT NOT NULL REFERENCES installations(id) ON DELETE RESTRICT,
    kind             TEXT NOT NULL CHECK (kind IN
                        ('adopt', 'enable', 'disable', 'group', 'profile', 'restore')),
    status           TEXT NOT NULL CHECK (status IN
                        ('open', 'committing', 'committed', 'rolling_back', 'rolled_back',
                         'needs_attention')),
    summary_json     TEXT NOT NULL DEFAULT '{}',
    error            TEXT,
    created_at       TEXT NOT NULL,
    finished_at      TEXT
);

CREATE INDEX transactions_by_status ON transactions (status);
CREATE INDEX transactions_by_installation ON transactions (installation_id, created_at);

CREATE TABLE transaction_steps (
    transaction_id   TEXT NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    seq              INTEGER NOT NULL CHECK (seq >= 0),
    kind             TEXT NOT NULL CHECK (kind IN
                        ('capture_to_vault', 'ensure_dir', 'place_from_vault', 'tombstone_game_file')),
    mod_id           TEXT REFERENCES mods(id) ON DELETE SET NULL,
    rel_path         TEXT NOT NULL,
    sha256           TEXT CHECK (sha256 IS NULL OR length(sha256) = 64),
    artifact_name    TEXT,
    created_by_step  INTEGER NOT NULL DEFAULT 0 CHECK (created_by_step IN (0, 1)),
    status           TEXT NOT NULL CHECK (status IN ('pending', 'started', 'done', 'undone')),
    error            TEXT,
    updated_at       TEXT NOT NULL,
    PRIMARY KEY (transaction_id, seq)
) WITHOUT ROWID;

CREATE TABLE settings (
    key    TEXT PRIMARY KEY,
    value  TEXT NOT NULL
) WITHOUT ROWID;

PRAGMA user_version = 1;
```

## Migration 0002 — mod dependencies (schema v2, Phase 2)

```sql
CREATE TABLE mod_dependencies (
    mod_id            TEXT NOT NULL REFERENCES mods(id) ON DELETE CASCADE,
    depends_on_mod_id TEXT NOT NULL REFERENCES mods(id) ON DELETE CASCADE,
    created_at        TEXT NOT NULL,
    PRIMARY KEY (mod_id, depends_on_mod_id),
    CHECK (mod_id <> depends_on_mod_id)
) WITHOUT ROWID;
CREATE INDEX mod_dependencies_reverse ON mod_dependencies (depends_on_mod_id);
```

Same-installation and no-cycle rules are enforced by the repository; the engine validates the
final states of every plan (an enabled mod never has a disabled dependency).

## Migration 0003 — provenance and restore point (schema v3, Phase 4)

```sql
ALTER TABLE mods ADD COLUMN source_kind TEXT NOT NULL DEFAULT 'manual'
    CHECK (source_kind IN ('recognized', 'grouped', 'manual', 'zip_import'));
ALTER TABLE mods ADD COLUMN source_detail TEXT;   -- signature id or archive file name
ALTER TABLE mods ADD COLUMN source_sha256 TEXT;   -- archive hash of a ZIP import
ALTER TABLE mods ADD COLUMN source_version TEXT;  -- signature/PE version when known

ALTER TABLE profiles ADD COLUMN kind TEXT NOT NULL DEFAULT 'user'
    CHECK (kind IN ('user', 'restore_point'));
CREATE UNIQUE INDEX profiles_one_restore_point ON profiles (installation_id)
    WHERE kind = 'restore_point';
```

Mods that existed before v3 are `manual`. The source travels in the adoption payload of the
journal (`AdoptPayload.source`, `manual` when absent), so recovery commits it too. The
`restore_point` profile is the snapshot vanilla mode saves before disabling every mod; the
repository replaces it atomically (`replace_restore_point`) and the app deletes it after a
successful restore.

## Notes

- `transaction_steps.artifact_name` holds the temp/tombstone file name (`<name>.gmm-tmp-<tx8>` / `<name>.gmm-tomb-<tx8>`) so recovery knows exactly what it may delete.
- `transaction_steps.created_by_step = 1` marks a vault object or directory created by this step (undo may remove it).
- `transactions.installation_id` is `RESTRICT`: an installation with history cannot be deleted silently; unregistering an installation is an explicit flow (Phase 2).
- Enabled-path exclusivity (one enabled mod per `path_key` per installation) is enforced by the engine inside the transaction, not by a constraint, because it depends on `mods.state`.
