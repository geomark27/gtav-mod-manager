//! Inventory repository: scan results, listing, counts and the hash cache.

use chrono::{DateTime, Utc};
use gmm_domain::{
    Category, Classification, Evidence, InstallationId, InventoryItem, RelPath, ScanId, Sha256,
};
use rusqlite::{ToSql, params, params_from_iter};

use crate::database::Database;
use crate::error::{Error, Result};
use crate::time::{from_db, to_db};

const TABLE: &str = "inventory_items";

/// Maximum page size accepted by `list_inventory`.
pub const MAX_PAGE_SIZE: u32 = 500;

/// Filter and pagination for `list_inventory`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryFilter {
    /// Only these classifications (empty = all).
    pub classifications: Vec<Classification>,
    /// Case-insensitive substring of the path key; `%`, `_` and `\` match literally.
    pub search: Option<String>,
    /// Page size (clamped to `MAX_PAGE_SIZE`).
    pub limit: u32,
    /// Rows to skip.
    pub offset: u32,
}

impl Default for InventoryFilter {
    fn default() -> Self {
        Self {
            classifications: Vec::new(),
            search: None,
            limit: 100,
            offset: 0,
        }
    }
}

/// One page of inventory items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryPage {
    /// Items ordered by `path_key`.
    pub items: Vec<InventoryItem>,
    /// Total rows matching the filter (ignoring pagination).
    pub total: u64,
}

fn corrupt(column: &'static str, value: &str) -> Error {
    Error::Corrupt {
        table: TABLE,
        column,
        value: value.to_owned(),
    }
}

fn to_i64(column: &'static str, v: u64) -> Result<i64> {
    i64::try_from(v).map_err(|_| corrupt(column, &v.to_string()))
}

fn to_u64(column: &'static str, v: i64) -> Result<u64> {
    u64::try_from(v).map_err(|_| corrupt(column, &v.to_string()))
}

