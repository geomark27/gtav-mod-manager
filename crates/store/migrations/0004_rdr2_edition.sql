-- Schema v4: Red Dead Redemption 2 support. The edition CHECK of installations accepts 'rdr2'.
-- The game is not stored: every edition belongs to exactly one game (gmm_domain::Edition::game).
--
-- SQLite cannot change a CHECK in place, so the table is rebuilt. The runner applies this
-- migration with foreign keys OFF (otherwise dropping the old table would cascade-delete every
-- scan, mod and profile) and runs PRAGMA foreign_key_check before committing.

CREATE TABLE installations_v4 (
    id            TEXT PRIMARY KEY,
    edition       TEXT NOT NULL CHECK (edition IN ('legacy', 'enhanced', 'rdr2')),
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

INSERT INTO installations_v4
    (id, edition, platform, display_name, root_path, root_key, exe_version, status, baseline,
     created_at, updated_at)
SELECT id, edition, platform, display_name, root_path, root_key, exe_version, status, baseline,
       created_at, updated_at
FROM installations;

DROP TABLE installations;
ALTER TABLE installations_v4 RENAME TO installations;
