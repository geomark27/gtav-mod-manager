//! Profiles and their entries (desired state per mod).

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use gmm_domain::{
    DesiredState, InstallationId, ModId, Profile, ProfileEntry, ProfileId, VANILLA_PROFILE_NAME,
};
use rusqlite::{Connection, OptionalExtension, Row, params};

use super::{is_conflict, parse_col};
use crate::database::Database;
use crate::error::{Error, Result};
use crate::time::{from_db, to_db};

/// Maximum length of a profile name, in characters (after trimming).
const MAX_NAME_CHARS: usize = 60;

const PROFILE_COLUMNS: &str = "id, installation_id, name, created_at, updated_at";

fn profile_from_row(r: &Row<'_>) -> Result<Profile> {
    let id: String = r.get(0)?;
    let inst: String = r.get(1)?;
    let created: String = r.get(3)?;
    let updated: String = r.get(4)?;
    Ok(Profile {
        id: parse_col("profiles", "id", &id)?,
        installation_id: parse_col("profiles", "installation_id", &inst)?,
        name: r.get(2)?,
        created_at: from_db("profiles", "created_at", &created)?,
        updated_at: from_db("profiles", "updated_at", &updated)?,
    })
}

fn invalid(what: &str) -> Error {
    Error::InvalidState {
        what: what.to_owned(),
    }
}

/// Validates a profile name and returns it trimmed.
fn clean_name(name: &str) -> Result<&str> {
    let t = name.trim();
    let n = t.chars().count();
    if n == 0 || n > MAX_NAME_CHARS {
        return Err(invalid("profile name must be 1-60 characters"));
    }
    if t.to_lowercase() == VANILLA_PROFILE_NAME.to_lowercase() {
        return Err(invalid("profile name is reserved"));
    }
    Ok(t)
}