fn escape_like(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '%' | '_' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

impl Database {
    /// Replaces the inventory of an installation with the result of `scan_id`.
    ///
    /// Single transaction: upsert every item (keeping the stored `first_seen_at`, setting
    /// `last_seen_at = seen_at` and `last_scan_id = scan_id`), then delete rows not seen by this
    /// scan. Any failure rolls everything back.
    pub fn replace_inventory(
        &self,
        installation_id: InstallationId,
        scan_id: ScanId,
        items: &[InventoryItem],
        seen_at: DateTime<Utc>,
    ) -> Result<()> {
        let tx = self.conn().unchecked_transaction()?;
        let inst = installation_id.to_string();
        let scan = scan_id.to_string();
        let seen = to_db(seen_at);
        {
            let mut upsert = tx.prepare_cached(
                "INSERT INTO inventory_items (installation_id, path_key, rel_path, size, mtime_ns, \
                   sha256, classification, suggested_category, evidence_json, first_seen_at, \
                   last_seen_at, last_scan_id) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10, ?11) \
                 ON CONFLICT (installation_id, path_key) DO UPDATE SET \
                   rel_path = excluded.rel_path, size = excluded.size, \
                   mtime_ns = excluded.mtime_ns, sha256 = excluded.sha256, \
                   classification = excluded.classification, \
                   suggested_category = excluded.suggested_category, \
                   evidence_json = excluded.evidence_json, \
                   last_seen_at = excluded.last_seen_at, last_scan_id = excluded.last_scan_id",
            )?;
            for item in items {
                let evidence = serde_json::to_string(&item.evidence)
                    .map_err(|_| corrupt("evidence_json", "unserializable evidence"))?;
                upsert.execute(params![
                    inst,
                    item.rel_path.key(),
                    item.rel_path.as_str(),
                    to_i64("size", item.size)?,
                    item.mtime_ns,
                    item.sha256.as_ref().map(Sha256::as_str),
                    item.classification.as_str(),
                    item.suggested_category.map(Category::as_str),
                    evidence,
                    seen,
                    scan,
                ])?;
            }
        }
        tx.execute(
            "DELETE FROM inventory_items \
             WHERE installation_id = ?1 AND (last_scan_id IS NOT ?2)",
            params![inst, scan],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Lists inventory items of an installation, ordered by path key.
    pub fn list_inventory(
        &self,
        installation_id: InstallationId,
        filter: &InventoryFilter,
    ) -> Result<InventoryPage> {
        let mut clauses = vec!["installation_id = ?".to_owned()];
        let mut args: Vec<Box<dyn ToSql>> = vec![Box::new(installation_id.to_string())];
        if !filter.classifications.is_empty() {
            let marks = vec!["?"; filter.classifications.len()].join(",");
            clauses.push(format!("classification IN ({marks})"));
            for c in &filter.classifications {
                args.push(Box::new(c.as_str()));
            }
        }
        if let Some(search) = filter.search.as_deref().filter(|s| !s.is_empty()) {
            clauses.push("path_key LIKE ? ESCAPE '\\'".to_owned());
            args.push(Box::new(format!(
                "%{}%",
                escape_like(&search.to_lowercase())
            )));
        }
        let where_sql = clauses.join(" AND ");

        let total: i64 = self.conn().query_row(
            &format!("SELECT COUNT(*) FROM inventory_items WHERE {where_sql}"),
            params_from_iter(args.iter().map(|a| a.as_ref())),
            |r| r.get(0),
        )?;

        args.push(Box::new(i64::from(filter.limit.min(MAX_PAGE_SIZE))));
        args.push(Box::new(i64::from(filter.offset)));
        let mut stmt = self.conn().prepare(&format!(
            "SELECT rel_path, size, mtime_ns, sha256, classification, suggested_category, \
                    evidence_json, first_seen_at, last_seen_at, last_scan_id \
             FROM inventory_items WHERE {where_sql} ORDER BY path_key LIMIT ? OFFSET ?"
        ))?;
        let rows = stmt.query_map(params_from_iter(args.iter().map(|a| a.as_ref())), |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, String>(8)?,
                r.get::<_, Option<String>>(9)?,
            ))
        })?;
        let mut items = Vec::new();
        for row in rows {
            let (rel, size, mtime, sha, class, cat, ev, first, last, scan) = row?;
            items.push(InventoryItem {
                installation_id,
                rel_path: RelPath::parse(&rel).map_err(|_| corrupt("rel_path", &rel))?,
                size: to_u64("size", size)?,
                mtime_ns: mtime,
                sha256: sha
                    .as_deref()
                    .map(|s| Sha256::parse(s).map_err(|_| corrupt("sha256", s)))
                    .transpose()?,
                classification: class
                    .parse()
                    .map_err(|_| corrupt("classification", &class))?,
                suggested_category: cat
                    .as_deref()
                    .map(|c| c.parse().map_err(|_| corrupt("suggested_category", c)))
                    .transpose()?,
                evidence: serde_json::from_str::<Vec<Evidence>>(&ev)
                    .map_err(|_| corrupt("evidence_json", &ev))?,
                first_seen_at: from_db(TABLE, "first_seen_at", &first)?,
                last_seen_at: from_db(TABLE, "last_seen_at", &last)?,
                last_scan_id: scan
                    .as_deref()
                    .map(|s| s.parse().map_err(|_| corrupt("last_scan_id", s)))
                    .transpose()?,
            });
        }
        Ok(InventoryPage {
            items,
            total: to_u64("total", total)?,
        })
    }

    /// Counts inventory items per classification, sorted by the classification's `as_str`.
    ///
    /// `Classification` has no `Ord` in `gmm-domain`, hence a sorted `Vec` instead of a map.
    /// Classifications with no items are omitted.
    pub fn inventory_counts(
        &self,
        installation_id: InstallationId,
    ) -> Result<Vec<(Classification, u64)>> {
        let mut stmt = self.conn().prepare(
            "SELECT classification, COUNT(*) FROM inventory_items \
             WHERE installation_id = ?1 GROUP BY classification ORDER BY classification",
        )?;
        let rows = stmt.query_map([installation_id.to_string()], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (class, n) = row?;
            out.push((
                class
                    .parse::<Classification>()
                    .map_err(|_| corrupt("classification", &class))?,
                to_u64("count", n)?,
            ));
        }
        out.sort_by_key(|(c, _)| c.as_str());
        Ok(out)
    }

    /// Returns `(path_key, size, mtime_ns, sha256)` for every row that has a hash.
    pub fn hash_cache(
        &self,
        installation_id: InstallationId,
    ) -> Result<Vec<(String, u64, i64, Sha256)>> {
        let mut stmt = self.conn().prepare(
            "SELECT path_key, size, mtime_ns, sha256 FROM inventory_items \
             WHERE installation_id = ?1 AND sha256 IS NOT NULL ORDER BY path_key",
        )?;
        let rows = stmt.query_map([installation_id.to_string()], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (key, size, mtime, sha) = row?;
            out.push((
                key,
                to_u64("size", size)?,
                mtime,
                Sha256::parse(&sha).map_err(|_| corrupt("sha256", &sha))?,
            ));
        }
        Ok(out)
    }
}
