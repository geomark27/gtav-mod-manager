//! Two-phase transaction execution with write-ahead journaling.

use chrono::Utc;
use gmm_domain::{StepStatus, TransactionId, TransactionStatus};
use gmm_store::{NewStep, TransactionRow};

use crate::context::EngineContext;
use crate::error::{Error, Result};
use crate::journal::{effects_from, finish_rollback, undo_all};
use crate::plan::Plan;
use crate::steps::{artifact_name, delete_tombstones, perform, precheck};

/// Executes a plan as one journaled transaction.
///
/// Prepare phase: every step is journaled as `started` before its effect and `done` after it.
/// Commit phase: status `committing`, tombstones deleted, database effects committed atomically.
/// Any error before `committing` rolls back; a simulated crash returns [`Error::Crashed`]
/// immediately without touching the journal again.
pub fn execute(ctx: &EngineContext<'_>, plan: Plan) -> Result<TransactionId> {
    let entry_changes = !plan.summary.entry_updates.is_empty()
        || !plan.summary.entry_removals.is_empty()
        || plan.summary.adopt.is_some();
    if plan.steps.is_empty() && !entry_changes {
        return Err(Error::InvalidInput {
            what: "the plan has no steps".into(),
        });
    }
    ensure_no_pending_work(ctx)?;
    let tx = TransactionId::new();
    let now = Utc::now();
    let summary_json = serde_json::to_string(&plan.summary).map_err(|e| Error::InvalidState {
        what: format!("cannot serialize the plan summary: {e}"),
    })?;
    let row = TransactionRow {
        id: tx,
        installation_id: ctx.installation_id,
        kind: plan.kind,
        status: TransactionStatus::Open,
        summary_json,
        error: None,
        created_at: now,
        finished_at: None,
    };
    let mut new_steps = Vec::with_capacity(plan.steps.len());
    for (i, s) in plan.steps.iter().enumerate() {
        new_steps.push(NewStep {
            seq: u32::try_from(i).map_err(|_| Error::InvalidInput {
                what: "too many steps".into(),
            })?,
            kind: s.kind,
            mod_id: s.mod_id,
            rel_path: s.rel_path.clone(),
            sha256: s.sha256.clone(),
        });
    }
    ctx.db.create_transaction(&row, &new_steps, now)?;
    tracing::info!(%tx, installation_id = %ctx.installation_id, steps = new_steps.len(), "transaction opened");

    if let Err(e) = prepare(ctx, tx, &new_steps) {
        return fail_before_commit(ctx, tx, e);
    }
    if let Err(e) =
        ctx.db
            .set_transaction_status(tx, TransactionStatus::Committing, None, Utc::now())
    {
        return fail_before_commit(ctx, tx, e.into());
    }
    finalize(ctx, tx, &row)
}

/// One transaction at a time per installation; unresolved ones block new work.
fn ensure_no_pending_work(ctx: &EngineContext<'_>) -> Result<()> {
    if ctx.db.installation_has_attention(ctx.installation_id)? {
        return Err(Error::InvalidState {
            what: "the installation has a transaction that needs attention".into(),
        });
    }
    if ctx
        .db
        .transactions_needing_recovery()?
        .iter()
        .any(|t| t.installation_id == ctx.installation_id)
    {
        return Err(Error::InvalidState {
            what: "the installation has an unrecovered transaction".into(),
        });
    }
    Ok(())
}

fn prepare(ctx: &EngineContext<'_>, tx: TransactionId, steps: &[NewStep]) -> Result<()> {
    for step in steps {
        precheck(ctx, step.kind, &step.rel_path)?;
        let artifact = artifact_name(step.kind, &step.rel_path, step.sha256.as_ref(), tx)?;
        ctx.db.mark_step(
            tx,
            step.seq,
            StepStatus::Started,
            artifact.as_deref(),
            None,
            None,
            Utc::now(),
        )?;
        let created = perform(ctx, tx, step.kind, &step.rel_path, step.sha256.as_ref())?;
        ctx.db.mark_step(
            tx,
            step.seq,
            StepStatus::Done,
            None,
            Some(created),
            None,
            Utc::now(),
        )?;
    }
    Ok(())
}

fn fail_before_commit(
    ctx: &EngineContext<'_>,
    tx: TransactionId,
    cause: Error,
) -> Result<TransactionId> {
    if cause.is_crash() {
        return Err(cause);
    }
    tracing::warn!(%tx, error = %cause, "transaction failed; rolling back");
    let msg = cause.to_string();
    ctx.db
        .set_transaction_status(tx, TransactionStatus::RollingBack, Some(&msg), Utc::now())?;
    let Some((_, steps)) = ctx.db.load_transaction(tx)? else {
        return Err(Error::NotFound {
            what: format!("transaction {tx}"),
        });
    };
    let env = ctx.env();
    let outcome = undo_all(&env, tx, &steps)?;
    let blocked = outcome.clone();
    finish_rollback(&env, tx, outcome, Some(&msg))?;
    match blocked {
        None => Err(cause),
        Some(reason) => Err(Error::NeedsAttention { tx, reason }),
    }
}

fn finalize(
    ctx: &EngineContext<'_>,
    tx: TransactionId,
    row: &TransactionRow,
) -> Result<TransactionId> {
    let result = (|| -> Result<()> {
        let Some((_, steps)) = ctx.db.load_transaction(tx)? else {
            return Err(Error::NotFound {
                what: format!("transaction {tx}"),
            });
        };
        delete_tombstones(&ctx.env(), &steps)?;
        let effects = effects_from(row)?;
        ctx.db.commit_transaction(tx, &effects, Utc::now())?;
        Ok(())
    })();
    match result {
        Ok(()) => {
            tracing::info!(%tx, "transaction committed");
            Ok(tx)
        }
        Err(e) if e.is_crash() => Err(e),
        Err(e) => {
            tracing::error!(%tx, error = %e, "commit incomplete; recovery will finish it");
            Err(Error::CommitIncomplete { tx })
        }
    }
}
