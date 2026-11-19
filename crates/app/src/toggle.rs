//! Enable, disable and category switches (one journaled transaction each), previews and
//! drift reconciliation.

use std::collections::HashMap;

use gmm_domain::{Category, InstallationId, Mod, ModId, ModState, StepKind, TransactionKind};
use gmm_engine::{DriftKind, Plan, plan_disable, plan_enable, plan_set_states};
use serde::Serialize;
use ts_rs::TS;

use crate::app::App;
use crate::error::{AppError, ErrorCode};
use crate::guards::GuardNeeds;
use crate::installations::parse_id;
use crate::mods::{ModDto, installation_mod_dtos, mod_not_found, parse_mod_id, read_mod_dto};
use crate::operation::{NullSink, Operation, OperationKind, ProgressSink};

/// A mod whose state a plan changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AffectedModDto {
    /// The mod.
    pub mod_id: String,
    /// Its name.
    pub name: String,
    /// State before.
    pub from: ModState,
    /// State after.
    pub to: ModState,
}

/// One file action of a plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlannedActionDto {
    /// What happens (`ensure_dir`, `place_from_vault`, `tombstone_game_file`).
    pub kind: StepKind,
    /// Path relative to the installation root.
    pub rel_path: String,
    /// The mod the action belongs to.
    pub mod_name: String,
}

/// Dry run of a category switch (D-08).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlanPreviewDto {
    /// Mods whose state would change.
    pub mods_affected: Vec<AffectedModDto>,
    /// File actions, in execution order.
    pub actions: Vec<PlannedActionDto>,
    /// Bytes that would be copied into the game folder.
    #[ts(type = "number")]
    pub bytes_to_copy: u64,
}

/// Kind of divergence between the database and the disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DriftKindDto {
    /// A file of an enabled mod is missing from the game folder.
    Missing,
    /// A file exists but its content is not what the manager placed.
    Modified,
    /// The stored copy of a disabled mod's file is missing or damaged.
    VaultCorrupt,
}

impl From<DriftKind> for DriftKindDto {
    fn from(k: DriftKind) -> Self {
        match k {
            DriftKind::Missing => Self::Missing,
            DriftKind::Modified => Self::Modified,
            DriftKind::VaultCorrupt => Self::VaultCorrupt,
        }
    }
}

/// One file whose state on disk differs from the database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DriftDto {
    /// Owning mod.
    pub mod_id: String,
    /// Its name.
    pub mod_name: String,
    /// Path relative to the installation root.
    pub rel_path: String,
    /// What differs.
    pub kind: DriftKindDto,
}

/// True when the mod's state forbids a state change (`drifted` or `error`).
fn unusable_state(m: &Mod) -> Option<ModState> {
    matches!(m.state, ModState::Drifted | ModState::Error).then_some(m.state)
}

/// The error for a mod whose state forbids a bulk change; the batch is all-or-nothing.
pub(crate) fn refuse_state(m: &Mod) -> AppError {
    let drifted = m.state == ModState::Drifted;
    AppError::new(
        if drifted {
            ErrorCode::DriftDetected
        } else {
            ErrorCode::HashMismatch
        },
        if drifted {
            format!("Files of \"{}\" changed outside the manager.", m.name)
        } else {
            format!("The stored copy of \"{}\" is missing or damaged.", m.name)
        },
    )
    .with_hint("Resolve that mod first; nothing was changed.")
}

/// Refuses the first mod of `mods` that is drifted or damaged.
pub(crate) fn ensure_usable(mods: &[Mod]) -> Result<(), AppError> {
    match mods.iter().find(|m| unusable_state(m).is_some()) {
        Some(bad) => Err(refuse_state(bad)),
        None => Ok(()),
    }
}

