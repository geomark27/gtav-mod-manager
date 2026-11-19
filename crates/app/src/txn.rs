//! Plumbing between the application layer and the operations engine: engine context,
//! error mapping, the guarded plan-then-execute pipeline and the post-transaction rescan.

use gmm_domain::{Installation, InstallationId, ModState, TransactionId, TransactionKind};
use gmm_engine::{DriftItem, EngineContext, Plan, Vault, execute, reconcile};
use gmm_platform::StdFs;

use crate::app::App;
use crate::error::{AppError, ErrorCode};
use crate::guards::GuardNeeds;
use crate::operation::Operation;

impl App {
    /// Maps an engine error; an `InvalidState` caused by an unresolved transaction becomes
    /// `RECOVERY_REQUIRED`.
    fn engine_error(
        &self,
        db: &gmm_store::Database,
        id: InstallationId,
        e: gmm_engine::Error,
    ) -> AppError {
        if matches!(e, gmm_engine::Error::InvalidState { .. })
            && self.has_unresolved_transactions(db, id).unwrap_or(false)
        {
            return AppError::new(
                ErrorCode::RecoveryRequired,
                "An earlier operation on this installation must be resolved first.",
            )
            .with_hint("Restart the app so it can finish or undo the interrupted operation.")
            .with_details(e.to_string());
        }
        e.into()
    }

    /// Runs `f` with an engine context for the installation. The database mutex is held for
    /// the whole call, so no other use case sees a half-applied transaction (I-07).
    pub(crate) fn with_engine<T>(
        &self,
        inst: &Installation,
        f: impl FnOnce(&EngineContext<'_>) -> gmm_engine::Result<T>,
    ) -> Result<T, AppError> {
        let db = self.lock_db()?;
        let vault = Vault::new(&self.dirs.vault_dir);
        let catalog = self.catalogs.get(inst.edition);
        let is_protected = |rel: &gmm_domain::RelPath| catalog.is_original(rel);
        let fs = StdFs::new();
        let ctx = EngineContext {
            fs: &fs,
            db: &db,
            vault: &vault,
            root: &inst.root_path,
            installation_id: inst.id,
            is_protected: &is_protected,
        };
        f(&ctx).map_err(|e| self.engine_error(&db, inst.id, e))
    }

    /// Reconciles the installation with the disk (updates drift flags only).
    pub(crate) fn reconcile_disk(&self, inst: &Installation) -> Result<Vec<DriftItem>, AppError> {
        let db = self.lock_db()?;
        let vault = Vault::new(&self.dirs.vault_dir);
        reconcile(&StdFs::new(), &db, &vault, &inst.root_path, inst.id)
            .map_err(|e| self.engine_error(&db, inst.id, e))
    }

    /// Reconciles, then refuses when one of `mod_ids` is drifted or has a damaged vault copy.
    pub(crate) fn ensure_actionable(
        &self,
        inst: &Installation,
        mod_ids: &[gmm_domain::ModId],
    ) -> Result<(), AppError> {
        let drift = self.reconcile_disk(inst)?;
        let db = self.lock_db()?;
        for id in mod_ids {
            let Some(m) = db.get_mod(*id)? else {
                return Err(AppError::new(ErrorCode::NotFound, "The mod was not found."));
            };
            let details = drift
                .iter()
                .filter(|d| d.mod_id == *id)
                .map(|d| format!("{} ({:?})", d.path, d.kind))
                .collect::<Vec<_>>()
                .join(", ");
            match m.state {
                ModState::Drifted => {
                    return Err(AppError::new(
                        ErrorCode::DriftDetected,
                        format!("Files of \"{}\" changed outside the manager.", m.name),
                    )
                    .with_hint(
                        "Restore the mod from its stored copy, re-adopt its files or forget it.",
                    )
                    .with_details(details));
                }
                ModState::Error => {
                    return Err(AppError::new(
                        ErrorCode::HashMismatch,
                        format!("The stored copy of \"{}\" is missing or damaged.", m.name),
                    )
                    .with_details(details));
                }
                ModState::Enabled | ModState::Disabled => {}
            }
        }
        Ok(())
    }

    /// Plans, re-checks the guards and executes one transaction.
    ///
    /// Guards run before planning and again right before `execute` (with the byte needs of
    /// the plan). Returns `None` when the plan has no steps (nothing to do). The inventory
    /// is refreshed after a committed transaction.
    pub(crate) fn commit_plan(
        &self,
        op: &Operation<'_>,
        inst: &Installation,
        game_folder_writes: bool,
        build: impl FnOnce(&EngineContext<'_>) -> gmm_engine::Result<Plan>,
    ) -> Result<Option<TransactionId>, AppError> {
        op.report(0, 0, Some("Checking the installation"));
        self.check_guards_id(
            inst.id,
            GuardNeeds {
                game_folder_writes,
                ..GuardNeeds::default()
            },
        )?;
        op.report(0, 0, Some("Planning"));
        let plan = self.with_engine(inst, build)?;
        // A plan without file steps still commits when it records database changes
        // (re-adopt of entries, registration of an imported mod).
        let has_entry_changes = !plan.summary.entry_updates.is_empty()
            || !plan.summary.entry_removals.is_empty()
            || plan.summary.adopt.is_some();
        if plan.steps.is_empty() && !has_entry_changes {
            return Ok(None);
        }
        let bytes = plan.bytes_to_copy;
        let (bytes_game, bytes_vault) = if plan.kind == TransactionKind::Adopt {
            (0, bytes)
        } else {
            (bytes, 0)
        };
        self.check_guards_id(
            inst.id,
            GuardNeeds {
                game_folder_writes,
                bytes_game,
                bytes_vault,
            },
        )?;
        op.report(0, 0, Some("Applying changes"));
        let tx = self.with_engine(inst, |ctx| execute(ctx, plan))?;
        tracing::info!(%tx, installation_id = %inst.id, "transaction committed");
        self.rescan_best_effort(inst.id, op);
        Ok(Some(tx))
    }

    /// Re-scans the installation after a committed change. A failure is logged and never
    /// turns the committed result into an error; the user cannot cancel this scan.
    pub(crate) fn rescan_best_effort(&self, id: InstallationId, op: &Operation<'_>) {
        op.report(0, 0, Some("Refreshing the inventory"));
        if let Err(e) = self.scan_inner(id, op, false) {
            tracing::warn!(installation_id = %id, error = %e, "rescan after the change failed");
        }
    }
}
