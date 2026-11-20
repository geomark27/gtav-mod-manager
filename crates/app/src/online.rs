//! Online readiness (Phase 5): what is left in the game folder before playing GTA Online, and
//! "prepare for online" (scan, vanilla mode, check) in one operation.
//!
//! The check reports what it found and what it did not find. It never states that playing
//! online is safe: Rockstar may look at things the app cannot see. Original game files are
//! only reported, never repaired (I-01); the fix for them is the launcher's "verify files".

use gmm_domain::{BaselineTrust, Classification, Evidence, InstallationId, ModState};
use gmm_scanner::Catalog;
use gmm_store::{Database, InventoryFilter, MAX_PAGE_SIZE};
use serde::Serialize;
use ts_rs::TS;

use crate::app::App;
use crate::error::AppError;
use crate::guards::GuardStatusDto;
use crate::installations::parse_id;
use crate::operation::{Operation, OperationKind, ProgressSink};
use crate::scan::rfc3339;

/// Most items listed per check; `count` always has the full number.
const MAX_ITEMS: u32 = 50;

/// One check of the online readiness list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ReadinessCheckId {
    /// GTA V, its launchers and Rockstar services are closed.
    GameClosed,
    /// No interrupted or unresolved transaction is waiting for recovery.
    Recovery,
    /// No managed mod has files in the game folder (enabled, drifted or damaged).
    ManagedMods,
    /// No unmanaged file matches a mod pattern (loaders, `.asi`, `scripts/`, `mods/`…), no
    /// managed file was changed outside the app, and no mod-like file is "original" only
    /// because a baseline lists it.
    ModFiles,
    /// Files the app does not recognize; they may be harmless.
    UnknownFiles,
    /// Original game files match the baseline.
    Originals,
    /// No temporary or tombstone files left by an interrupted operation.
    Leftovers,
}

/// Result of one check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CheckStatus {
    /// Nothing found.
    Pass,
    /// Something to look at, or a check that could not run.
    Warn,
    /// Something to fix before playing online.
    Fail,
}

/// Summary of the whole list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ReadinessVerdict {
    /// Every check passed: no mod files were found.
    Clear,
    /// No failure, but at least one warning to review.
    Review,
    /// At least one check failed.
    Blocked,
}

/// One check with what it found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReadinessCheckDto {
    /// Which check.
    pub id: ReadinessCheckId,
    /// Its result.
    pub status: CheckStatus,
    /// How many things it found.
    #[ts(type = "number")]
    pub count: u64,
    /// What it found (paths, mod or process names), at most 50.
    pub items: Vec<String>,
}

/// The online readiness of an installation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OnlineCheckDto {
    /// When the check ran, RFC 3339 UTC.
    pub checked_at: String,
    /// Start of the scan the file checks used, RFC 3339 UTC.
    pub scanned_at: Option<String>,
    /// Summary.
    pub verdict: ReadinessVerdict,
    /// Every check, in display order.
    pub checks: Vec<ReadinessCheckDto>,
    /// Trust of the baseline the original-file check compared against.
    pub baseline: BaselineTrust,
    /// True when "prepare for online" disabled mods in this run.
    pub vanilla_applied: bool,
    /// Why "prepare for online" could not disable the mods; the checks still ran.
    pub action_error: Option<AppError>,
}

fn check(
    id: ReadinessCheckId,
    failing: CheckStatus,
    items: Vec<String>,
    count: u64,
) -> ReadinessCheckDto {
    ReadinessCheckDto {
        id,
        status: if count == 0 {
            CheckStatus::Pass
        } else {
            failing
        },
        count,
        items,
    }
}

/// Paths of one classification in the last scan: `(count, first MAX_ITEMS paths)`.
fn paths_of(
    db: &Database,
    id: InstallationId,
    classes: &[Classification],
) -> Result<(u64, Vec<String>), AppError> {
    let page = db.list_inventory(
        id,
        &InventoryFilter {
            classifications: classes.to_vec(),
            search: None,
            limit: MAX_ITEMS,
            offset: 0,
        },
    )?;
    Ok((
        page.total,
        page.items
            .iter()
            .map(|i| i.rel_path.as_str().to_owned())
            .collect(),
    ))
}

