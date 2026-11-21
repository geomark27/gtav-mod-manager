//! Deleting a mod: its files leave the game folder, the app forgets it, and its stored copy is
//! freed when nothing else uses it.
//!
//! Unlike "stop managing" (`forget_mod`), which leaves the files in the game folder, delete is
//! for a mod the user does not want at all (for example one imported into the wrong game).
//! Original game files are never touched: an enabled mod is first disabled through the usual
//! journaled transaction (I-01, I-02), then its rows are deleted.

use std::collections::BTreeSet;

use gmm_domain::{ModId, ModState, Sha256};
use gmm_engine::{Vault, vault_gc};
use gmm_platform::StdFs;
use serde::Serialize;
use ts_rs::TS;

use crate::app::App;
use crate::error::{AppError, ErrorCode};
use crate::guards::GuardNeeds;
use crate::mods::{mod_not_found, parse_mod_id};
use crate::operation::{Operation, OperationKind, ProgressSink};

/// Outcome of [`App::delete_mod`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DeleteModResultDto {
    /// Name of the deleted mod.
    pub name: String,
    /// Files removed from the game folder (0 when the mod was disabled).
    #[ts(type = "number")]
    pub files_removed: u64,
    /// Bytes of stored copies freed in the vault. Copies another mod still uses are kept.
    #[ts(type = "number")]
    pub vault_bytes_freed: u64,
}

impl App {
    /// Deletes a mod. An enabled mod is disabled first (its files leave the game folder in a
    /// journaled transaction); then the mod, its entries, dependency links and profile entries
    /// are deleted, and its vault objects that nothing else references are removed.
    ///
    /// Refused when another mod requires it (`DEPENDENCY_BLOCKED`, delete those first), when
    /// its files changed outside the app (drifted: restore, keep or stop managing first), when
    /// its stored copy is damaged (stop managing instead), and by the usual guards.
    pub fn delete_mod(
        &self,
        mod_id: &str,
        sink: &dyn ProgressSink,
    ) -> Result<DeleteModResultDto, AppError> {
        let id = parse_mod_id(mod_id)?;
        let installation_id = self
            .lock_db()?
            .get_mod(id)?
            .ok_or_else(mod_not_found)?
            .installation_id;
        self.run_operation(OperationKind::Transaction, sink, |op| {
            self.delete_mod_inner(id, op)
        })
        .inspect(|r| {
            tracing::info!(%installation_id, mod_id = %id, name = %r.name, "mod deleted");
        })
    }

    fn delete_mod_inner(
        &self,
        id: ModId,
        op: &Operation<'_>,
    ) -> Result<DeleteModResultDto, AppError> {
        let m = self.lock_db()?.get_mod(id)?.ok_or_else(mod_not_found)?;
        let inst = self.load_validated(m.installation_id)?;
        self.check_guards_id(
            inst.id,
            GuardNeeds {
                game_folder_writes: m.state == ModState::Enabled,
                ..GuardNeeds::default()
            },
        )?;
        let (dependents, entries) = {
            let db = self.lock_db()?;
            let mut names = Vec::new();
            for d in db.dependents_of(id)? {
                if let Some(dm) = db.get_mod(d)? {
                    names.push(dm.name);
                }
            }
            (names, db.list_mod_entries(id)?)
        };
        if !dependents.is_empty() {
            return Err(AppError::new(
                ErrorCode::DependencyBlocked,
                format!("Other mods require \"{}\".", m.name),
            )
            .with_hint("Delete the mods that require it first, or remove those requirements.")
            .with_details(dependents.join(", ")));
        }
        // Drifted or damaged mods are refused here, before anything changes.
        self.ensure_actionable(&inst, &[id])?;

        let files_removed = if m.state == ModState::Enabled {
            op.report(0, 0, Some("Removing the files from the game folder"));
            self.set_enabled_inner(inst.id, id, false, false, op)?;
            entries.len() as u64
        } else {
            0
        };

        op.report(0, 0, Some("Deleting the mod"));
        let shas: BTreeSet<Sha256> = entries.into_iter().map(|e| e.sha256).collect();
        let db = self.lock_db()?;
        db.delete_mod(id)?;
        // Free the stored copies nothing else uses. Skipped (left to Settings → Vault) while a
        // transaction anywhere is unresolved, as the vault cleanup does.
        let unresolved = !db.transactions_needing_recovery()?.is_empty();
        let mut vault_bytes_freed = 0;
        if !unresolved {
            let mut free = Vec::new();
            for sha in shas {
                if db.vault_object_unreferenced(&sha)? {
                    free.push(sha);
                }
            }
            vault_bytes_freed =
                vault_gc(&StdFs::new(), &db, &Vault::new(&self.dirs.vault_dir), &free)?;
        }
        Ok(DeleteModResultDto {
            name: m.name,
            files_removed,
            vault_bytes_freed,
        })
    }
}