/// Describes what a plan would do, with names taken from `mods`.
pub(crate) fn plan_preview(plan: &Plan, mods: &[Mod]) -> PlanPreviewDto {
    let names: HashMap<ModId, &str> = mods.iter().map(|m| (m.id, m.name.as_str())).collect();
    let by_id: HashMap<ModId, ModState> = mods.iter().map(|m| (m.id, m.state)).collect();
    let mods_affected = plan
        .summary
        .mod_states
        .iter()
        .map(|(mod_id, to)| AffectedModDto {
            mod_id: mod_id.to_string(),
            name: names.get(mod_id).copied().unwrap_or_default().to_owned(),
            from: by_id.get(mod_id).copied().unwrap_or(*to),
            to: *to,
        })
        .collect();
    let actions = plan
        .steps
        .iter()
        .map(|s| PlannedActionDto {
            kind: s.kind,
            rel_path: s.rel_path.as_str().to_owned(),
            mod_name: s
                .mod_id
                .and_then(|m| names.get(&m).copied())
                .unwrap_or_default()
                .to_owned(),
        })
        .collect();
    PlanPreviewDto {
        mods_affected,
        actions,
        bytes_to_copy: plan.bytes_to_copy,
    }
}

/// Plans enabling or disabling `ids` together (mods already in the state are skipped).
pub(crate) fn set_states_plan(
    ctx: &gmm_engine::EngineContext<'_>,
    ids: &[ModId],
    enabled: bool,
) -> gmm_engine::Result<Plan> {
    let desired: Vec<(ModId, bool)> = ids.iter().map(|id| (*id, enabled)).collect();
    let mut plan = plan_set_states(ctx, &desired)?;
    plan.kind = if enabled {
        TransactionKind::Enable
    } else {
        TransactionKind::Disable
    };
    Ok(plan)
}

impl App {
    /// Enables or disables one mod in a single journaled transaction and returns the mod
    /// as read back from the database. Already in the requested state: nothing happens.
    ///
    /// Dependency rules are enforced: a violation fails with `DEPENDENCY_BLOCKED`.
    pub fn set_mod_enabled(
        &self,
        mod_id: &str,
        enabled: bool,
        sink: &dyn ProgressSink,
    ) -> Result<ModDto, AppError> {
        self.set_mod_enabled_with(mod_id, enabled, false, sink)
    }

    /// Like [`App::set_mod_enabled`]. With `with_dependencies`, the dependency closure (the
    /// required mods when enabling, the dependents when disabling) is applied in the same
    /// all-or-nothing transaction.
    pub fn set_mod_enabled_with(
        &self,
        mod_id: &str,
        enabled: bool,
        with_dependencies: bool,
        sink: &dyn ProgressSink,
    ) -> Result<ModDto, AppError> {
        let id = parse_mod_id(mod_id)?;
        let installation_id = self
            .lock_db()?
            .get_mod(id)?
            .ok_or_else(mod_not_found)?
            .installation_id;
        self.run_operation(OperationKind::Transaction, sink, |op| {
            self.set_enabled_inner(installation_id, id, enabled, with_dependencies, op)
        })
    }

    pub(crate) fn set_enabled_inner(
        &self,
        installation_id: InstallationId,
        id: ModId,
        enabled: bool,
        with_dependencies: bool,
        op: &Operation<'_>,
    ) -> Result<ModDto, AppError> {
        let inst = self.load_validated(installation_id)?;
        // Cheap guards first, so a running game blocks before anything is reconciled.
        self.check_guards_id(
            installation_id,
            GuardNeeds {
                game_folder_writes: true,
                ..GuardNeeds::default()
            },
        )?;
        let ids = if with_dependencies {
            let db = self.lock_db()?;
            gmm_engine::dependency_closure(&db, installation_id, &[id], enabled)?
        } else {
            vec![id]
        };
        self.ensure_actionable(&inst, &ids)?;
        if with_dependencies {
            self.commit_plan(op, &inst, true, |ctx| set_states_plan(ctx, &ids, enabled))?;
        } else {
            let current = self
                .lock_db()?
                .get_mod(id)?
                .ok_or_else(mod_not_found)?
                .state;
            let wanted = if enabled {
                ModState::Enabled
            } else {
                ModState::Disabled
            };
            if current != wanted {
                self.commit_plan(op, &inst, true, |ctx| {
                    if enabled {
                        plan_enable(ctx, &[id])
                    } else {
                        plan_disable(ctx, &[id])
                    }
                })?;
            }
        }
        let db = self.lock_db()?;
        read_mod_dto(&db, id)
    }