/// Files that count as original only because a baseline lists them, although their path
/// matches a mod pattern of the catalog (a baseline captured with mods installed). Files of
/// the catalog originals and files signed by a trusted publisher are left out.
fn mod_like_originals(
    db: &Database,
    id: InstallationId,
    catalog: &Catalog,
) -> Result<Vec<String>, AppError> {
    let mut out = Vec::new();
    let mut offset = 0;
    loop {
        let page = db.list_inventory(
            id,
            &InventoryFilter {
                classifications: vec![Classification::Original, Classification::OriginalModified],
                search: None,
                limit: MAX_PAGE_SIZE,
                offset,
            },
        )?;
        let n = page.items.len() as u32;
        out.extend(
            page.items
                .iter()
                .filter(|i| {
                    !catalog.is_original(&i.rel_path)
                        && catalog.mod_pattern(&i.rel_path).is_some()
                        && !i
                            .evidence
                            .iter()
                            .any(|e| matches!(e, Evidence::SignedBy { .. }))
                })
                .map(|i| i.rel_path.as_str().to_owned()),
        );
        offset += n;
        if n == 0 || u64::from(offset) >= page.total {
            return Ok(out);
        }
    }
}

/// Builds the checks from the guard state and the database (no file-system access).
fn evaluate(
    db: &Database,
    id: InstallationId,
    catalog: &Catalog,
    guard: &GuardStatusDto,
    baseline: BaselineTrust,
) -> Result<Vec<ReadinessCheckDto>, AppError> {
    let mut checks = Vec::new();
    checks.push(check(
        ReadinessCheckId::GameClosed,
        CheckStatus::Fail,
        guard.game_running.clone(),
        guard.game_running.len() as u64,
    ));
    checks.push(check(
        ReadinessCheckId::Recovery,
        CheckStatus::Fail,
        Vec::new(),
        u64::from(guard.recovery_required),
    ));
    let in_game: Vec<String> = db
        .list_mods(id)?
        .into_iter()
        .filter(|m| m.state != ModState::Disabled)
        .map(|m| match m.state {
            ModState::Enabled => m.name,
            other => format!("{} ({other})", m.name),
        })
        .collect();
    let n = in_game.len() as u64;
    checks.push(check(
        ReadinessCheckId::ManagedMods,
        CheckStatus::Fail,
        in_game,
        n,
    ));
    let (n, mut items) = paths_of(
        db,
        id,
        &[Classification::Candidate, Classification::Drifted],
    )?;
    let in_baseline = mod_like_originals(db, id, catalog)?;
    let n = n + in_baseline.len() as u64;
    items.extend(
        in_baseline
            .into_iter()
            .map(|p| format!("{p} (in the baseline)")),
    );
    items.truncate(MAX_ITEMS as usize);
    checks.push(check(
        ReadinessCheckId::ModFiles,
        CheckStatus::Fail,
        items,
        n,
    ));
    let (n, items) = paths_of(db, id, &[Classification::Unknown])?;
    checks.push(check(
        ReadinessCheckId::UnknownFiles,
        CheckStatus::Warn,
        items,
        n,
    ));
    checks.push(if baseline == BaselineTrust::None {
        // Without a baseline nothing can say whether an original changed.
        ReadinessCheckDto {
            id: ReadinessCheckId::Originals,
            status: CheckStatus::Warn,
            count: 0,
            items: Vec::new(),
        }
    } else {
        let (n, items) = paths_of(db, id, &[Classification::OriginalModified])?;
        check(ReadinessCheckId::Originals, CheckStatus::Fail, items, n)
    });
    let (n, items) = paths_of(db, id, &[Classification::EngineArtifact])?;
    checks.push(check(
        ReadinessCheckId::Leftovers,
        CheckStatus::Warn,
        items,
        n,
    ));
    Ok(checks)
}

fn verdict(checks: &[ReadinessCheckDto]) -> ReadinessVerdict {
    if checks.iter().any(|c| c.status == CheckStatus::Fail) {
        ReadinessVerdict::Blocked
    } else if checks.iter().any(|c| c.status == CheckStatus::Warn) {
        ReadinessVerdict::Review
    } else {
        ReadinessVerdict::Clear
    }
}

