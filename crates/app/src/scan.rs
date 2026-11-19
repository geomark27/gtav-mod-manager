//! The scan use case: classify every file of an installation and store the inventory.
//!
//! The database mutex is held only around database calls, never while the folder is walked.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, SecondsFormat, Utc};
use gmm_domain::{
    Classification, Installation, InstallationId, InstallationStatus, ModId, ModState,
};
use gmm_platform::StdFs;
use gmm_scanner::{BaselineRef, CachedHash, ManagedRef, ScanContext, ScanOutcome, scan};
use gmm_store::{ScanRecord, ScanStatus};
use serde::Serialize;
use ts_rs::TS;

use crate::app::App;
use crate::error::{AppError, ErrorCode};
use crate::installations::{not_found, parse_id};
use crate::operation::{Operation, OperationKind, ProgressSink};

/// Status of a scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ScanStatusDto {
    /// In progress.
    Running,
    /// Finished successfully.
    Completed,
    /// Failed.
    Failed,
    /// Cancelled by the user.
    Cancelled,
}

impl From<ScanStatus> for ScanStatusDto {
    fn from(s: ScanStatus) -> Self {
        match s {
            ScanStatus::Running => Self::Running,
            ScanStatus::Completed => Self::Completed,
            ScanStatus::Failed => Self::Failed,
            ScanStatus::Cancelled => Self::Cancelled,
        }
    }
}

/// Number of inventory items with one classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClassCountDto {
    /// Classification.
    pub classification: Classification,
    /// Items with that classification.
    #[ts(type = "number")]
    pub count: u64,
}

/// Result of a completed scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScanSummaryDto {
    /// Scan id.
    pub scan_id: String,
    /// Files found.
    #[ts(type = "number")]
    pub files_seen: u64,
    /// Bytes hashed (cache hits cost nothing).
    #[ts(type = "number")]
    pub bytes_hashed: u64,
    /// Items per classification, sorted by classification name.
    pub counts: Vec<ClassCountDto>,
    /// Finish time, RFC 3339 UTC.
    pub finished_at: String,
}

/// A scan record as shown by the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScanDto {
    /// Scan id.
    pub id: String,
    /// Start time, RFC 3339 UTC.
    pub started_at: String,
    /// Finish time, `null` while running.
    pub finished_at: Option<String>,
    /// Status.
    pub status: ScanStatusDto,
    /// Files seen.
    #[ts(type = "number")]
    pub files_seen: u64,
    /// Error message, when failed.
    pub error: Option<String>,
}

pub(crate) fn rfc3339(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Secs, true)
}

impl From<&ScanRecord> for ScanDto {
    fn from(r: &ScanRecord) -> Self {
        Self {
            id: r.id.to_string(),
            started_at: rfc3339(r.started_at),
            finished_at: r.finished_at.map(rfc3339),
            status: r.status.into(),
            files_seen: r.files_seen,
            error: r.error.clone(),
        }
    }
}

pub(crate) fn counts_dto(counts: Vec<(Classification, u64)>) -> Vec<ClassCountDto> {
    counts
        .into_iter()
        .map(|(classification, count)| ClassCountDto {
            classification,
            count,
        })
        .collect()
}

impl App {
    /// Loads an installation and re-validates its root, persisting the new status.
    /// A vanished folder yields `NOT_FOUND`.
    pub(crate) fn load_validated(&self, id: InstallationId) -> Result<Installation, AppError> {
        let inst = self
            .lock_db()?
            .get_installation(id)?
            .ok_or_else(not_found)?;
        let report = self.validate_root(&inst.root_path);
        self.lock_db()?.update_installation_validation(
            id,
            report.status,
            report.exe_version.as_deref(),
            Utc::now(),
        )?;
        if report.status == InstallationStatus::Missing {
            return Err(AppError::new(
                ErrorCode::NotFound,
                "The installation folder was not found.",
            ));
        }
        Ok(inst)
    }

