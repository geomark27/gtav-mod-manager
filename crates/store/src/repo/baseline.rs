//! Baseline repository: reference hashes of the original installation.

use chrono::{DateTime, Utc};
use gmm_domain::{BaselineTrust, InstallationId, RelPath, Sha256};
use rusqlite::params;

use crate::database::Database;
use crate::error::{Error, Result};
use crate::time::{from_db, to_db};

fn corrupt(column: &'static str, value: &str) -> Error {
    Error::Corrupt {
        table: "baseline_entries",
        column,
        value: value.to_owned(),
    }
}

impl Database {
    /// Replaces the baseline of an installation and records its trust level.
    ///
    /// One transaction: delete old entries, insert the new ones, update
    /// `installations.baseline` and `updated_at`. `NotFound` when the installation is unknown.
    pub fn replace_baseline(
        &self,
        installation_id: InstallationId,
        entries: &[(RelPath, u64, Sha256)],
        trust: BaselineTrust,
        captured_at: DateTime<Utc>,
    ) -> Result<()> {
        let tx = self.conn().unchecked_transaction()?;
        let inst = installation_id.to_string();
        let at = to_db(captured_at);
        let n = tx.execute(
            "UPDATE installations SET baseline = ?2, updated_at = ?3 WHERE id = ?1",
            params![inst, trust.as_str(), at],
        )?;
        if n == 0 {
            return Err(Error::NotFound {
                what: "installation",
            });
        }
        tx.execute(
            "DELETE FROM baseline_entries WHERE installation_id = ?1",
            [&inst],
        )?;
        {
            let mut ins = tx.prepare_cached(
                "INSERT INTO baseline_entries \
                   (installation_id, path_key, rel_path, size, sha256, captured_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
                 ON CONFLICT (installation_id, path_key) DO UPDATE SET \
                   rel_path = excluded.rel_path, size = excluded.size, \
                   sha256 = excluded.sha256, captured_at = excluded.captured_at",
            )?;
            for (path, size, sha) in entries {
                let size = i64::try_from(*size).map_err(|_| corrupt("size", &size.to_string()))?;
                ins.execute(params![
                    inst,
                    path.key(),
                    path.as_str(),
                    size,
                    sha.as_str(),
                    at
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Returns `(path_key, size, sha256)` of every baseline entry, ordered by path key.
    pub fn load_baseline(
        &self,
        installation_id: InstallationId,
    ) -> Result<Vec<(String, u64, Sha256)>> {
        let mut stmt = self.conn().prepare(
            "SELECT path_key, size, sha256 FROM baseline_entries \
             WHERE installation_id = ?1 ORDER BY path_key",
        )?;
        let rows = stmt.query_map([installation_id.to_string()], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (key, size, sha) = row?;
            out.push((
                key,
                u64::try_from(size).map_err(|_| corrupt("size", &size.to_string()))?,
                Sha256::parse(&sha).map_err(|_| corrupt("sha256", &sha))?,
            ));
        }
        Ok(out)
    }

    /// Latest `captured_at` among the baseline entries, `None` when there is no baseline.
    pub fn baseline_captured_at(
        &self,
        installation_id: InstallationId,
    ) -> Result<Option<DateTime<Utc>>> {
        let at: Option<String> = self.conn().query_row(
            "SELECT MAX(captured_at) FROM baseline_entries WHERE installation_id = ?1",
            [installation_id.to_string()],
            |r| r.get(0),
        )?;
        at.map(|a| from_db("baseline_entries", "captured_at", &a))
            .transpose()
    }

    /// Deletes the baseline entries and sets the trust to `none`.
    pub fn clear_baseline(
        &self,
        installation_id: InstallationId,
        updated_at: DateTime<Utc>,
    ) -> Result<()> {
        self.replace_baseline(installation_id, &[], BaselineTrust::None, updated_at)
    }
}
