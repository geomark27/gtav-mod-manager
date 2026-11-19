//! Vault object registry: which content-addressed blobs the vault holds.

use chrono::{DateTime, Utc};
use gmm_domain::Sha256;
use rusqlite::{OptionalExtension, params};

use super::{parse_col, to_u64};
use crate::database::Database;
use crate::error::{Error, Result};
use crate::time::{from_db, to_db};

/// `(size, created_at, verified_at)` of a vault object.
pub type VaultObjectInfo = (u64, DateTime<Utc>, DateTime<Utc>);

impl Database {
    /// Inserts a vault object, or refreshes `verified_at` when it already exists.
    ///
    /// `created_at` is only set on insert (to `verified_at`); the stored size is not changed.
    pub fn upsert_vault_object(
        &self,
        sha: &Sha256,
        size: u64,
        verified_at: DateTime<Utc>,
    ) -> Result<()> {
        upsert_vault_object(self.conn(), sha, size, verified_at)
    }

    /// Returns `(size, created_at, verified_at)` of a vault object.
    pub fn get_vault_object(&self, sha: &Sha256) -> Result<Option<VaultObjectInfo>> {
        let row = self
            .conn()
            .query_row(
                "SELECT size, created_at, verified_at FROM vault_objects WHERE sha256 = ?1",
                [sha.as_str()],
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?;
        row.map(|(size, created, verified)| {
            Ok((
                to_u64("vault_objects", "size", size)?,
                from_db("vault_objects", "created_at", &created)?,
                from_db("vault_objects", "verified_at", &verified)?,
            ))
        })
        .transpose()
    }

    /// True when any mod entry references the vault object.
    pub fn vault_object_referenced(&self, sha: &Sha256) -> Result<bool> {
        let n: i64 = self.conn().query_row(
            "SELECT EXISTS (SELECT 1 FROM mod_entries WHERE sha256 = ?1)",
            [sha.as_str()],
            |r| r.get(0),
        )?;
        Ok(n != 0)
    }

    /// Vault objects no mod references and no unresolved transaction needs: `(sha256, size)`,
    /// ordered by digest.
    ///
    /// A transaction is unresolved when it is `open`, `committing`, `rolling_back` or
    /// `needs_attention`. Besides its journaled steps, its persisted summary is searched for the
    /// digest: an import that has not yet created steps still names its objects there.
    pub fn unreferenced_vault_objects(&self) -> Result<Vec<(Sha256, u64)>> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT v.sha256, v.size FROM vault_objects v WHERE {UNREFERENCED} ORDER BY v.sha256"
        ))?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        let mut out = Vec::new();
        for row in rows {
            let (sha, size) = row?;
            out.push((
                parse_col("vault_objects", "sha256", &sha)?,
                to_u64("vault_objects", "size", size)?,
            ));
        }
        Ok(out)
    }

    /// True when the vault object exists in the registry and nothing references it (see
    /// [`Database::unreferenced_vault_objects`]).
    pub fn vault_object_unreferenced(&self, sha: &Sha256) -> Result<bool> {
        let n: i64 = self.conn().query_row(
            &format!(
                "SELECT EXISTS (SELECT 1 FROM vault_objects v WHERE v.sha256 = ?1 AND {UNREFERENCED})"
            ),
            [sha.as_str()],
            |r| r.get(0),
        )?;
        Ok(n != 0)
    }

    /// Deletes the registry row of an unreferenced vault object (the file is the caller's job).
    ///
    /// `InvalidState` when a mod entry or an unresolved transaction still references it,
    /// `NotFound` when there is no such row.
    pub fn delete_vault_object_row(&self, sha: &Sha256) -> Result<()> {
        let n = self.conn().execute(
            &format!("DELETE FROM vault_objects AS v WHERE v.sha256 = ?1 AND {UNREFERENCED}"),
            [sha.as_str()],
        )?;
        if n > 0 {
            return Ok(());
        }
        if self.get_vault_object(sha)?.is_none() {
            return Err(Error::NotFound {
                what: "vault object",
            });
        }
        Err(Error::InvalidState {
            what: format!("vault object {sha} is still referenced"),
        })
    }
}

const UNREFERENCED: &str = "\
    NOT EXISTS (SELECT 1 FROM mod_entries e WHERE e.sha256 = v.sha256) \
    AND NOT EXISTS (SELECT 1 FROM transaction_steps s \
        JOIN transactions t ON t.id = s.transaction_id \
        WHERE s.sha256 = v.sha256 \
        AND t.status IN ('open', 'committing', 'rolling_back', 'needs_attention')) \
    AND NOT EXISTS (SELECT 1 FROM transactions t \
        WHERE t.status IN ('open', 'committing', 'rolling_back', 'needs_attention') \
        AND instr(t.summary_json, v.sha256) > 0)";

/// Upsert usable inside a caller's SQLite transaction.
pub(crate) fn upsert_vault_object(
    conn: &rusqlite::Connection,
    sha: &Sha256,
    size: u64,
    verified_at: DateTime<Utc>,
) -> Result<()> {
    let size = i64::try_from(size).map_err(|_| crate::Error::Corrupt {
        table: "vault_objects",
        column: "size",
        value: size.to_string(),
    })?;
    let t = to_db(verified_at);
    conn.execute(
        "INSERT INTO vault_objects (sha256, size, created_at, verified_at) VALUES (?1, ?2, ?3, ?3) \
         ON CONFLICT(sha256) DO UPDATE SET verified_at = excluded.verified_at",
        params![sha.as_str(), size, t],
    )?;
    Ok(())
}
