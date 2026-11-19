-- Schema v2: dependencies between mods of the same installation.
CREATE TABLE mod_dependencies (
    mod_id            TEXT NOT NULL REFERENCES mods(id) ON DELETE CASCADE,
    depends_on_mod_id TEXT NOT NULL REFERENCES mods(id) ON DELETE CASCADE,
    created_at        TEXT NOT NULL,
    PRIMARY KEY (mod_id, depends_on_mod_id),
    CHECK (mod_id <> depends_on_mod_id)
) WITHOUT ROWID;
CREATE INDEX mod_dependencies_reverse ON mod_dependencies (depends_on_mod_id);
