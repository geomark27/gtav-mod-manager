//! Inventory queries: paged listing and the summary shown above it.

use gmm_domain::{BaselineTrust, Category, Classification, Evidence, Installation, InventoryItem};
use gmm_store::{Database, InventoryFilter, MAX_PAGE_SIZE};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::app::App;
use crate::baseline::BaselineDto;
use crate::error::AppError;
use crate::installations::{not_found, parse_id};
use crate::scan::{ClassCountDto, ScanDto, counts_dto, rfc3339};

/// Filter and pagination of `list_inventory`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InventoryFilterDto {
    /// Only these classifications (empty = all).
    pub classifications: Vec<Classification>,
    /// Case-insensitive substring of the path.
    pub search: Option<String>,
    /// Page size (1 to 500).
    pub limit: u32,
    /// Rows to skip.
    pub offset: u32,
}

/// One scanned file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InventoryItemDto {
    /// Path relative to the installation root.
    pub rel_path: String,
    /// Size in bytes.
    #[ts(type = "number")]
    pub size: u64,
    /// Modification time in nanoseconds since the Unix epoch, as a string (exceeds 2^53).
    pub mtime_ns: String,
    /// Content hash, when computed.
    pub sha256: Option<String>,
    /// Classification.
    pub classification: Classification,
    /// Category suggested by catalog patterns.
    pub suggested_category: Option<Category>,
    /// Reasons behind the classification.
    pub evidence: Vec<Evidence>,
    /// First time the file was seen, RFC 3339 UTC.
    pub first_seen_at: String,
    /// Last time the file was seen, RFC 3339 UTC.
    pub last_seen_at: String,
}

impl From<&InventoryItem> for InventoryItemDto {
    fn from(i: &InventoryItem) -> Self {
        Self {
            rel_path: i.rel_path.as_str().to_owned(),
            size: i.size,
            mtime_ns: i.mtime_ns.to_string(),
            sha256: i.sha256.as_ref().map(|s| s.as_str().to_owned()),
            classification: i.classification,
            suggested_category: i.suggested_category,
            evidence: i.evidence.clone(),
            first_seen_at: rfc3339(i.first_seen_at),
            last_seen_at: rfc3339(i.last_seen_at),
        }
    }
}

/// One page of inventory items.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InventoryPageDto {
    /// Items ordered by path.
    pub items: Vec<InventoryItemDto>,
    /// Total items matching the filter.
    #[ts(type = "number")]
    pub total: u64,
}

/// Counts, last scan and baseline of an installation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InventorySummaryDto {
    /// Items per classification (absent classifications have no items).
    pub counts: Vec<ClassCountDto>,
    /// Most recent scan, if any.
    pub last_scan: Option<ScanDto>,
    /// Current baseline.
    pub baseline: BaselineDto,
}

/// The baseline of an installation as shown by the UI (`captured_at` is `None` without one).
pub(crate) fn baseline_dto(db: &Database, inst: &Installation) -> Result<BaselineDto, AppError> {
    if inst.baseline == BaselineTrust::None {
        return Ok(BaselineDto {
            entries: 0,
            trust: inst.baseline,
            captured_at: None,
        });
    }
    Ok(BaselineDto {
        entries: db.load_baseline(inst.id)?.len() as u64,
        trust: inst.baseline,
        captured_at: db.baseline_captured_at(inst.id)?.map(rfc3339),
    })
}

impl App {
    /// Lists inventory items with filter and pagination.
    pub fn list_inventory(
        &self,
        id: &str,
        filter: InventoryFilterDto,
    ) -> Result<InventoryPageDto, AppError> {
        let id = parse_id(id)?;
        let db = self.lock_db()?;
        if db.get_installation(id)?.is_none() {
            return Err(not_found());
        }
        let page = db.list_inventory(
            id,
            &InventoryFilter {
                classifications: filter.classifications,
                search: filter.search.map(|s| s.trim().to_owned()),
                limit: filter.limit.clamp(1, MAX_PAGE_SIZE),
                offset: filter.offset,
            },
        )?;
        Ok(InventoryPageDto {
            items: page.items.iter().map(InventoryItemDto::from).collect(),
            total: page.total,
        })
    }

    /// Returns the counts per classification, the last scan and the baseline.
    pub fn inventory_summary(&self, id: &str) -> Result<InventorySummaryDto, AppError> {
        let id = parse_id(id)?;
        let db = self.lock_db()?;
        let inst = db.get_installation(id)?.ok_or_else(not_found)?;
        Ok(InventorySummaryDto {
            counts: counts_dto(db.inventory_counts(id)?),
            last_scan: db.last_scan(id)?.as_ref().map(ScanDto::from),
            baseline: baseline_dto(&db, &inst)?,
        })
    }
}