    /// Mods of a category, refusing drifted or damaged ones (the switch is all-or-nothing).
    fn category_mods(
        &self,
        installation_id: InstallationId,
        category: Category,
    ) -> Result<Vec<Mod>, AppError> {
        let mods: Vec<Mod> = self
            .lock_db()?
            .list_mods(installation_id)?
            .into_iter()
            .filter(|m| m.category == category)
            .collect();
        ensure_usable(&mods)?;
        Ok(mods)
    }

    fn group_plan(
        ctx: &gmm_engine::EngineContext<'_>,
        mods: &[Mod],
        enabled: bool,
    ) -> gmm_engine::Result<Plan> {
        let desired: Vec<(ModId, bool)> = mods.iter().map(|m| (m.id, enabled)).collect();
        let mut plan = plan_set_states(ctx, &desired)?;
        plan.kind = TransactionKind::Group;
        Ok(plan)
    }

    /// Dry run of `set_category_enabled`: which mods change and which files move.
    pub fn preview_set_category(
        &self,
        installation_id: &str,
        category: Category,
        enabled: bool,
    ) -> Result<PlanPreviewDto, AppError> {
        let id = parse_id(installation_id)?;
        let inst = self.load_validated(id)?;
        let mods = self.category_mods(id, category)?;
        let plan = self.with_engine(&inst, |ctx| Self::group_plan(ctx, &mods, enabled))?;
        Ok(plan_preview(&plan, &mods))
    }

    /// Enables or disables every mod of a category in one all-or-nothing transaction and
    /// returns those mods as read back from the database.
    pub fn set_category_enabled(
        &self,
        installation_id: &str,
        category: Category,
        enabled: bool,
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
            let ids: Vec<ModId> = self
                .lock_db()?
                .list_mods(id)?
                .into_iter()
                .filter(|m| m.category == category)
                .map(|m| m.id)
                .collect();
            self.ensure_actionable(&inst, &ids)?;
            let mods = self.category_mods(id, category)?;
            if !mods.is_empty() {
                self.commit_plan(op, &inst, true, |ctx| Self::group_plan(ctx, &mods, enabled))?;
            }
            let wanted: Vec<String> = mods.iter().map(|m| m.id.to_string()).collect();
            let db = self.lock_db()?;
            Ok(installation_mod_dtos(&db, id)?
                .into_iter()
                .filter(|d| wanted.contains(&d.id))
                .collect())
        })
    }

    /// Compares the database with the disk, updates drift flags and returns every divergence.
    pub fn reconcile_installation(&self, installation_id: &str) -> Result<Vec<DriftDto>, AppError> {
        let id = parse_id(installation_id)?;
        self.run_operation(OperationKind::Transaction, &NullSink, |_op| {
            let inst = self.load_validated(id)?;
            let items = self.reconcile_disk(&inst)?;
            let db = self.lock_db()?;
            let names: HashMap<ModId, String> = db
                .list_mods(id)?
                .into_iter()
                .map(|m| (m.id, m.name))
                .collect();
            Ok(items
                .into_iter()
                .map(|d| DriftDto {
                    mod_id: d.mod_id.to_string(),
                    mod_name: names.get(&d.mod_id).cloned().unwrap_or_default(),
                    rel_path: d.path.as_str().to_owned(),
                    kind: d.kind.into(),
                })
                .collect())
        })
    }
}
