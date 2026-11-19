//! Transactions journal: transactions, their steps and the atomic commit.
//!
//! Every multi-row change runs inside one SQLite transaction; state transitions are validated
//! against the stored state, never against what the caller believes.

use chrono::{DateTime, Utc};
use gmm_domain::{
    InstallationId, Mod, ModEntry, ModEntryId, ModId, ModState, RelPath, Sha256, StepKind,
    StepStatus, TransactionId, TransactionKind, TransactionStatus,
};
use rusqlite::{OptionalExtension, Row, params};

use super::mods::{insert_entry, insert_mod};
use super::vault::upsert_vault_object;
use super::{is_conflict, parse_col, to_u64};
use crate::database::Database;
use crate::error::{Error, Result};
use crate::time::{from_db, to_db};

const TX_COLUMNS: &str =
    "id, installation_id, kind, status, summary_json, error, created_at, finished_at";
const STEP_COLUMNS: &str = "transaction_id, seq, kind, mod_id, rel_path, sha256, artifact_name, \
                            created_by_step, status, error, updated_at";

/// A step to register when a transaction is created (status starts as `pending`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewStep {
    /// Position in the transaction (unique per transaction).
    pub seq: u32,
    /// What the step does.
    pub kind: StepKind,
    /// Mod the step belongs to, if any.
    pub mod_id: Option<ModId>,
    /// Target path inside the installation.
    pub rel_path: RelPath,
    /// Expected content hash, if any.
    pub sha256: Option<Sha256>,
}

/// A stored transaction step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepRow {
    /// Owning transaction.
    pub transaction_id: TransactionId,
    /// Position in the transaction.
    pub seq: u32,
    /// What the step does.
    pub kind: StepKind,
    /// Mod the step belongs to, if any.
    pub mod_id: Option<ModId>,
    /// Target path inside the installation.
    pub rel_path: RelPath,
    /// Expected content hash, if any.
    pub sha256: Option<Sha256>,
    /// Temp/tombstone file name created by the step.
    pub artifact_name: Option<String>,
    /// True when the step created the object (undo may remove it).
    pub created_by_step: bool,
    /// Current status.
    pub status: StepStatus,
    /// Last error, if any.
    pub error: Option<String>,
    /// Last update time.
    pub updated_at: DateTime<Utc>,
}

/// A stored transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionRow {
    /// Identifier.
    pub id: TransactionId,
    /// Installation the transaction acts on.
    pub installation_id: InstallationId,
    /// What the transaction does.
    pub kind: TransactionKind,
    /// Current status.
    pub status: TransactionStatus,
    /// Free-form JSON summary for the history view.
    pub summary_json: String,
    /// Last error, if any.
    pub error: Option<String>,
    /// Creation time.
    pub created_at: DateTime<Utc>,
    /// Time it reached a final or attention status.
    pub finished_at: Option<DateTime<Utc>>,
}

/// A mod to register atomically when an adopt transaction commits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdoptRecord {
    /// The mod row.
    pub r#mod: Mod,
    /// Its file entries.
    pub entries: Vec<ModEntry>,
    /// Whether the user included files classified as unknown.
    pub included_unknown: bool,
    /// Snapshot of what the user confirmed.
    pub manifest_json: String,
    /// When the user confirmed ownership.
    pub confirmed_at: DateTime<Utc>,
}

/// Everything `commit_transaction` applies together with the status change.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommitEffects {
    /// New states for existing mods.
    pub mod_states: Vec<(ModId, ModState)>,
    /// Mod to insert (adopt).
    pub adopt: Option<AdoptRecord>,
    /// Vault objects to register or re-verify: `(sha256, size)`.
    pub vault_objects: Vec<(Sha256, u64)>,
    /// New `(sha256, size)` of existing entries (re-adopt). Applied after the vault objects.
    pub entry_updates: Vec<(ModEntryId, Sha256, u64)>,
    /// Entries to delete (re-adopt of missing files). Removing every entry of a mod is invalid.
    pub entry_removals: Vec<ModEntryId>,
}

