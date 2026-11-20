//! Vanilla mode: disable every managed mod in one step and bring them back later.
//!
//! Going vanilla first stores a **restore point** (a profile of kind `restore_point` with the
//! current state of every mod, at most one per installation), then disables every mod in one
//! journaled transaction (kind `Profile`), with the usual guards and drift check. "Restore my
//! mods" applies that restore point in one transaction and deletes it once committed.
//!
//! Vanilla mode removes only the files of **managed** mods. Files the app does not manage stay
//! in the game folder; the status reports how many the last scan saw. It never claims that
//! playing online is safe.

use chrono::{DateTime, Utc};
use gmm_domain::{
    Classification, DesiredState, InstallationId, ModId, ModState, Profile, ProfileId, ProfileKind,
};
use gmm_store::Database;
use serde::Serialize;
use ts_rs::TS;

use crate::app::App;
use crate::error::{AppError, ErrorCode};
use crate::guards::GuardNeeds;
use crate::installations::{not_found, parse_id};
use crate::operation::{Operation, OperationKind, ProgressSink};
use crate::profiles::{ProfileDto, current_entries, profile_dto, profile_plan};

/// Prefix of the name of the restore point.
const RESTORE_POINT_PREFIX: &str = "Before vanilla";

/// Vanilla mode of one installation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VanillaStatusDto {
    /// True when a restore point exists and no managed mod has files in the game folder.
    pub active: bool,
    /// The snapshot "Restore my mods" applies, if any.
    pub restore_point: Option<ProfileDto>,
    /// Number of managed mods.
    #[ts(type = "number")]
    pub mods_total: u64,
    /// Managed mods whose files are in the game folder (enabled, drifted or damaged).
    #[ts(type = "number")]
    pub mods_in_game: u64,
    /// Files of the last scan that vanilla mode does not remove: candidates, unknown files and
    /// modified original files. `null` when the installation was never scanned.
    #[ts(type = "number | null")]
    pub unmanaged_files: Option<u64>,
}

fn restore_point_name(now: DateTime<Utc>) -> String {
    format!(
        "{RESTORE_POINT_PREFIX} — {}",
        now.format("%Y-%m-%d %H:%M:%S UTC")
    )
}

fn no_restore_point() -> AppError {
    AppError::new(
        ErrorCode::NotFound,
        "There is no restore point for this installation.",
    )
    .with_hint("Restore points are saved when you disable all mods with vanilla mode.")
}

/// Reads the vanilla status of an installation (the caller checked that it exists).
fn status(db: &Database, id: InstallationId) -> Result<VanillaStatusDto, AppError> {
    let mods = db.list_mods(id)?;
    let mods_in_game = mods
        .iter()
        .filter(|m| m.state != ModState::Disabled)
        .count() as u64;
    let restore_point = db
        .restore_point(id)?
        .map(|(p, entries)| profile_dto(&p, entries.len() as u64));
    let unmanaged_files = match db.last_scan(id)? {
        None => None,
        Some(_) => Some(
            db.inventory_counts(id)?
                .into_iter()
                .filter(|(c, _)| {
                    matches!(
                        c,
                        Classification::Candidate
                            | Classification::Unknown
                            | Classification::OriginalModified
                    )
                })
                .map(|(_, n)| n)
                .sum(),
        ),
    };
    Ok(VanillaStatusDto {
        active: restore_point.is_some() && mods_in_game == 0,
        restore_point,
        mods_total: mods.len() as u64,
        mods_in_game,
        unmanaged_files,
    })
}

/// Stores the current state of every mod as the restore point, replacing the previous one.
fn save_restore_point(db: &Database, id: InstallationId) -> Result<(), AppError> {
    let now = Utc::now();
    let mut profile = Profile {
        id: ProfileId::new(),
        installation_id: id,
        name: restore_point_name(now),
        kind: ProfileKind::RestorePoint,
        created_at: now,
        updated_at: now,
    };
    let entries = current_entries(&db.list_mods(id)?, profile.id);
    match db.replace_restore_point(&profile, &entries) {
        Err(gmm_store::Error::Conflict { .. }) => {
            // A user profile already has this exact name: keep both apart.
            profile.name = format!("{} ({})", profile.name, &profile.id.to_string()[..8]);
            db.replace_restore_point(&profile, &entries)?;
        }
        other => other?,
    }
    Ok(())
}

