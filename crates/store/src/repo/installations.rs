//! Installation repository.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use gmm_domain::{
    BaselineTrust, DistributionPlatform, Edition, Installation, InstallationId, InstallationStatus,
};
use rusqlite::{OptionalExtension, Row, params};

use crate::database::Database;
use crate::error::{Error, Result};
use crate::time::{from_db, to_db};

const COLUMNS: &str = "id, edition, platform, display_name, root_path, root_key, exe_version, \
                       status, baseline, created_at, updated_at";

const TABLE: &str = "installations";

fn parse<T: std::str::FromStr>(column: &'static str, value: &str) -> Result<T> {
    value.parse().map_err(|_| Error::Corrupt {
        table: TABLE,
        column,
        value: value.to_owned(),
    })
}

/// Reads the raw text columns, then decodes them (so decoding errors are store errors).
fn from_row(row: &Row<'_>) -> Result<Installation> {
    let id: String = row.get(0)?;
    let edition: String = row.get(1)?;
    let platform: String = row.get(2)?;
    let root_path: String = row.get(4)?;
    let status: String = row.get(7)?;
    let baseline: String = row.get(8)?;
    let created: String = row.get(9)?;
    let updated: String = row.get(10)?;
    Ok(Installation {
        id: parse::<InstallationId>("id", &id)?,
        edition: parse::<Edition>("edition", &edition)?,
        platform: parse::<DistributionPlatform>("platform", &platform)?,
        display_name: row.get(3)?,
        root_path: PathBuf::from(root_path),
        root_key: row.get(5)?,
        exe_version: row.get(6)?,
        status: parse::<InstallationStatus>("status", &status)?,
        baseline: parse::<BaselineTrust>("baseline", &baseline)?,
        created_at: from_db(TABLE, "created_at", &created)?,
        updated_at: from_db(TABLE, "updated_at", &updated)?,
    })
}

fn is_unique_violation(e: &rusqlite::Error) -> bool {
    matches!(
        e,
        rusqlite::Error::SqliteFailure(f, _)
            if f.code == rusqlite::ErrorCode::ConstraintViolation
                && f.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
    )
}

impl Database {
    /// Inserts an installation; `Conflict` when its root key is already registered.
    pub fn insert_installation(&self, i: &Installation) -> Result<()> {
        let result = self.conn().execute(
            &format!(
                "INSERT INTO installations ({COLUMNS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)"
            ),
            params![
                i.id.to_string(),
                i.edition.as_str(),
                i.platform.as_str(),
                i.display_name,
                i.root_path.to_string_lossy(),
                i.root_key,
                i.exe_version,
                i.status.as_str(),
                i.baseline.as_str(),
                to_db(i.created_at),
                to_db(i.updated_at),
            ],
        );
        match result {
            Ok(_) => Ok(()),
            Err(e) if is_unique_violation(&e) => Err(Error::Conflict {
                what: "installation root",
            }),
            Err(e) => Err(e.into()),
        }
    }

    /// Returns one installation by id.
    pub fn get_installation(&self, id: InstallationId) -> Result<Option<Installation>> {
        self.query_one_installation("id = ?1", &id.to_string())
    }

    /// Returns one installation by normalized root key.
    pub fn find_installation_by_root_key(&self, root_key: &str) -> Result<Option<Installation>> {
        self.query_one_installation("root_key = ?1", root_key)
    }

    fn query_one_installation(&self, filter: &str, arg: &str) -> Result<Option<Installation>> {
        self.conn()
            .query_row(
                &format!("SELECT {COLUMNS} FROM installations WHERE {filter}"),
                [arg],
                |r| Ok(from_row(r)),
            )
            .optional()?
            .transpose()
    }

    /// Lists all installations ordered by creation time, then id.
    pub fn list_installations(&self) -> Result<Vec<Installation>> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {COLUMNS} FROM installations ORDER BY created_at, id"
        ))?;
        let rows = stmt.query_map([], |r| Ok(from_row(r)))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row??);
        }
        Ok(out)
    }

    /// Updates the validation result of an installation; `NotFound` when it does not exist.
    pub fn update_installation_validation(
        &self,
        id: InstallationId,
        status: InstallationStatus,
        exe_version: Option<&str>,
        updated_at: DateTime<Utc>,
    ) -> Result<()> {
        let n = self.conn().execute(
            "UPDATE installations SET status = ?2, exe_version = ?3, updated_at = ?4 WHERE id = ?1",
            params![
                id.to_string(),
                status.as_str(),
                exe_version,
                to_db(updated_at)
            ],
        )?;
        found(n)
    }

    /// Renames an installation; `NotFound` when it does not exist.
    pub fn rename_installation(
        &self,
        id: InstallationId,
        display_name: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<()> {
        let n = self.conn().execute(
            "UPDATE installations SET display_name = ?2, updated_at = ?3 WHERE id = ?1",
            params![id.to_string(), display_name, to_db(updated_at)],
        )?;
        found(n)
    }
}

fn found(changed: usize) -> Result<()> {
    if changed == 0 {
        Err(Error::NotFound {
            what: "installation",
        })
    } else {
        Ok(())
    }
}