/// Applies `entry_updates` and `entry_removals`; fails when a mod would end up with no entries.
fn apply_entry_changes(conn: &rusqlite::Connection, effects: &CommitEffects) -> Result<()> {
    let mut touched: Vec<String> = Vec::new();
    for (id, sha, size) in &effects.entry_updates {
        let size = i64::try_from(*size).map_err(|_| Error::Corrupt {
            table: "mod_entries",
            column: "size",
            value: size.to_string(),
        })?;
        let n = conn.execute(
            "UPDATE mod_entries SET sha256 = ?2, size = ?3 WHERE id = ?1",
            params![id.to_string(), sha.as_str(), size],
        )?;
        if n == 0 {
            return Err(Error::NotFound { what: "mod entry" });
        }
    }
    for id in &effects.entry_removals {
        let owner: Option<String> = conn
            .query_row(
                "SELECT mod_id FROM mod_entries WHERE id = ?1",
                [id.to_string()],
                |r| r.get(0),
            )
            .optional()?;
        let Some(owner) = owner else {
            return Err(Error::NotFound { what: "mod entry" });
        };
        conn.execute("DELETE FROM mod_entries WHERE id = ?1", [id.to_string()])?;
        if !touched.contains(&owner) {
            touched.push(owner);
        }
    }
    for owner in touched {
        let left: i64 = conn.query_row(
            "SELECT COUNT(*) FROM mod_entries WHERE mod_id = ?1",
            [&owner],
            |r| r.get(0),
        )?;
        if left == 0 {
            return Err(Error::InvalidState {
                what: format!("removing these entries would leave mod {owner} without files"),
            });
        }
    }
    Ok(())
}

fn tx_from_row(r: &Row<'_>) -> Result<TransactionRow> {
    let id: String = r.get(0)?;
    let inst: String = r.get(1)?;
    let kind: String = r.get(2)?;
    let status: String = r.get(3)?;
    let created: String = r.get(6)?;
    let finished: Option<String> = r.get(7)?;
    Ok(TransactionRow {
        id: parse_col("transactions", "id", &id)?,
        installation_id: parse_col("transactions", "installation_id", &inst)?,
        kind: parse_col("transactions", "kind", &kind)?,
        status: parse_col("transactions", "status", &status)?,
        summary_json: r.get(4)?,
        error: r.get(5)?,
        created_at: from_db("transactions", "created_at", &created)?,
        finished_at: finished
            .map(|f| from_db("transactions", "finished_at", &f))
            .transpose()?,
    })
}

fn step_from_row(r: &Row<'_>) -> Result<StepRow> {
    const T: &str = "transaction_steps";
    let tx: String = r.get(0)?;
    let seq: i64 = r.get(1)?;
    let kind: String = r.get(2)?;
    let mod_id: Option<String> = r.get(3)?;
    let rel: String = r.get(4)?;
    let sha: Option<String> = r.get(5)?;
    let created_by: i64 = r.get(7)?;
    let status: String = r.get(8)?;
    let updated: String = r.get(10)?;
    Ok(StepRow {
        transaction_id: parse_col(T, "transaction_id", &tx)?,
        seq: u32::try_from(to_u64(T, "seq", seq)?).map_err(|_| Error::Corrupt {
            table: T,
            column: "seq",
            value: seq.to_string(),
        })?,
        kind: parse_col(T, "kind", &kind)?,
        mod_id: mod_id.map(|m| parse_col(T, "mod_id", &m)).transpose()?,
        rel_path: RelPath::parse(&rel).map_err(|_| Error::Corrupt {
            table: T,
            column: "rel_path",
            value: rel.clone(),
        })?,
        sha256: sha
            .map(|s| {
                Sha256::parse(&s).map_err(|_| Error::Corrupt {
                    table: T,
                    column: "sha256",
                    value: s.clone(),
                })
            })
            .transpose()?,
        artifact_name: r.get(6)?,
        created_by_step: created_by != 0,
        status: parse_col(T, "status", &status)?,
        error: r.get(9)?,
        updated_at: from_db(T, "updated_at", &updated)?,
    })
}

