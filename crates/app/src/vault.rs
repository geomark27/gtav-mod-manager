//! Vault report and garbage collection of unreferenced objects.

use std::collections::HashSet;

use gmm_domain::Sha256;
use gmm_engine::{Vault, vault_gc, vault_gc_candidates};
use gmm_platform::StdFs;
use serde::Serialize;
use ts_rs::TS;

use crate::app::App;
use crate::error::{AppError, ErrorCode};
use crate::operation::{NullSink, OperationKind};

/// A vault object nothing references.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VaultObjectDto {
    /// Content digest (lowercase hex).
    pub sha256: String,
    /// Size in bytes.
    #[ts(type = "number")]
    pub size: u64,
}

/// What the vault holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VaultReportDto {
    /// Objects referenced by a mod plus the unreferenced ones.
    #[ts(type = "number")]
    pub object_count: u64,
    /// Total size of those objects in bytes.
    #[ts(type = "number")]
    pub total_bytes: u64,
    /// Objects no mod and no unfinished operation needs; safe to collect.
    pub unreferenced: Vec<VaultObjectDto>,
    /// Total size of the unreferenced objects.
    #[ts(type = "number")]
    pub unreferenced_bytes: u64,
    /// Files in the vault that are not registered objects (reported only, never deleted),
    /// relative to the vault folder.
    pub stray_files: Vec<String>,
}

impl App {
    /// Reports the vault contents: counts, unreferenced objects and stray files.
    pub fn vault_report(&self) -> Result<VaultReportDto, AppError> {
        let db = self.lock_db()?;
        let report = vault_gc_candidates(&StdFs::new(), &db, &Vault::new(&self.dirs.vault_dir))?;
        let mut seen: HashSet<String> = HashSet::new();
        let mut object_count = 0u64;
        let mut total_bytes = 0u64;
        for inst in db.list_installations()? {
            for (_mod, _state, entry) in db.list_entries_for_installation(inst.id)? {
                if seen.insert(entry.sha256.as_str().to_owned()) {
                    object_count += 1;
                    total_bytes += entry.size;
                }
            }
        }
        let mut unreferenced_bytes = 0u64;
        let unreferenced: Vec<VaultObjectDto> = report
            .unreferenced
            .iter()
            .map(|(sha, size)| {
                unreferenced_bytes += size;
                VaultObjectDto {
                    sha256: sha.as_str().to_owned(),
                    size: *size,
                }
            })
            .collect();
        for (sha, size) in &report.unreferenced {
            if seen.insert(sha.as_str().to_owned()) {
                object_count += 1;
                total_bytes += size;
            }
        }
        let stray_files = report
            .stray_files
            .iter()
            .map(|p| {
                p.strip_prefix(&self.dirs.vault_dir)
                    .unwrap_or(p)
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        Ok(VaultReportDto {
            object_count,
            total_bytes,
            unreferenced,
            unreferenced_bytes,
            stray_files,
        })
    }

    /// Deletes the given unreferenced vault objects and returns the bytes freed.
    ///
    /// Refused with `RECOVERY_REQUIRED` while any transaction anywhere is unresolved, and
    /// with `INVALID_INPUT` when an object is still referenced.
    pub fn vault_gc(&self, shas: &[String]) -> Result<u64, AppError> {
        let parsed: Vec<Sha256> = shas
            .iter()
            .map(|s| Sha256::parse(s))
            .collect::<Result<_, _>>()?;
        self.run_operation(OperationKind::Transaction, &NullSink, |_op| {
            let db = self.lock_db()?;
            let mut unresolved = !db.transactions_needing_recovery()?.is_empty();
            for inst in db.list_installations()? {
                unresolved |= db.installation_has_attention(inst.id)?;
            }
            if unresolved {
                return Err(AppError::new(
                    ErrorCode::RecoveryRequired,
                    "An earlier operation must be resolved before the vault can be cleaned.",
                )
                .with_hint("Restart the app so it can finish or undo the interrupted operation."));
            }
            let freed = vault_gc(
                &StdFs::new(),
                &db,
                &Vault::new(&self.dirs.vault_dir),
                &parsed,
            )?;
            tracing::info!(objects = parsed.len(), freed, "vault garbage collected");
            Ok(freed)
        })
    }
}
