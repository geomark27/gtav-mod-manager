//! Transaction history: a readable view of the journal.

use std::collections::{HashMap, HashSet};

use gmm_domain::{ModId, TransactionKind, TransactionStatus};
use gmm_engine::TxSummary;
use gmm_store::{Database, TransactionRow};
use serde::Serialize;
use ts_rs::TS;

use crate::app::App;
use crate::error::AppError;
use crate::installations::{not_found, parse_id};
use crate::scan::rfc3339;

/// Largest page `list_transactions` returns.
pub const MAX_HISTORY_PAGE: u32 = 200;

/// One journaled transaction as shown in the history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TransactionDto {
    /// Transaction id.
    pub id: String,
    /// What it did.
    pub kind: TransactionKind,
    /// How it ended (or where it stands).
    pub status: TransactionStatus,
    /// Start time, RFC 3339 UTC.
    pub created_at: String,
    /// Finish time, `null` while unfinished.
    pub finished_at: Option<String>,
    /// Last error, if any.
    pub error: Option<String>,
    /// Number of file steps.
    #[ts(type = "number")]
    pub step_count: u64,
    /// Names of the mods involved that still exist (the adopted name is always included).
    pub mod_names: Vec<String>,
    /// One-line description, e.g. "Disabled 2 mods".
    pub description: String,
}

/// One page of the history, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TransactionPageDto {
    /// Transactions of this page.
    pub items: Vec<TransactionDto>,
    /// Total transactions of the installation.
    #[ts(type = "number")]
    pub total: u64,
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn describe(
    kind: TransactionKind,
    summary: &TxSummary,
    names: &[String],
    changed: usize,
) -> String {
    let quoted = |label: &str| match names {
        [one] => format!("{label} mod “{one}”"),
        _ => format!("{label} a mod"),
    };
    match kind {
        TransactionKind::Adopt => match &summary.adopt {
            Some(a) => format!("Adopted mod “{}”", a.name),
            None => quoted("Re-adopted"),
        },
        TransactionKind::Enable => format!("Enabled {}", plural(changed, "mod", "mods")),
        TransactionKind::Disable => format!("Disabled {}", plural(changed, "mod", "mods")),
        TransactionKind::Group => format!(
            "Switched a category ({} changed)",
            plural(changed, "mod", "mods")
        ),
        TransactionKind::Profile => "Applied profile".to_owned(),
        TransactionKind::Restore => quoted("Restored"),
    }
}

/// Builds the DTO of a journal row; mod ids that no longer exist are simply left out.
pub(crate) fn transaction_dto(
    db: &Database,
    row: &TransactionRow,
    names: &HashMap<ModId, String>,
) -> Result<TransactionDto, AppError> {
    // An unreadable summary only costs the description detail, never the history.
    let summary: TxSummary = serde_json::from_str(&row.summary_json).unwrap_or_default();
    let steps = db
        .load_transaction(row.id)?
        .map(|(_, steps)| steps)
        .unwrap_or_default();
    let mut ids: Vec<ModId> = summary.mod_states.iter().map(|(id, _)| *id).collect();
    ids.extend(steps.iter().filter_map(|s| s.mod_id));
    let mut seen = HashSet::new();
    ids.retain(|id| seen.insert(*id));
    let mut mod_names: Vec<String> = ids.iter().filter_map(|id| names.get(id).cloned()).collect();
    if let Some(a) = &summary.adopt
        && !mod_names.contains(&a.name)
    {
        mod_names.push(a.name.clone());
    }
    mod_names.sort_by_key(|n| n.to_lowercase());
    let changed = if summary.mod_states.is_empty() {
        ids.len()
    } else {
        summary.mod_states.len()
    };
    Ok(TransactionDto {
        id: row.id.to_string(),
        kind: row.kind,
        status: row.status,
        created_at: rfc3339(row.created_at),
        finished_at: row.finished_at.map(rfc3339),
        error: row.error.clone(),
        step_count: steps.len() as u64,
        description: describe(row.kind, &summary, &mod_names, changed),
        mod_names,
    })
}

/// Builds DTOs for rows, resolving mod names once.
pub(crate) fn transaction_dtos(
    db: &Database,
    installation_id: gmm_domain::InstallationId,
    rows: &[TransactionRow],
) -> Result<Vec<TransactionDto>, AppError> {
    let names: HashMap<ModId, String> = db
        .list_mods(installation_id)?
        .into_iter()
        .map(|m| (m.id, m.name))
        .collect();
    rows.iter()
        .map(|r| transaction_dto(db, r, &names))
        .collect()
}

impl App {
    /// Pages the transaction history of an installation, newest first.
    pub fn list_transactions(
        &self,
        installation_id: &str,
        limit: u32,
        offset: u32,
    ) -> Result<TransactionPageDto, AppError> {
        let id = parse_id(installation_id)?;
        let db = self.lock_db()?;
        if db.get_installation(id)?.is_none() {
            return Err(not_found());
        }
        let (rows, total) = db.list_transactions(id, limit.clamp(1, MAX_HISTORY_PAGE), offset)?;
        Ok(TransactionPageDto {
            items: transaction_dtos(&db, id, &rows)?,
            total,
        })
    }
}
