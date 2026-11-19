//! The dashboard: counts and state of one installation in a single call.

use std::collections::HashSet;

use gmm_domain::ModState;
use serde::Serialize;
use ts_rs::TS;

use crate::app::App;
use crate::baseline::BaselineDto;
use crate::error::AppError;
use crate::guards::GuardStatusDto;
use crate::history::{TransactionDto, transaction_dtos};
use crate::installations::{not_found, parse_id};
use crate::inventory::baseline_dto;
use crate::scan::{ClassCountDto, ScanDto, counts_dto};

/// Number of recent transactions on the dashboard.
const RECENT_TRANSACTIONS: u32 = 5;

/// Mods per state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModCountsDto {
    /// All managed mods.
    #[ts(type = "number")]
    pub total: u64,
    /// Mods in state `enabled`.
    #[ts(type = "number")]
    pub enabled: u64,
    /// Mods in state `disabled`.
    #[ts(type = "number")]
    pub disabled: u64,
    /// Mods in state `drifted`.
    #[ts(type = "number")]
    pub drifted: u64,
    /// Mods in state `error`.
    #[ts(type = "number")]
    pub error: u64,
}

/// Everything the dashboard shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DashboardDto {
    /// Mods per state.
    pub mods: ModCountsDto,
    /// Inventory items per classification.
    pub inventory: Vec<ClassCountDto>,
    /// Most recent scan, if any.
    pub last_scan: Option<ScanDto>,
    /// Current baseline.
    pub baseline: BaselineDto,
    /// Bytes of the distinct vault objects the mods of this installation reference.
    #[ts(type = "number")]
    pub vault_bytes: u64,
    /// The latest transactions, newest first.
    pub recent_transactions: Vec<TransactionDto>,
    /// Guard state.
    pub guard: GuardStatusDto,
}

impl App {
    /// Collects the dashboard of an installation.
    pub fn dashboard(&self, installation_id: &str) -> Result<DashboardDto, AppError> {
        let id = parse_id(installation_id)?;
        // Probes the guards (and refreshes the installation status) before the data is read.
        let guard = self.guard_status(installation_id)?;
        let db = self.lock_db()?;
        let inst = db.get_installation(id)?.ok_or_else(not_found)?;
        let mut mods = ModCountsDto {
            total: 0,
            enabled: 0,
            disabled: 0,
            drifted: 0,
            error: 0,
        };
        for m in db.list_mods(id)? {
            mods.total += 1;
            match m.state {
                ModState::Enabled => mods.enabled += 1,
                ModState::Disabled => mods.disabled += 1,
                ModState::Drifted => mods.drifted += 1,
                ModState::Error => mods.error += 1,
            }
        }
        let mut objects = HashSet::new();
        let mut vault_bytes = 0_u64;
        for (_, _, entry) in db.list_entries_for_installation(id)? {
            if objects.insert(entry.sha256.as_str().to_owned()) {
                vault_bytes += entry.size;
            }
        }
        let (rows, _) = db.list_transactions(id, RECENT_TRANSACTIONS, 0)?;
        Ok(DashboardDto {
            mods,
            inventory: counts_dto(db.inventory_counts(id)?),
            last_scan: db.last_scan(id)?.as_ref().map(ScanDto::from),
            baseline: baseline_dto(&db, &inst)?,
            vault_bytes,
            recent_transactions: transaction_dtos(&db, id, &rows)?,
            guard,
        })
    }
}
