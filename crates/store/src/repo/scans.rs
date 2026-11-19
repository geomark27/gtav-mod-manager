//! Scan repository: lifecycle records of inventory scans.

use chrono::{DateTime, Utc};
use gmm_domain::{InstallationId, ScanId};
use rusqlite::{OptionalExtension, params};

use crate::database::Database;
use crate::error::{Error, Result};
use crate::time::{from_db, to_db};

const TABLE: &str = "scans";

/// Status of a scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScanStatus {
    /// The scan is in progress.
    Running,
    /// The scan finished successfully.
    Completed,
    /// The scan failed.
    Failed,
    /// The scan was cancelled.
    Cancelled,
}

impl ScanStatus {
    /// The persisted string (matches the SQL `CHECK` value).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

impl std::str::FromStr for ScanStatus {
    type Err = ();

    fn from_str(s: &str) -> std::result::Result<Self, ()> {
        match s {
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(()),
        }
    }
}

/// A row of the `scans` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanRecord {
    /// Scan id.
    pub id: ScanId,
    /// Scanned installation.
    pub installation_id: InstallationId,
    /// When the scan started.
    pub started_at: DateTime<Utc>,
    /// When the scan finished (`None` while running).
    pub finished_at: Option<DateTime<Utc>>,
    /// Current status.
    pub status: ScanStatus,
    /// Number of files seen.
    pub files_seen: u64,
    /// Error message, when failed.
    pub error: Option<String>,
}

fn corrupt(column: &'static str, value: &str) -> Error {
    Error::Corrupt {
        table: TABLE,
        column,
        value: value.to_owned(),
    }
}

impl Database {
    /// Registers a new running scan. Fails when the installation does not exist.
    pub fn start_scan(
        &self,
        installation_id: InstallationId,
        started_at: DateTime<Utc>,
    ) -> Result<ScanId> {
        let id = ScanId::new();
        self.conn().execute(
            "INSERT INTO scans (id, installation_id, started_at, status) \
             VALUES (?1, ?2, ?3, 'running')",
            params![
                id.to_string(),
                installation_id.to_string(),
                to_db(started_at)
            ],
        )?;
        Ok(id)
    }

    /// Records the outcome of a scan; `NotFound` when the scan does not exist.
    pub fn finish_scan(
        &self,
        id: ScanId,
        status: ScanStatus,
        files_seen: u64,
        error: Option<&str>,
        finished_at: DateTime<Utc>,
    ) -> Result<()> {
        let files = i64::try_from(files_seen).map_err(|_| corrupt("files_seen", "overflow"))?;
        let n = self.conn().execute(
            "UPDATE scans SET status = ?2, files_seen = ?3, error = ?4, finished_at = ?5 \
             WHERE id = ?1",
            params![
                id.to_string(),
                status.as_str(),
                files,
                error,
                to_db(finished_at)
            ],
        )?;
        if n == 0 {
            return Err(Error::NotFound { what: "scan" });
        }
        Ok(())
    }

    /// Returns the most recently started scan of an installation.
    pub fn last_scan(&self, installation_id: InstallationId) -> Result<Option<ScanRecord>> {
        let raw = self
            .conn()
            .query_row(
                "SELECT id, started_at, finished_at, status, files_seen, error FROM scans \
                 WHERE installation_id = ?1 ORDER BY started_at DESC, id DESC LIMIT 1",
                [installation_id.to_string()],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Option<String>>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, i64>(4)?,
                        r.get::<_, Option<String>>(5)?,
                    ))
                },
            )
            .optional()?;
        let Some((id, started, finished, status, files, error)) = raw else {
            return Ok(None);
        };
        Ok(Some(ScanRecord {
            id: id.parse().map_err(|_| corrupt("id", &id))?,
            installation_id,
            started_at: from_db(TABLE, "started_at", &started)?,
            finished_at: finished
                .as_deref()
                .map(|f| from_db(TABLE, "finished_at", f))
                .transpose()?,
            status: status.parse().map_err(|()| corrupt("status", &status))?,
            files_seen: u64::try_from(files)
                .map_err(|_| corrupt("files_seen", &files.to_string()))?,
            error,
        }))
    }
}
