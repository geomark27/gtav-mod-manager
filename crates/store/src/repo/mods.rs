//! Mods and their file entries.
//!
//! Mods are only inserted through `commit_transaction` (adopt), never directly.

use chrono::{DateTime, Utc};
use gmm_domain::{
    Category, InstallMechanism, InstallationId, Mod, ModEntry, ModId, ModSource, ModSourceKind,
    ModState, RelPath, Sha256,
};
use rusqlite::{Row, params, params_from_iter};

use super::{is_conflict, parse_col, to_u64};
use crate::database::Database;
use crate::error::{Error, Result};
use crate::time::{from_db, to_db};

const MOD_COLUMNS: &str = "id, installation_id, name, category, state, mechanism, notes, \
     created_at, updated_at, source_kind, source_detail, source_sha256, source_version";
const ENTRY_COLUMNS: &str = "id, mod_id, rel_path, sha256, size";

/// Maximum number of bound keys per `IN (...)` query.
const KEY_CHUNK: usize = 500;

fn mod_from_row(r: &Row<'_>) -> Result<Mod> {
    let id: String = r.get(0)?;
    let inst: String = r.get(1)?;
    let category: String = r.get(3)?;
    let state: String = r.get(4)?;
    let mechanism: String = r.get(5)?;
    let created: String = r.get(7)?;
    let updated: String = r.get(8)?;
    let source_kind: String = r.get(9)?;
    let source_sha: Option<String> = r.get(11)?;
    let source = ModSource {
        kind: parse_col::<ModSourceKind>("mods", "source_kind", &source_kind)?,
        detail: r.get(10)?,
        sha256: source_sha
            .map(|s| {
                Sha256::parse(&s).map_err(|_| Error::Corrupt {
                    table: "mods",
                    column: "source_sha256",
                    value: s.clone(),
                })
            })
            .transpose()?,
        version: r.get(12)?,
    };
    Ok(Mod {
        id: parse_col("mods", "id", &id)?,
        installation_id: parse_col("mods", "installation_id", &inst)?,
        name: r.get(2)?,
        category: parse_col::<Category>("mods", "category", &category)?,
        state: parse_col::<ModState>("mods", "state", &state)?,
        mechanism: parse_col::<InstallMechanism>("mods", "mechanism", &mechanism)?,
        notes: r.get(6)?,
        source,
        created_at: from_db("mods", "created_at", &created)?,
        updated_at: from_db("mods", "updated_at", &updated)?,
    })
}

fn entry_from_row(r: &Row<'_>, offset: usize) -> Result<ModEntry> {
    let id: String = r.get(offset)?;
    let mod_id: String = r.get(offset + 1)?;
    let rel: String = r.get(offset + 2)?;
    let sha: String = r.get(offset + 3)?;
    let size: i64 = r.get(offset + 4)?;
    Ok(ModEntry {
        id: parse_col("mod_entries", "id", &id)?,
        mod_id: parse_col("mod_entries", "mod_id", &mod_id)?,
        rel_path: RelPath::parse(&rel).map_err(|_| Error::Corrupt {
            table: "mod_entries",
            column: "rel_path",
            value: rel.clone(),
        })?,
        sha256: Sha256::parse(&sha).map_err(|_| Error::Corrupt {
            table: "mod_entries",
            column: "sha256",
            value: sha.clone(),
        })?,
        size: to_u64("mod_entries", "size", size)?,
    })
}

/// Inserts a mod row (used by the adopt commit, inside its SQLite transaction).
pub(crate) fn insert_mod(conn: &rusqlite::Connection, m: &Mod) -> Result<()> {
    conn.execute(
        &format!(
            "INSERT INTO mods ({MOD_COLUMNS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)"
        ),
        params![
            m.id.to_string(),
            m.installation_id.to_string(),
            m.name,
            m.category.as_str(),
            m.state.as_str(),
            m.mechanism.as_str(),
            m.notes,
            to_db(m.created_at),
            to_db(m.updated_at),
            m.source.kind.as_str(),
            m.source.detail,
            m.source.sha256.as_ref().map(Sha256::as_str),
            m.source.version,
        ],
    )
    .map_err(|e| {
        if is_conflict(&e) {
            Error::Conflict { what: "mod name" }
        } else {
            e.into()
        }
    })?;
    Ok(())
}

/// Inserts a mod entry row (used by the adopt commit).
pub(crate) fn insert_entry(conn: &rusqlite::Connection, e: &ModEntry) -> Result<()> {
    let size = i64::try_from(e.size).map_err(|_| Error::Corrupt {
        table: "mod_entries",
        column: "size",
        value: e.size.to_string(),
    })?;
    conn.execute(
        &format!("INSERT INTO mod_entries ({ENTRY_COLUMNS}, path_key) VALUES (?1,?2,?3,?4,?5,?6)"),
        params![
            e.id.to_string(),
            e.mod_id.to_string(),
            e.rel_path.as_str(),
            e.sha256.as_str(),
            size,
            e.rel_path.key(),
        ],
    )
    .map_err(|err| {
        if is_conflict(&err) {
            Error::Conflict {
                what: "mod entry path",
            }
        } else {
            err.into()
        }
    })?;
    Ok(())
}

