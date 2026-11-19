//! Drift detection: compares the database with the disk. Never modifies a file.

use std::path::Path;

use chrono::Utc;
use gmm_domain::{InstallationId, ModEntry, ModId, ModState, RelPath};
use gmm_platform::{Fs, sha256_file};
use gmm_store::Database;

use crate::context::{inspect_file, resolve};
use crate::error::Result;
use crate::vault::Vault;

/// Kind of divergence found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriftKind {
    /// An entry of an enabled mod is absent from the game folder.
    Missing,
    /// An entry exists but its content (or file type) is not what the engine placed.
    Modified,
    /// A vault object of a disabled mod is missing or corrupt (mod set to `Error`).
    VaultCorrupt,
}

/// One divergence between the database and the disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriftItem {
    /// Mod owning the entry.
    pub mod_id: ModId,
    /// Entry path.
    pub path: RelPath,
    /// What differs.
    pub kind: DriftKind,
}

fn check_entry(fs: &dyn Fs, root: &Path, e: &ModEntry) -> Result<Option<DriftKind>> {
    let Ok(path) = resolve(fs, root, &e.rel_path) else {
        return Ok(Some(DriftKind::Modified));
    };
    let meta = match inspect_file(fs, &path, &e.rel_path) {
        Ok(Some(meta)) => meta,
        Ok(None) => return Ok(Some(DriftKind::Missing)),
        Err(crate::Error::OwnershipUncertain { .. }) => return Ok(Some(DriftKind::Modified)),
        Err(other) => return Err(other),
    };
    if meta.len != e.size || sha256_file(fs, &path)? != e.sha256.as_str() {
        return Ok(Some(DriftKind::Modified));
    }
    Ok(None)
}

/// Reconciles every mod of the installation with the disk and returns the current drift.
///
/// `Enabled` mods with a missing/modified entry become `Drifted`; `Drifted` mods whose entries
/// all match again return to `Enabled`; `Disabled` mods with an unverifiable vault object
/// become `Error`.
pub fn reconcile(
    fs: &dyn Fs,
    db: &Database,
    vault: &Vault,
    root: &Path,
    installation_id: InstallationId,
) -> Result<Vec<DriftItem>> {
    let mut items = Vec::new();
    for m in db.list_mods(installation_id)? {
        let entries = db.list_mod_entries(m.id)?;
        match m.state {
            ModState::Enabled | ModState::Drifted => {
                let mut found = Vec::new();
                for e in &entries {
                    if let Some(kind) = check_entry(fs, root, e)? {
                        found.push(DriftItem {
                            mod_id: m.id,
                            path: e.rel_path.clone(),
                            kind,
                        });
                    }
                }
                let target = if found.is_empty() {
                    ModState::Enabled
                } else {
                    ModState::Drifted
                };
                if target != m.state {
                    db.update_mod_state(m.id, target, Utc::now())?;
                }
                items.extend(found);
            }
            ModState::Disabled => {
                let mut bad = false;
                for e in &entries {
                    if !vault.verify(fs, &e.sha256)? {
                        bad = true;
                        items.push(DriftItem {
                            mod_id: m.id,
                            path: e.rel_path.clone(),
                            kind: DriftKind::VaultCorrupt,
                        });
                    }
                }
                if bad {
                    db.update_mod_state(m.id, ModState::Error, Utc::now())?;
                }
            }
            ModState::Error => {}
        }
    }
    Ok(items)
}