fn stored_status(conn: &rusqlite::Connection, tx: TransactionId) -> Result<TransactionStatus> {
    let s: Option<String> = conn
        .query_row(
            "SELECT status FROM transactions WHERE id = ?1",
            [tx.to_string()],
            |r| r.get(0),
        )
        .optional()?;
    match s {
        Some(s) => parse_col("transactions", "status", &s),
        None => Err(Error::NotFound {
            what: "transaction",
        }),
    }
}

fn rows<T>(
    stmt: &mut rusqlite::Statement<'_>,
    args: impl rusqlite::Params,
    f: fn(&Row<'_>) -> Result<T>,
) -> Result<Vec<T>> {
    let it = stmt.query_map(args, |r| Ok(f(r)))?;
    let mut out = Vec::new();
    for row in it {
        out.push(row??);
    }
    Ok(out)
}

impl Database {
    /// Creates a transaction (status forced to `open`) and all its steps (`pending`)
    /// in one SQLite transaction. `Conflict` on a duplicate transaction id or step `seq`.
    pub fn create_transaction(
        &self,
        row: &TransactionRow,
        steps: &[NewStep],
        now: DateTime<Utc>,
    ) -> Result<()> {
        let db_tx = self.conn().unchecked_transaction()?;
        db_tx
            .execute(
                &format!(
                    "INSERT INTO transactions ({TX_COLUMNS}) VALUES (?1,?2,?3,?4,?5,NULL,?6,NULL)"
                ),
                params![
                    row.id.to_string(),
                    row.installation_id.to_string(),
                    row.kind.as_str(),
                    TransactionStatus::Open.as_str(),
                    row.summary_json,
                    to_db(row.created_at),
                ],
            )
            .map_err(|e| {
                if is_conflict(&e) {
                    Error::Conflict {
                        what: "transaction",
                    }
                } else {
                    e.into()
                }
            })?;
        for s in steps {
            db_tx
                .execute(
                    &format!(
                        "INSERT INTO transaction_steps ({STEP_COLUMNS}) \
                         VALUES (?1,?2,?3,?4,?5,?6,NULL,0,?7,NULL,?8)"
                    ),
                    params![
                        row.id.to_string(),
                        i64::from(s.seq),
                        s.kind.as_str(),
                        s.mod_id.map(|m| m.to_string()),
                        s.rel_path.as_str(),
                        s.sha256.as_ref().map(Sha256::as_str),
                        StepStatus::Pending.as_str(),
                        to_db(now),
                    ],
                )
                .map_err(|e| {
                    if is_conflict(&e) {
                        Error::Conflict {
                            what: "transaction step",
                        }
                    } else {
                        e.into()
                    }
                })?;
        }
        db_tx.commit()?;
        Ok(())
    }

    /// Moves a step to `status`, validating the transition against the stored status.
    ///
    /// `artifact_name` and `created_by_step` are left unchanged when `None`; `error` is
    /// always overwritten (`None` clears it). `NotFound` when the step does not exist,
    /// `InvalidState` on an illegal transition.
    #[allow(clippy::too_many_arguments)] // signature fixed by the T12 spec
    pub fn mark_step(
        &self,
        tx: TransactionId,
        seq: u32,
        status: StepStatus,
        artifact_name: Option<&str>,
        created_by_step: Option<bool>,
        error: Option<&str>,
        now: DateTime<Utc>,
    ) -> Result<()> {
        let db_tx = self.conn().unchecked_transaction()?;
        let current: Option<String> = db_tx
            .query_row(
                "SELECT status FROM transaction_steps WHERE transaction_id = ?1 AND seq = ?2",
                params![tx.to_string(), i64::from(seq)],
                |r| r.get(0),
            )
            .optional()?;
        let current: StepStatus = match current {
            Some(s) => parse_col("transaction_steps", "status", &s)?,
            None => {
                return Err(Error::NotFound {
                    what: "transaction step",
                });
            }
        };
        if !current.can_transition_to(status) {
            return Err(Error::InvalidState {
                what: format!(
                    "step {seq} of transaction {tx} cannot go from {current} to {status}"
                ),
            });
        }
        db_tx.execute(
            "UPDATE transaction_steps SET status = ?3, \
             artifact_name = COALESCE(?4, artifact_name), \
             created_by_step = COALESCE(?5, created_by_step), \
             error = ?6, updated_at = ?7 \
             WHERE transaction_id = ?1 AND seq = ?2",
            params![
                tx.to_string(),
                i64::from(seq),
                status.as_str(),
                artifact_name,
                created_by_step.map(i64::from),
                error,
                to_db(now),
            ],
        )?;
        db_tx.commit()?;
        Ok(())
    }

    /// Moves a transaction to `status`, validating against the stored status.
    ///
    /// `finished_at` is set for `committed`, `rolled_back` and `needs_attention`. `error` is
    /// overwritten (`None` clears it). Prefer `commit_transaction` for `committed`, which
    /// also applies the effects atomically. `NotFound` / `InvalidState` as usual.
    pub fn set_transaction_status(
        &self,
        tx: TransactionId,
        status: TransactionStatus,
        error: Option<&str>,
        now: DateTime<Utc>,
    ) -> Result<()> {
        let db_tx = self.conn().unchecked_transaction()?;
        let current = stored_status(&db_tx, tx)?;
        if !current.can_transition_to(status) {
            return Err(Error::InvalidState {
                what: format!("transaction {tx} cannot go from {current} to {status}"),
            });
        }
        let finished = (status.is_terminal() || status == TransactionStatus::NeedsAttention)
            .then(|| to_db(now));
        db_tx.execute(
            "UPDATE transactions SET status = ?2, error = ?3, finished_at = ?4 WHERE id = ?1",
            params![tx.to_string(), status.as_str(), error, finished],
        )?;
        db_tx.commit()?;
        Ok(())
    }

    /// Commits a transaction atomically: requires the stored status to be `committing`,
    /// registers vault objects, inserts the adopt record, applies mod states and sets the
    /// status to `committed`. Any failure rolls everything back and leaves the status as is.
    ///
    /// Entry updates and removals are applied after the vault objects (so the foreign key
    /// holds) and before the mod states.
    ///
    /// Errors: `NotFound` (transaction, a mod in `mod_states` or an entry), `InvalidState`
    /// (status not `committing`, the adopted mod belongs to another installation, or a mod
    /// would be left without entries), `Conflict`
    /// (duplicate mod name or entry path).
    pub fn commit_transaction(
        &self,
        tx: TransactionId,
        effects: &CommitEffects,
        now: DateTime<Utc>,
    ) -> Result<()> {
        let db_tx = self.conn().unchecked_transaction()?;
        let installation: String = db_tx
            .query_row(
                "SELECT installation_id FROM transactions WHERE id = ?1",
                [tx.to_string()],
                |r| r.get(0),
            )
            .optional()?
            .ok_or(Error::NotFound {
                what: "transaction",
            })?;
        let current = stored_status(&db_tx, tx)?;
        if current != TransactionStatus::Committing {
            return Err(Error::InvalidState {
                what: format!("transaction {tx} is {current}, expected committing"),
            });
        }
        for (sha, size) in &effects.vault_objects {
            upsert_vault_object(&db_tx, sha, *size, now)?;
        }
        if let Some(adopt) = &effects.adopt {
            if adopt.r#mod.installation_id.to_string() != installation {
                return Err(Error::InvalidState {
                    what: format!("adopted mod does not belong to the installation of {tx}"),
                });
            }
            insert_mod(&db_tx, &adopt.r#mod)?;
            for e in &adopt.entries {
                if e.mod_id != adopt.r#mod.id {
                    return Err(Error::InvalidState {
                        what: "adopted entry references a different mod".to_owned(),
                    });
                }
                insert_entry(&db_tx, e)?;
            }
            db_tx.execute(
                "INSERT INTO ownership_confirmations \
                 (id, mod_id, confirmed_at, included_unknown, manifest_json) \
                 VALUES (?1,?2,?3,?4,?5)",
                params![
                    ModId::new().to_string(),
                    adopt.r#mod.id.to_string(),
                    to_db(adopt.confirmed_at),
                    i64::from(adopt.included_unknown),
                    adopt.manifest_json,
                ],
            )?;
        }
        apply_entry_changes(&db_tx, effects)?;
        for (mod_id, state) in &effects.mod_states {
            let n = db_tx.execute(
                "UPDATE mods SET state = ?2, updated_at = ?3 WHERE id = ?1",
                params![mod_id.to_string(), state.as_str(), to_db(now)],
            )?;
            if n == 0 {
                return Err(Error::NotFound { what: "mod" });
            }
        }
        db_tx.execute(
            "UPDATE transactions SET status = ?2, error = NULL, finished_at = ?3 WHERE id = ?1",
            params![
                tx.to_string(),
                TransactionStatus::Committed.as_str(),
                to_db(now)
            ],
        )?;
        db_tx.commit()?;
        Ok(())
    }

    /// Loads a transaction with its steps ordered by `seq`.
    pub fn load_transaction(
        &self,
        tx: TransactionId,
    ) -> Result<Option<(TransactionRow, Vec<StepRow>)>> {
        let Some(row) = self
            .conn()
            .query_row(
                &format!("SELECT {TX_COLUMNS} FROM transactions WHERE id = ?1"),
                [tx.to_string()],
                |r| Ok(tx_from_row(r)),
            )
            .optional()?
            .transpose()?
        else {
            return Ok(None);
        };
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {STEP_COLUMNS} FROM transaction_steps WHERE transaction_id = ?1 ORDER BY seq"
        ))?;
        let steps = rows(&mut stmt, [tx.to_string()], step_from_row)?;
        Ok(Some((row, steps)))
    }

    /// Transactions startup recovery must handle (`open`, `committing`, `rolling_back`),
    /// oldest first.
    pub fn transactions_needing_recovery(&self) -> Result<Vec<TransactionRow>> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {TX_COLUMNS} FROM transactions \
             WHERE status IN ('open','committing','rolling_back') \
             ORDER BY created_at, id"
        ))?;
        rows(&mut stmt, [], tx_from_row)
    }

    /// Pages the history of an installation, newest first, with the total count.
    pub fn list_transactions(
        &self,
        installation_id: InstallationId,
        limit: u32,
        offset: u32,
    ) -> Result<(Vec<TransactionRow>, u64)> {
        let inst = installation_id.to_string();
        let total: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM transactions WHERE installation_id = ?1",
            [&inst],
            |r| r.get(0),
        )?;
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {TX_COLUMNS} FROM transactions WHERE installation_id = ?1 \
             ORDER BY created_at DESC, id DESC LIMIT ?2 OFFSET ?3"
        ))?;
        let items = rows(
            &mut stmt,
            params![inst, i64::from(limit), i64::from(offset)],
            tx_from_row,
        )?;
        Ok((items, to_u64("transactions", "count", total)?))
    }

    /// True when the installation has a transaction in `needs_attention`.
    pub fn installation_has_attention(&self, installation_id: InstallationId) -> Result<bool> {
        let n: i64 = self.conn().query_row(
            "SELECT EXISTS (SELECT 1 FROM transactions \
             WHERE installation_id = ?1 AND status = 'needs_attention')",
            [installation_id.to_string()],
            |r| r.get(0),
        )?;
        Ok(n != 0)
    }
}
