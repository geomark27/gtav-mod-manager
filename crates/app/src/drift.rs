//! Drift resolution: restore a mod from its stored copy, or accept its current files.

use gmm_domain::{ModId, ModState};
use gmm_engine::{plan_readopt, plan_restore};

use crate::app::App;
use crate::error::{AppError, ErrorCode};
use crate::guards::GuardNeeds;
use crate::mods::{ModDto, mod_not_found, parse_mod_id, read_mod_dto};
use crate::operation::{OperationKind, ProgressSink};

fn nothing_to_resolve() -> AppError {
    AppError::new(
        ErrorCode::InvalidInput,
        "Nothing to resolve: the files already match.",
    )
}

impl App {
    /// Puts the stored version of every changed or missing file of a drifted mod back into
    /// the game folder. Modified files are first saved to the vault, so the user's version
    /// is not lost. Writes the game folder, so every guard applies.
    pub fn restore_mod(&self, mod_id: &str, sink: &dyn ProgressSink) -> Result<ModDto, AppError> {
        self.resolve_drift(mod_id, true, sink)
    }

    /// Accepts the current files of a drifted mod as the mod's content. The game folder is
    /// not touched, so only the running-game and recovery guards apply.
    pub fn readopt_mod(&self, mod_id: &str, sink: &dyn ProgressSink) -> Result<ModDto, AppError> {
        self.resolve_drift(mod_id, false, sink)
    }

    fn resolve_drift(
        &self,
        mod_id: &str,
        restore: bool,
        sink: &dyn ProgressSink,
    ) -> Result<ModDto, AppError> {
        let id = parse_mod_id(mod_id)?;
        let installation_id = self
            .lock_db()?
            .get_mod(id)?
            .ok_or_else(mod_not_found)?
            .installation_id;
        self.run_operation(OperationKind::Transaction, sink, |op| {
            let inst = self.load_validated(installation_id)?;
            self.check_guards_id(
                installation_id,
                GuardNeeds {
                    game_folder_writes: restore,
                    ..GuardNeeds::default()
                },
            )?;
            // The stored state may be stale: look at the disk first.
            self.reconcile_disk(&inst)?;
            let state = self
                .lock_db()?
                .get_mod(id)?
                .ok_or_else(mod_not_found)?
                .state;
            match state {
                ModState::Drifted => {}
                ModState::Error => {
                    return Err(AppError::new(
                        ErrorCode::HashMismatch,
                        "The stored copy of this mod is missing or damaged.",
                    )
                    .with_hint("Forget the mod, then adopt its files again."));
                }
                ModState::Enabled | ModState::Disabled => return Err(nothing_to_resolve()),
            }
            self.commit_plan(op, &inst, restore, |ctx| {
                let mod_id: ModId = id;
                if restore {
                    plan_restore(ctx, mod_id)
                } else {
                    plan_readopt(ctx, mod_id)
                }
            })?;
            let db = self.lock_db()?;
            read_mod_dto(&db, id)
        })
    }
}