impl Database {
    /// Returns one mod by id.
    pub fn get_mod(&self, id: ModId) -> Result<Option<Mod>> {
        use rusqlite::OptionalExtension;
        self.conn()
            .query_row(
                &format!("SELECT {MOD_COLUMNS} FROM mods WHERE id = ?1"),
                [id.to_string()],
                |r| Ok(mod_from_row(r)),
            )
            .optional()?
            .transpose()
    }

    /// Lists the mods of an installation ordered by name (case-insensitive), then id.
    pub fn list_mods(&self, installation_id: InstallationId) -> Result<Vec<Mod>> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {MOD_COLUMNS} FROM mods WHERE installation_id = ?1 \
             ORDER BY lower(name), id"
        ))?;
        let rows = stmt.query_map([installation_id.to_string()], |r| Ok(mod_from_row(r)))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row??);
        }
        Ok(out)
    }

    /// Lists the entries of a mod ordered by path key.
    pub fn list_mod_entries(&self, mod_id: ModId) -> Result<Vec<ModEntry>> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {ENTRY_COLUMNS} FROM mod_entries WHERE mod_id = ?1 ORDER BY path_key"
        ))?;
        let rows = stmt.query_map([mod_id.to_string()], |r| Ok(entry_from_row(r, 0)))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row??);
        }
        Ok(out)
    }

    /// Lists every entry of an installation with the state of its mod, ordered by path key, mod id.
    pub fn list_entries_for_installation(
        &self,
        installation_id: InstallationId,
    ) -> Result<Vec<(ModId, ModState, ModEntry)>> {
        let mut stmt = self.conn().prepare(
            "SELECT m.state, e.id, e.mod_id, e.rel_path, e.sha256, e.size \
             FROM mod_entries e JOIN mods m ON m.id = e.mod_id \
             WHERE m.installation_id = ?1 ORDER BY e.path_key, e.mod_id",
        )?;
        let rows = stmt.query_map([installation_id.to_string()], |r| {
            let state: String = r.get(0)?;
            Ok(entry_from_row(r, 1).and_then(|e| {
                let st = parse_col::<ModState>("mods", "state", &state)?;
                Ok((e.mod_id, st, e))
            }))
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row??);
        }
        Ok(out)
    }

    /// For the given path keys, returns `(path_key, mod_id)` of every `enabled` mod of the
    /// installation that owns an entry with that key. Ordered by key, then mod id.
    pub fn enabled_owners(
        &self,
        installation_id: InstallationId,
        path_keys: &[String],
    ) -> Result<Vec<(String, ModId)>> {
        let mut out = Vec::new();
        for chunk in path_keys.chunks(KEY_CHUNK) {
            let marks = vec!["?"; chunk.len()].join(",");
            let mut stmt = self.conn().prepare(&format!(
                "SELECT e.path_key, e.mod_id FROM mod_entries e \
                 JOIN mods m ON m.id = e.mod_id \
                 WHERE m.installation_id = ? AND m.state = 'enabled' \
                 AND e.path_key IN ({marks})"
            ))?;
            let args = std::iter::once(installation_id.to_string())
                .chain(chunk.iter().cloned())
                .collect::<Vec<_>>();
            let rows = stmt.query_map(params_from_iter(args), |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?;
            for row in rows {
                let (key, mod_id) = row?;
                out.push((key, parse_col("mod_entries", "mod_id", &mod_id)?));
            }
        }
        out.sort();
        out.dedup();
        Ok(out)
    }

    /// Sets the state of a mod; `NotFound` when it does not exist.
    pub fn update_mod_state(
        &self,
        id: ModId,
        state: ModState,
        updated_at: DateTime<Utc>,
    ) -> Result<()> {
        let n = self.conn().execute(
            "UPDATE mods SET state = ?2, updated_at = ?3 WHERE id = ?1",
            params![id.to_string(), state.as_str(), to_db(updated_at)],
        )?;
        found(n)
    }

    /// Updates name, category and notes of a mod.
    ///
    /// The caller validates the name (`gmm_domain::validate_mod_name`). `NotFound` when the
    /// mod does not exist, `Conflict` when another mod of the installation has that name.
    pub fn update_mod_meta(
        &self,
        id: ModId,
        name: &str,
        category: Category,
        notes: Option<&str>,
        updated_at: DateTime<Utc>,
    ) -> Result<()> {
        let n = self
            .conn()
            .execute(
                "UPDATE mods SET name = ?2, category = ?3, notes = ?4, updated_at = ?5 WHERE id = ?1",
                params![
                    id.to_string(),
                    name,
                    category.as_str(),
                    notes,
                    to_db(updated_at)
                ],
            )
            .map_err(|e| {
                if is_conflict(&e) {
                    Error::Conflict { what: "mod name" }
                } else {
                    Error::from(e)
                }
            })?;
        found(n)
    }

    /// Deletes a mod; its entries, ownership confirmations and profile entries cascade.
    /// Vault objects are left alone. `NotFound` when it does not exist.
    pub fn delete_mod(&self, id: ModId) -> Result<()> {
        let n = self
            .conn()
            .execute("DELETE FROM mods WHERE id = ?1", [id.to_string()])?;
        found(n)
    }
}

fn found(changed: usize) -> Result<()> {
    if changed == 0 {
        Err(Error::NotFound { what: "mod" })
    } else {
        Ok(())
    }
}