impl App {
    /// The vanilla status of an installation.
    pub fn vanilla_status(&self, installation_id: &str) -> Result<VanillaStatusDto, AppError> {
        let id = parse_id(installation_id)?;
        let db = self.lock_db()?;
        if db.get_installation(id)?.is_none() {
            return Err(not_found());
        }
        status(&db, id)
    }

    /// Disables every managed mod in one transaction, after saving their current states as
    /// the restore point.
    ///
    /// When no mod has files in the game folder nothing happens and an existing restore point
    /// is kept (a second press never overwrites the states worth restoring). Drifted or damaged
    /// mods block the operation before the restore point is touched.
    pub fn enter_vanilla(
        &self,
        installation_id: &str,
        sink: &dyn ProgressSink,
    ) -> Result<VanillaStatusDto, AppError> {
        let id = parse_id(installation_id)?;
        self.run_operation(OperationKind::Transaction, sink, |op| {
            self.enter_vanilla_inner(id, op)?;
            status(&*self.lock_db()?, id)
        })
    }

    /// [`App::enter_vanilla`] under an already running operation. Returns `true` when mods
    /// were disabled, `false` when none had files in the game folder.
    pub(crate) fn enter_vanilla_inner(
        &self,
        id: InstallationId,
        op: &Operation<'_>,
    ) -> Result<bool, AppError> {
        let inst = self.load_validated(id)?;
        self.check_guards_id(
            id,
            GuardNeeds {
                game_folder_writes: true,
                ..GuardNeeds::default()
            },
        )?;
        let mods = self.lock_db()?.list_mods(id)?;
        if mods.iter().all(|m| m.state == ModState::Disabled) {
            return Ok(false);
        }
        let ids: Vec<ModId> = mods.iter().map(|m| m.id).collect();
        self.ensure_actionable(&inst, &ids)?;
        // Saved before the files move: if the transaction fails, the restore point only
        // describes the state the mods are still in.
        save_restore_point(&*self.lock_db()?, id)?;
        let desired: Vec<(ModId, bool)> = ids.iter().map(|m| (*m, false)).collect();
        self.commit_plan(op, &inst, true, |ctx| profile_plan(ctx, &desired))?;
        tracing::info!(installation_id = %id, mods = ids.len(), "vanilla mode entered");
        Ok(true)
    }

    /// Applies the restore point in one transaction and deletes it once committed. Mods
    /// adopted after the restore point are left as they are.
    pub fn restore_from_vanilla(
        &self,
        installation_id: &str,
        sink: &dyn ProgressSink,
    ) -> Result<VanillaStatusDto, AppError> {
        let id = parse_id(installation_id)?;
        self.run_operation(OperationKind::Transaction, sink, |op| {
            let inst = self.load_validated(id)?;
            self.check_guards_id(
                id,
                GuardNeeds {
                    game_folder_writes: true,
                    ..GuardNeeds::default()
                },
            )?;
            let (profile, entries) = self
                .lock_db()?
                .restore_point(id)?
                .ok_or_else(no_restore_point)?;
            let desired: Vec<(ModId, bool)> = entries
                .iter()
                .map(|e| (e.mod_id, e.desired == DesiredState::Enabled))
                .collect();
            let ids: Vec<ModId> = desired.iter().map(|(m, _)| *m).collect();
            self.ensure_actionable(&inst, &ids)?;
            if !desired.is_empty() {
                self.commit_plan(op, &inst, true, |ctx| profile_plan(ctx, &desired))?;
            }
            let db = self.lock_db()?;
            db.delete_profile(profile.id)?;
            tracing::info!(installation_id = %id, mods = ids.len(), "mods restored from vanilla");
            status(&db, id)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_point_name_fits_profile_names() {
        let now = DateTime::parse_from_rfc3339("2026-11-19T14:03:09Z")
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_default();
        let name = restore_point_name(now);
        assert_eq!(name, "Before vanilla — 2026-11-19 14:03:09 UTC");
        // Profile names are 1-60 characters, with room for the " (xxxxxxxx)" fallback.
        assert!(name.chars().count() + 11 <= 60);
    }
}