    fn scan_context(&self, inst: &Installation) -> Result<ScanContext, AppError> {
        let db = self.lock_db()?;
        // A disabled mod has no files in the game folder: a file at one of its paths is a
        // different file and must be classified on its own merits.
        let present: HashSet<ModId> = db
            .list_mods(inst.id)?
            .into_iter()
            .filter(|m| m.state != ModState::Disabled)
            .map(|m| m.id)
            .collect();
        let managed = db
            .managed_entries(inst.id)?
            .into_iter()
            .filter(|e| present.contains(&e.mod_id))
            .map(|e| {
                (
                    e.rel_path.key(),
                    ManagedRef {
                        mod_id: e.mod_id,
                        sha256: e.sha256,
                        size: e.size,
                    },
                )
            })
            .collect();
        let baseline = db
            .load_baseline(inst.id)?
            .into_iter()
            .map(|(key, size, sha256)| (key, BaselineRef { size, sha256 }))
            .collect();
        let hash_cache: HashMap<String, CachedHash> = db
            .hash_cache(inst.id)?
            .into_iter()
            .map(|(key, size, mtime_ns, sha256)| {
                (
                    key,
                    CachedHash {
                        size,
                        mtime_ns,
                        sha256,
                    },
                )
            })
            .collect();
        Ok(ScanContext {
            managed,
            baseline,
            baseline_trust: inst.baseline,
            hash_cache,
        })
    }

    /// Scans the installation, replaces its inventory and records the scan.
    ///
    /// Fails with `OPERATION_IN_PROGRESS` while another operation runs and with `CANCELLED`
    /// when [`App::cancel_operation`] is called (the previous inventory is kept).
    pub fn scan_installation(
        &self,
        id: &str,
        sink: &dyn ProgressSink,
    ) -> Result<ScanSummaryDto, AppError> {
        let id = parse_id(id)?;
        self.run_operation(OperationKind::Scan, sink, |op| {
            self.scan_inner(id, op, true)
        })
    }

    /// Scans and stores the inventory under an already running operation. When `cancellable`
    /// is false the user's cancel request is ignored (used for the post-transaction rescan).
    pub(crate) fn scan_inner(
        &self,
        id: InstallationId,
        op: &Operation<'_>,
        cancellable: bool,
    ) -> Result<ScanSummaryDto, AppError> {
        let inst = self.load_validated(id)?;
        let ctx = self.scan_context(&inst)?;
        let scan_id = self.lock_db()?.start_scan(id, Utc::now())?;
        let result = scan(
            &StdFs::new(),
            &*self.os,
            self.catalogs.get(inst.edition),
            &inst.root_path,
            &ctx,
            &mut |p| op.report(p.files_seen, p.bytes_hashed, p.current.as_deref()) || !cancellable,
        );
        match result {
            Ok(outcome) => self.store_scan(id, scan_id, outcome),
            Err(e) => {
                let (status, message) = match &e {
                    gmm_scanner::Error::Cancelled => (ScanStatus::Cancelled, None),
                    other => (ScanStatus::Failed, Some(other.to_string())),
                };
                self.finish_best_effort(scan_id, status, op.files_seen(), message.as_deref());
                Err(match e {
                    gmm_scanner::Error::Fs(_) => AppError::new(
                        ErrorCode::Io,
                        "The scan could not read the installation folder.",
                    )
                    .with_details(message.unwrap_or_default()),
                    other => other.into(),
                })
            }
        }
    }

    fn finish_best_effort(
        &self,
        scan_id: gmm_domain::ScanId,
        status: ScanStatus,
        files: u64,
        error: Option<&str>,
    ) {
        let outcome = self.lock_db().map_err(|e| e.message).and_then(|db| {
            db.finish_scan(scan_id, status, files, error, Utc::now())
                .map_err(|e| e.to_string())
        });
        if let Err(e) = outcome {
            tracing::warn!(error = %e, "could not record the scan outcome");
        }
    }

    fn store_scan(
        &self,
        id: InstallationId,
        scan_id: gmm_domain::ScanId,
        outcome: ScanOutcome,
    ) -> Result<ScanSummaryDto, AppError> {
        let files = outcome.stats.files;
        let bytes_hashed = outcome.stats.bytes_hashed;
        let outcome = outcome.with_owner(id, scan_id);
        let finished_at = Utc::now();
        let stored = {
            let db = self.lock_db()?;
            db.replace_inventory(id, scan_id, &outcome.items, finished_at)
                .and_then(|()| {
                    db.finish_scan(scan_id, ScanStatus::Completed, files, None, finished_at)
                })
                .and_then(|()| db.inventory_counts(id))
        };
        match stored {
            Ok(counts) => Ok(ScanSummaryDto {
                scan_id: scan_id.to_string(),
                files_seen: files,
                bytes_hashed,
                counts: counts_dto(counts),
                finished_at: rfc3339(finished_at),
            }),
            Err(e) => {
                self.finish_best_effort(scan_id, ScanStatus::Failed, files, Some(&e.to_string()));
                Err(e.into())
            }
        }
    }
}
