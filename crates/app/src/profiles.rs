//! Profiles: named sets of desired mod states, plus the built-in Vanilla profile.
//!
//! Profiles only describe wanted states. Applying one is a single journaled transaction
//! (`plan_set_states`, kind `Profile`) with the same guards, drift check and rescan as every
//! other change to the game folder. Profile bookkeeping itself touches only the database.

use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use chrono::Utc;
use gmm_domain::{
    DesiredState, InstallationId, Mod, ModId, ModState, Profile, ProfileEntry, ProfileId,
    ProfileKind, SUGGESTED_PROFILE_NAMES, TransactionKind,
};
use gmm_engine::{EngineContext, Plan, plan_set_states};
use gmm_store::Database;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::app::App;
use crate::error::{AppError, ErrorCode};
use crate::guards::GuardNeeds;
use crate::installations::{not_found, parse_id};
use crate::mods::{ModDto, installation_mod_dtos, parse_mod_id};
use crate::operation::{OperationKind, ProgressSink};
use crate::scan::rfc3339;
use crate::toggle::{PlanPreviewDto, ensure_usable, plan_preview};

/// Id of the computed profile in which every managed mod is disabled.
pub const VANILLA_PROFILE_ID: &str = "vanilla";

/// A profile as listed by the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProfileDto {
    /// Identifier (UUID), or `"vanilla"` for the built-in profile.
    pub id: String,
    /// Name, unique within the installation.
    pub name: String,
    /// Number of mods the profile mentions.
    #[ts(type = "number")]
    pub entry_count: u64,
    /// True only for the computed Vanilla profile (it cannot be edited or deleted).
    pub built_in: bool,
    /// `restore_point` for the snapshot taken before vanilla mode, `user` otherwise.
    pub kind: ProfileKind,
    /// Creation time, RFC 3339 UTC (Vanilla: the time the app started).
    pub created_at: String,
    /// Last update time, RFC 3339 UTC (Vanilla: the time the app started).
    pub updated_at: String,
}

/// The desired state of one mod inside a profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProfileEntryDto {
    /// The mod.
    pub mod_id: String,
    /// Its current name.
    pub mod_name: String,
    /// What the profile wants.
    pub desired: DesiredState,
}

/// A profile with its entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProfileDetailDto {
    /// The profile summary.
    #[serde(flatten)]
    #[ts(flatten)]
    pub summary: ProfileDto,
    /// Entries ordered by mod name.
    pub entries: Vec<ProfileEntryDto>,
}

/// One entry sent by the UI when it replaces the entries of a profile.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProfileEntryInput {
    /// The mod.
    pub mod_id: String,
    /// What the profile wants.
    pub desired: DesiredState,
}

/// Names offered when the user creates a profile.
pub fn profile_name_suggestions() -> Vec<String> {
    SUGGESTED_PROFILE_NAMES
        .iter()
        .map(|n| (*n).to_owned())
        .collect()
}

pub(crate) fn profile_dto(p: &Profile, entry_count: u64) -> ProfileDto {
    ProfileDto {
        id: p.id.to_string(),
        name: p.name.clone(),
        entry_count,
        built_in: false,
        kind: p.kind,
        created_at: rfc3339(p.created_at),
        updated_at: rfc3339(p.updated_at),
    }
}

fn is_vanilla(id: &str) -> bool {
    id == VANILLA_PROFILE_ID
}

fn vanilla_is_fixed() -> AppError {
    AppError::new(
        ErrorCode::InvalidInput,
        "Vanilla is built in and cannot be edited or deleted.",
    )
}

fn parse_profile_id(id: &str) -> Result<ProfileId, AppError> {
    if is_vanilla(id) {
        return Err(vanilla_is_fixed());
    }
    Ok(ProfileId::from_str(id)?)
}

fn profile_not_found() -> AppError {
    AppError::new(ErrorCode::NotFound, "The profile was not found.")
}

fn profile_error(e: gmm_store::Error) -> AppError {
    match e {
        gmm_store::Error::Conflict { .. } => AppError::new(
            ErrorCode::InvalidInput,
            "A profile with this name already exists in this installation.",
        )
        .with_hint("Choose a different name."),
        other => other.into(),
    }
}

