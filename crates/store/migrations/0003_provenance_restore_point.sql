-- Schema v3: provenance of every mod (Phase 4 R5) and the vanilla restore point (Phase 4 R4).

-- Existing mods were adopted or imported before provenance existed: they become 'manual'.
ALTER TABLE mods ADD COLUMN source_kind TEXT NOT NULL DEFAULT 'manual'
    CHECK (source_kind IN ('recognized', 'grouped', 'manual', 'zip_import'));
ALTER TABLE mods ADD COLUMN source_detail TEXT;
ALTER TABLE mods ADD COLUMN source_sha256 TEXT;
ALTER TABLE mods ADD COLUMN source_version TEXT;

-- 'restore_point' is the snapshot taken before vanilla mode: at most one per installation.
ALTER TABLE profiles ADD COLUMN kind TEXT NOT NULL DEFAULT 'user'
    CHECK (kind IN ('user', 'restore_point'));
CREATE UNIQUE INDEX profiles_one_restore_point ON profiles (installation_id)
    WHERE kind = 'restore_point';
