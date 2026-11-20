//! Journal helpers shared by execution and recovery: undo of a whole transaction, commit
//! effects built from the persisted summary, and status transitions.

use chrono::Utc;
use gmm_domain::{
    InstallMechanism, Mod, ModEntry, ModEntryId, ModState, StepStatus, TransactionId,
    TransactionStatus,
};
use gmm_store::{AdoptRecord, CommitEffects, StepRow, TransactionRow};

use crate::error::{Error, Result};
use crate::plan::TxSummary;
use crate::steps::{Env, Undo, undo_step};

/// Undoes every `started`/`done` step in reverse `seq` order, marking each `undone`.
///
/// Returns `None` when everything was undone, or the reason of the first undo that could not be
/// proven safe (the remaining steps are left untouched).
pub(crate) fn undo_all(
    env: &Env<'_>,
    tx: TransactionId,
    steps: &[StepRow],
) -> Result<Option<String>> {
    for step in steps.iter().rev() {
        if !matches!(step.status, StepStatus::Started | StepStatus::Done) {
            continue;
        }
        match undo_step(env, step)? {
            Undo::Undone => {
                env.db.mark_step(
                    tx,
                    step.seq,
                    StepStatus::Undone,
                    None,
                    None,
                    None,
                    Utc::now(),
                )?;
            }
            Undo::Unsafe(reason) => return Ok(Some(reason)),
        }
    }
    Ok(None)
}

/// Moves a transaction in `rolling_back` to its final state after [`undo_all`].
pub(crate) fn finish_rollback(
    env: &Env<'_>,
    tx: TransactionId,
    outcome: Option<String>,
    cause: Option<&str>,
) -> Result<()> {
    match outcome {
        None => {
            env.db
                .set_transaction_status(tx, TransactionStatus::RolledBack, cause, Utc::now())?
        }
        Some(reason) => env.db.set_transaction_status(
            tx,
            TransactionStatus::NeedsAttention,
            Some(&reason),
            Utc::now(),
        )?,
    }
    Ok(())
}

/// Builds the database effects of a transaction from its persisted summary.
pub(crate) fn effects_from(row: &TransactionRow) -> Result<CommitEffects> {
    let summary: TxSummary =
        serde_json::from_str(&row.summary_json).map_err(|e| Error::InvalidState {
            what: format!("transaction {} has an unreadable summary: {e}", row.id),
        })?;
    let now = Utc::now();
    let mut effects = CommitEffects {
        mod_states: summary.mod_states,
        vault_objects: summary.vault_objects,
        entry_updates: summary.entry_updates,
        entry_removals: summary.entry_removals,
        ..CommitEffects::default()
    };
    if let Some(a) = summary.adopt {
        if !matches!(a.initial_state, ModState::Enabled | ModState::Disabled) {
            return Err(Error::InvalidState {
                what: format!("an adoption cannot create a {} mod", a.initial_state),
            });
        }
        let manifest_json = serde_json::to_string(&a).map_err(|e| Error::InvalidState {
            what: format!("cannot serialize the adoption manifest: {e}"),
        })?;
        let mut vault_objects = Vec::new();
        for (_, sha, size) in &a.entries {
            if !vault_objects.iter().any(|(s, _)| s == sha) {
                vault_objects.push((sha.clone(), *size));
            }
        }
        for (sha, size) in vault_objects {
            if !effects.vault_objects.iter().any(|(s, _)| *s == sha) {
                effects.vault_objects.push((sha, size));
            }
        }
        effects.adopt = Some(AdoptRecord {
            r#mod: Mod {
                id: a.mod_id,
                installation_id: row.installation_id,
                name: a.name.clone(),
                category: a.category,
                state: a.initial_state,
                mechanism: InstallMechanism::LooseFiles,
                notes: a.notes.clone(),
                source: a.source.clone(),
                created_at: now,
                updated_at: now,
            },
            entries: a
                .entries
                .iter()
                .map(|(rel, sha, size)| ModEntry {
                    id: ModEntryId::new(),
                    mod_id: a.mod_id,
                    rel_path: rel.clone(),
                    sha256: sha.clone(),
                    size: *size,
                })
                .collect(),
            included_unknown: a.included_unknown,
            manifest_json,
            confirmed_at: now,
        });
    }
    Ok(effects)
}