fn name_taken(
    conn: &Connection,
    inst: InstallationId,
    name: &str,
    except: Option<ProfileId>,
) -> Result<bool> {
    let except = except.map(|i| i.to_string()).unwrap_or_default();
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM profiles WHERE installation_id = ?1 AND lower(name) = lower(?2) \
         AND id <> ?3",
        params![inst.to_string(), name, except],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

fn conflict_map(e: rusqlite::Error) -> Error {
    if is_conflict(&e) {
        Error::Conflict {
            what: "profile name",
        }
    } else {
        e.into()
    }
}

/// Checks that entries belong to `profile_id`, are unique per mod, and that every mod belongs
/// to the installation.
fn check_entries(
    conn: &Connection,
    profile_id: ProfileId,
    inst: InstallationId,
    entries: &[ProfileEntry],
) -> Result<()> {
    let mut seen: HashSet<ModId> = HashSet::new();
    for e in entries {
        if e.profile_id != profile_id {
            return Err(invalid("profile entry belongs to another profile"));
        }
        if !seen.insert(e.mod_id) {
            return Err(invalid("duplicate mod in profile entries"));
        }
        let ok: Option<i64> = conn
            .query_row(
                "SELECT 1 FROM mods WHERE id = ?1 AND installation_id = ?2",
                params![e.mod_id.to_string(), inst.to_string()],
                |r| r.get(0),
            )
            .optional()?;
        if ok.is_none() {
            return Err(invalid(
                "profile entry references a mod of another installation",
            ));
        }
    }
    Ok(())
}

fn insert_entries(conn: &Connection, entries: &[ProfileEntry]) -> Result<()> {
    let mut stmt = conn
        .prepare("INSERT INTO profile_entries (profile_id, mod_id, desired) VALUES (?1, ?2, ?3)")?;
    for e in entries {
        stmt.execute(params![
            e.profile_id.to_string(),
            e.mod_id.to_string(),
            e.desired.as_str()
        ])?;
    }
    Ok(())
}

fn profile_installation(conn: &Connection, id: ProfileId) -> Result<InstallationId> {
    let inst: Option<String> = conn
        .query_row(
            "SELECT installation_id FROM profiles WHERE id = ?1",
            [id.to_string()],
            |r| r.get(0),
        )
        .optional()?;
    let inst = inst.ok_or(Error::NotFound { what: "profile" })?;
    parse_col("profiles", "installation_id", &inst)
}

impl Database {
    /// Creates a profile with its entries in one transaction.
    ///
    /// The name (trimmed, stored trimmed) must be 1-60 characters and must not be the reserved
    /// Vanilla name in any case (`InvalidState`). A name already used in the installation
    /// (case-insensitive) is a `Conflict`. Every entry must reference a mod of the profile's
    /// installation (`InvalidState`); nothing is written on failure.
    pub fn create_profile(&self, p: &Profile, entries: &[ProfileEntry]) -> Result<()> {
        let name = clean_name(&p.name)?;
        let tx = self.conn().unchecked_transaction()?;
        if name_taken(&tx, p.installation_id, name, None)? {
            return Err(Error::Conflict {
                what: "profile name",
            });
        }
        check_entries(&tx, p.id, p.installation_id, entries)?;
        tx.execute(
            &format!("INSERT INTO profiles ({PROFILE_COLUMNS}) VALUES (?1,?2,?3,?4,?5)"),
            params![
                p.id.to_string(),
                p.installation_id.to_string(),
                name,
                to_db(p.created_at),
                to_db(p.updated_at),
            ],
        )
        .map_err(conflict_map)?;
        insert_entries(&tx, entries)?;
        tx.commit()?;
        Ok(())
    }

    /// Lists the profiles of an installation with their entry counts, by name
    /// (case-insensitive), then id.
    pub fn list_profiles(&self, installation_id: InstallationId) -> Result<Vec<(Profile, u32)>> {
        let mut stmt = self.conn().prepare(
            "SELECT p.id, p.installation_id, p.name, p.created_at, p.updated_at, \
             (SELECT COUNT(*) FROM profile_entries e WHERE e.profile_id = p.id) \
             FROM profiles p WHERE p.installation_id = ?1 ORDER BY lower(p.name), p.id",
        )?;
        let rows = stmt.query_map([installation_id.to_string()], |r| {
            let count: i64 = r.get(5)?;
            Ok(profile_from_row(r).and_then(|p| {
                let c = u32::try_from(count).map_err(|_| Error::Corrupt {
                    table: "profile_entries",
                    column: "count",
                    value: count.to_string(),
                })?;
                Ok((p, c))
            }))
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row??);
        }
        Ok(out)
    }

    /// Returns a profile with its entries (ordered by mod id), or `None` when it does not exist.
    pub fn get_profile(&self, id: ProfileId) -> Result<Option<(Profile, Vec<ProfileEntry>)>> {
        let profile = self
            .conn()
            .query_row(
                &format!("SELECT {PROFILE_COLUMNS} FROM profiles WHERE id = ?1"),
                [id.to_string()],
                |r| Ok(profile_from_row(r)),
            )
            .optional()?
            .transpose()?;
        let Some(profile) = profile else {
            return Ok(None);
        };
        let mut stmt = self.conn().prepare(
            "SELECT mod_id, desired FROM profile_entries WHERE profile_id = ?1 ORDER BY mod_id",
        )?;
        let rows = stmt.query_map([id.to_string()], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        let mut entries = Vec::new();
        for row in rows {
            let (mod_id, desired) = row?;
            entries.push(ProfileEntry {
                profile_id: id,
                mod_id: parse_col("profile_entries", "mod_id", &mod_id)?,
                desired: parse_col::<DesiredState>("profile_entries", "desired", &desired)?,
            });
        }
        Ok(Some((profile, entries)))
    }

    /// Replaces all entries of a profile in one transaction and sets `updated_at`.
    ///
    /// `NotFound` when the profile does not exist; `InvalidState` when an entry references a
    /// mod of another installation, another profile, or a mod twice. Nothing changes on failure.
    pub fn replace_profile_entries(
        &self,
        id: ProfileId,
        entries: &[ProfileEntry],
        updated_at: DateTime<Utc>,
    ) -> Result<()> {
        let tx = self.conn().unchecked_transaction()?;
        let inst = profile_installation(&tx, id)?;
        check_entries(&tx, id, inst, entries)?;
        tx.execute(
            "DELETE FROM profile_entries WHERE profile_id = ?1",
            [id.to_string()],
        )?;
        insert_entries(&tx, entries)?;
        tx.execute(
            "UPDATE profiles SET updated_at = ?2 WHERE id = ?1",
            params![id.to_string(), to_db(updated_at)],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Renames a profile (same name rules as `create_profile`) and sets `updated_at`.
    ///
    /// `NotFound` when missing; `Conflict` when another profile of the installation has the name.
    pub fn rename_profile(
        &self,
        id: ProfileId,
        name: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<()> {
        let name = clean_name(name)?;
        let tx = self.conn().unchecked_transaction()?;
        let inst = profile_installation(&tx, id)?;
        if name_taken(&tx, inst, name, Some(id))? {
            return Err(Error::Conflict {
                what: "profile name",
            });
        }
        tx.execute(
            "UPDATE profiles SET name = ?2, updated_at = ?3 WHERE id = ?1",
            params![id.to_string(), name, to_db(updated_at)],
        )
        .map_err(conflict_map)?;
        tx.commit()?;
        Ok(())
    }

    /// Deletes a profile; its entries cascade. `NotFound` when it does not exist.
    pub fn delete_profile(&self, id: ProfileId) -> Result<()> {
        let n = self
            .conn()
            .execute("DELETE FROM profiles WHERE id = ?1", [id.to_string()])?;
        if n == 0 {
            return Err(Error::NotFound { what: "profile" });
        }
        Ok(())
    }
}