fn state_of(desired: DesiredState) -> bool {
    desired == DesiredState::Enabled
}

fn detail(db: &Database, id: ProfileId) -> Result<ProfileDetailDto, AppError> {
    let (profile, entries) = db.get_profile(id)?.ok_or_else(profile_not_found)?;
    let names: HashMap<ModId, String> = db
        .list_mods(profile.installation_id)?
        .into_iter()
        .map(|m| (m.id, m.name))
        .collect();
    let mut entries: Vec<ProfileEntryDto> = entries
        .iter()
        .map(|e| ProfileEntryDto {
            mod_id: e.mod_id.to_string(),
            mod_name: names.get(&e.mod_id).cloned().unwrap_or_default(),
            desired: e.desired,
        })
        .collect();
    entries.sort_by(|a, b| {
        a.mod_name
            .to_lowercase()
            .cmp(&b.mod_name.to_lowercase())
            .then_with(|| a.mod_id.cmp(&b.mod_id))
    });
    Ok(ProfileDetailDto {
        summary: profile_dto(&profile, entries.len() as u64),
        entries,
    })
}

/// The current enabled/disabled state of every mod as profile entries. Drifted and damaged
/// mods are recorded as `enabled` (their files are, or should be, in the game).
pub(crate) fn current_entries(mods: &[Mod], profile_id: ProfileId) -> Vec<ProfileEntry> {
    mods.iter()
        .map(|m| ProfileEntry {
            profile_id,
            mod_id: m.id,
            desired: if m.state == ModState::Disabled {
                DesiredState::Disabled
            } else {
                DesiredState::Enabled
            },
        })
        .collect()
}

pub(crate) fn profile_plan(
    ctx: &EngineContext<'_>,
    desired: &[(ModId, bool)],
) -> gmm_engine::Result<Plan> {
    let mut plan = plan_set_states(ctx, desired)?;
    plan.kind = TransactionKind::Profile;
    Ok(plan)
}

impl App {
    /// Lists the profiles of an installation: Vanilla first, then the stored ones by name.
    pub fn list_profiles(&self, installation_id: &str) -> Result<Vec<ProfileDto>, AppError> {
        let id = parse_id(installation_id)?;
        let db = self.lock_db()?;
        if db.get_installation(id)?.is_none() {
            return Err(not_found());
        }
        let started = rfc3339(self.started_at);
        let mut out = vec![ProfileDto {
            id: VANILLA_PROFILE_ID.to_owned(),
            name: gmm_domain::VANILLA_PROFILE_NAME.to_owned(),
            entry_count: db.list_mods(id)?.len() as u64,
            built_in: true,
            kind: ProfileKind::User,
            created_at: started.clone(),
            updated_at: started,
        }];
        out.extend(
            db.list_profiles(id)?
                .iter()
                .map(|(p, n)| profile_dto(p, u64::from(*n))),
        );
        Ok(out)
    }

    /// Saves the current enabled/disabled state of every mod as a new profile. Drifted and
    /// damaged mods are recorded as `enabled` (their files are, or should be, in the game).
    pub fn create_profile_from_current(
        &self,
        installation_id: &str,
        name: &str,
    ) -> Result<ProfileDto, AppError> {
        let installation_id = parse_id(installation_id)?;
        // One database lock: no transaction can change a state between the read and the write.
        let db = self.lock_db()?;
        if db.get_installation(installation_id)?.is_none() {
            return Err(not_found());
        }
        let now = Utc::now();
        let profile = Profile {
            id: ProfileId::new(),
            installation_id,
            name: name.to_owned(),
            kind: ProfileKind::User,
            created_at: now,
            updated_at: now,
        };
        let entries = current_entries(&db.list_mods(installation_id)?, profile.id);
        db.create_profile(&profile, &entries)
            .map_err(profile_error)?;
        // The store trims the name; read it back.
        Ok(detail(&db, profile.id)?.summary)
    }

    /// Returns a stored profile with its entries.
    pub fn get_profile(&self, profile_id: &str) -> Result<ProfileDetailDto, AppError> {
        let id = parse_profile_id(profile_id)?;
        detail(&*self.lock_db()?, id)
    }

