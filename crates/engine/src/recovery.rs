//! Crash recovery: finishes or undoes every transaction left unfinished.

use std::path::PathBuf;

use chrono::Utc;
use gmm_domain::{InstallationId, StepStatus, TransactionId, TransactionStatus};
use gmm_platform::Fs;
use gmm_store::{Database, StepRow, TransactionRow};

use crate::error::{Error, Result};
use crate::journal::{effects_from, finish_rollback, undo_all};
use crate::steps::{Env, delete_tombstones};
use crate::vault::Vault;

/// What [`recover`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecoveryReport {
    /// Transactions undone (`rolled_back`).
    pub rolled_back: Vec<TransactionId>,
    /// Transactions rolled forward (`committed`).
    pub committed: Vec<TransactionId>,
    /// Transactions whose undo could not be proven safe (`needs_attention`).
    pub needs_attention: Vec<TransactionId>,
    /// Transactions left untouched (root unavailable, or an unexpected failure that was logged).
    pub skipped: Vec<TransactionId>,
}

/// Runs recovery for every unfinished transaction. Call it before any action is enabled.
///
/// `roots` maps an installation to its root directory, `None` when it is unavailable: such
/// transactions are left untouched and reported in `skipped`. Recovery only ever deletes
/// artifacts recorded in the journal (I-08).
pub fn recover(
    fs: &dyn Fs,
    db: &Database,
    vault: &Vault,
    roots: &dyn Fn(InstallationId) -> Option<PathBuf>,
) -> Result<RecoveryReport> {
    let mut report = RecoveryReport::default();
    for row in db.transactions_needing_recovery()? {
        let Some(root) = roots(row.installation_id) else {
            tracing::warn!(tx = %row.id, "installation root unavailable; transaction skipped");
            report.skipped.push(row.id);
            continue;
        };
        let env = Env {
            fs,
            db,
            vault,
            root: &root,
        };
        match recover_one(&env, &row) {
            Ok(Outcome::RolledBack) => report.rolled_back.push(row.id),
            Ok(Outcome::Committed) => report.committed.push(row.id),
            Ok(Outcome::NeedsAttention) => report.needs_attention.push(row.id),
            Err(e) if e.is_crash() => return Err(e),
            Err(e) => {
                tracing::error!(tx = %row.id, error = %e, "recovery failed; transaction left as is");
                report.skipped.push(row.id);
            }
        }
    }
    Ok(report)
}

enum Outcome {
    RolledBack,
    Committed,
    NeedsAttention,
}

fn recover_one(env: &Env<'_>, row: &TransactionRow) -> Result<Outcome> {
    let tx = row.id;
    let Some((_, steps)) = env.db.load_transaction(tx)? else {
        return Err(Error::NotFound {
            what: format!("transaction {tx}"),
        });
    };
    match row.status {
        TransactionStatus::Committing => roll_forward(env, row, &steps),
        TransactionStatus::Open | TransactionStatus::RollingBack => {
            if row.status == TransactionStatus::Open {
                env.db.set_transaction_status(
                    tx,
                    TransactionStatus::RollingBack,
                    Some("interrupted; rolled back by recovery"),
                    Utc::now(),
                )?;
            }
            let outcome = undo_all(env, tx, &steps)?;
            let blocked = outcome.is_some();
            finish_rollback(env, tx, outcome, row.error.as_deref())?;
            Ok(if blocked {
                Outcome::NeedsAttention
            } else {
                Outcome::RolledBack
            })
        }
        other => Err(Error::InvalidState {
            what: format!("transaction {tx} is {other}; nothing to recover"),
        }),
    }
}

fn roll_forward(env: &Env<'_>, row: &TransactionRow, steps: &[StepRow]) -> Result<Outcome> {
    let tx = row.id;
    if steps.iter().any(|s| s.status != StepStatus::Done) {
        env.db.set_transaction_status(
            tx,
            TransactionStatus::NeedsAttention,
            Some("committing transaction has steps that are not done"),
            Utc::now(),
        )?;
        return Ok(Outcome::NeedsAttention);
    }
    delete_tombstones(env, steps)?;
    let effects = effects_from(row)?;
    env.db.commit_transaction(tx, &effects, Utc::now())?;
    Ok(Outcome::Committed)
}