impl App {
    /// Compares the disk with the database, scans the installation and runs every check.
    /// Read-only for the game folder. Cancellable; emits `scan` progress.
    pub fn check_online_readiness(
        &self,
        installation_id: &str,
        sink: &dyn ProgressSink,
    ) -> Result<OnlineCheckDto, AppError> {
        let id = parse_id(installation_id)?;
        self.run_operation(OperationKind::Scan, sink, |op| {
            self.fresh_scan(id, op)?;
            self.readiness(id, false, None)
        })
    }

    /// Scans, disables every managed mod (vanilla mode, with its restore point) and runs every
    /// check, in one operation. When the mods cannot be disabled (game running, drift…) the
    /// checks still run and `action_error` says why.
    pub fn prepare_for_online(
        &self,
        installation_id: &str,
        sink: &dyn ProgressSink,
    ) -> Result<OnlineCheckDto, AppError> {
        let id = parse_id(installation_id)?;
        self.run_operation(OperationKind::Transaction, sink, |op| {
            self.fresh_scan(id, op)?;
            let (applied, error) = match self.enter_vanilla_inner(id, op) {
                Ok(applied) => (applied, None),
                Err(e) => {
                    tracing::warn!(installation_id = %id, error = %e, "prepare for online: mods not disabled");
                    (false, Some(e))
                }
            };
            if applied {
                // The vanilla transaction rescanned; reconcile once more so states are current.
                let inst = self.load_validated(id)?;
                self.reconcile_disk(&inst)?;
            }
            self.readiness(id, applied, error)
        })
    }

    fn fresh_scan(&self, id: InstallationId, op: &Operation<'_>) -> Result<(), AppError> {
        let inst = self.load_validated(id)?;
        op.report(0, 0, Some("Comparing the folder with the managed mods"));
        self.reconcile_disk(&inst)?;
        self.scan_inner(id, op, true)?;
        Ok(())
    }

    fn readiness(
        &self,
        id: InstallationId,
        vanilla_applied: bool,
        action_error: Option<AppError>,
    ) -> Result<OnlineCheckDto, AppError> {
        let guard = self.guard_status(&id.to_string())?;
        let db = self.lock_db()?;
        let inst = db
            .get_installation(id)?
            .ok_or_else(crate::installations::not_found)?;
        let checks = evaluate(
            &db,
            id,
            self.catalogs.get(inst.edition),
            &guard,
            inst.baseline,
        )?;
        Ok(OnlineCheckDto {
            checked_at: rfc3339(chrono::Utc::now()),
            scanned_at: db.last_scan(id)?.map(|s| rfc3339(s.started_at)),
            verdict: verdict(&checks),
            checks,
            baseline: inst.baseline,
            vanilla_applied,
            action_error,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(status: CheckStatus) -> ReadinessCheckDto {
        ReadinessCheckDto {
            id: ReadinessCheckId::ModFiles,
            status,
            count: 0,
            items: Vec::new(),
        }
    }

    #[test]
    fn verdict_takes_the_worst_status() {
        assert_eq!(verdict(&[c(CheckStatus::Pass)]), ReadinessVerdict::Clear);
        assert_eq!(
            verdict(&[c(CheckStatus::Pass), c(CheckStatus::Warn)]),
            ReadinessVerdict::Review
        );
        assert_eq!(
            verdict(&[c(CheckStatus::Warn), c(CheckStatus::Fail)]),
            ReadinessVerdict::Blocked
        );
        assert_eq!(verdict(&[]), ReadinessVerdict::Clear);
    }

    #[test]
    fn a_check_passes_only_when_nothing_was_found() {
        let found = check(
            ReadinessCheckId::Leftovers,
            CheckStatus::Warn,
            vec!["x".into()],
            1,
        );
        assert_eq!(found.status, CheckStatus::Warn);
        let none = check(
            ReadinessCheckId::Leftovers,
            CheckStatus::Warn,
            Vec::new(),
            0,
        );
        assert_eq!(none.status, CheckStatus::Pass);
    }
}