    /// Replaces the entries of a stored profile.
    pub fn update_profile_entries(
        &self,
        profile_id: &str,
        entries: Vec<ProfileEntryInput>,
    ) -> Result<ProfileDetailDto, AppError> {
        let id = parse_profile_id(profile_id)?;
        let rows = entries
            .iter()
            .map(|e| {
                Ok(ProfileEntry {
                    profile_id: id,
                    mod_id: parse_mod_id(&e.mod_id)?,
                    desired: e.desired,
                })
            })
            .collect::<Result<Vec<_>, AppError>>()?;
        let db = self.lock_db()?;
        db.replace_profile_entries(id, &rows, Utc::now())
            .map_err(profile_error)?;
        detail(&db, id)
    }

    /// Renames a stored profile.
    pub fn rename_profile(&self, profile_id: &str, name: &str) -> Result<ProfileDto, AppError> {
        let id = parse_profile_id(profile_id)?;
        let db = self.lock_db()?;
        db.rename_profile(id, name, Utc::now())
            .map_err(profile_error)?;
        Ok(detail(&db, id)?.summary)
    }

    /// Deletes a stored profile. The mods and their states are not touched.
    pub fn delete_profile(&self, profile_id: &str) -> Result<(), AppError> {
        let id = parse_profile_id(profile_id)?;
        self.lock_db()?.delete_profile(id)?;
        Ok(())
    }

    /// The `(mod, wanted enabled)` pairs a profile prescribes. Mods the profile does not
    /// mention are left out, so they stay as they are; Vanilla prescribes every mod disabled.
    fn profile_desired(
        &self,
        installation_id: InstallationId,
        profile_id: &str,
    ) -> Result<Vec<(ModId, bool)>, AppError> {
        let db = self.lock_db()?;
        if is_vanilla(profile_id) {
            return Ok(db
                .list_mods(installation_id)?
                .iter()
                .map(|m| (m.id, false))
                .collect());
        }
        let id = ProfileId::from_str(profile_id)?;
        let (profile, entries) = db.get_profile(id)?.ok_or_else(profile_not_found)?;
        if profile.installation_id != installation_id {
            return Err(profile_not_found());
        }
        Ok(entries
            .iter()
            .map(|e| (e.mod_id, state_of(e.desired)))
            .collect())
    }

    /// Dry run of `apply_profile`: which mods change state and which files move.
    pub fn preview_apply_profile(
        &self,
        installation_id: &str,
        profile_id: &str,
    ) -> Result<PlanPreviewDto, AppError> {
        let id = parse_id(installation_id)?;
        let inst = self.load_validated(id)?;
        let desired = self.profile_desired(id, profile_id)?;
        let mods = self.selected_mods(id, &desired)?;
        ensure_usable(&mods)?;
        let plan = self.with_engine(&inst, |ctx| profile_plan(ctx, &desired))?;
        Ok(plan_preview(&plan, &mods))
    }

    fn selected_mods(
        &self,
        id: InstallationId,
        desired: &[(ModId, bool)],
    ) -> Result<Vec<Mod>, AppError> {
        let wanted: HashSet<ModId> = desired.iter().map(|(m, _)| *m).collect();
        Ok(self
            .lock_db()?
            .list_mods(id)?
            .into_iter()
            .filter(|m| wanted.contains(&m.id))
            .collect())
    }

    /// Applies a profile (or `"vanilla"`) in one all-or-nothing transaction and returns the
    /// mods it covers as read back from the database.
    pub fn apply_profile(
        &self,
        installation_id: &str,
        profile_id: &str,
        sink: &dyn ProgressSink,
    ) -> Result<Vec<ModDto>, AppError> {
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
            let desired = self.profile_desired(id, profile_id)?;
            let ids: Vec<ModId> = desired.iter().map(|(m, _)| *m).collect();
            self.ensure_actionable(&inst, &ids)?;
            if !desired.is_empty() {
                self.commit_plan(op, &inst, true, |ctx| profile_plan(ctx, &desired))?;
            }
            let wanted: HashSet<String> = ids.iter().map(ToString::to_string).collect();
            let db = self.lock_db()?;
            Ok(installation_mod_dtos(&db, id)?
                .into_iter()
                .filter(|d| wanted.contains(&d.id))
                .collect())
        })
    }
}
