//! Vault garbage collection: list and delete objects no mod references.
//!
//! Objects are only ever deleted when the database proves nothing references them. Files under
//! `objects/` without a registry row are reported, never deleted (I-08).

use std::path::{Path, PathBuf};

use gmm_domain::{Sha256, parse_artifact};
use gmm_platform::{EntryKind, Fs};
use gmm_store::Database;

use crate::error::{Error, Result};
use crate::vault::Vault;

/// What the vault holds that could be cleaned up.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GcReport {
    /// Registered objects no mod and no unresolved transaction references: `(sha256, size)`.
    /// Includes rows whose file is already gone (left by an interrupted collection).
    pub unreferenced: Vec<(Sha256, u64)>,
    /// Files under `objects/` that have no registry row (old temps, foreign files). Only
    /// reported.
    pub stray_files: Vec<PathBuf>,
}

/// Lists unreferenced vault objects and stray files.
pub fn vault_gc_candidates(fs: &dyn Fs, db: &Database, vault: &Vault) -> Result<GcReport> {
    let mut stray_files = Vec::new();
    let objects = vault.root.join("objects");
    if fs.exists(&objects)? {
        scan_dir(fs, db, &objects, &mut stray_files)?;
    }
    stray_files.sort();
    Ok(GcReport {
        unreferenced: db.unreferenced_vault_objects()?,
        stray_files,
    })
}

fn scan_dir(fs: &dyn Fs, db: &Database, dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs.read_dir(dir)? {
        let meta = fs.metadata(&entry.path)?;
        if meta.kind == EntryKind::Dir && !meta.is_reparse_point {
            scan_dir(fs, db, &entry.path, out)?;
            continue;
        }
        let registered = meta.kind == EntryKind::File
            && !meta.is_reparse_point
            && parse_artifact(&entry.file_name).is_none()
            && Sha256::parse(&entry.file_name)
                .ok()
                .is_some_and(|sha| db.get_vault_object(&sha).ok().flatten().is_some());
        if !registered {
            out.push(entry.path);
        }
    }
    Ok(())
}

/// Deletes the given unreferenced vault objects and returns the bytes freed.
///
/// Each object is re-checked inside this call: one that became referenced is refused with
/// `InvalidState` (earlier ones stay deleted). The file is made writable and removed first,
/// then the row is deleted, so an interruption leaves at worst a row without a file, which
/// [`vault_gc_candidates`] still reports and a second call finishes. Digests with no registry
/// row are skipped.
pub fn vault_gc(fs: &dyn Fs, db: &Database, vault: &Vault, shas: &[Sha256]) -> Result<u64> {
    let mut freed = 0u64;
    for sha in shas {
        let Some((size, _, _)) = db.get_vault_object(sha)? else {
            continue;
        };
        if !db.vault_object_unreferenced(sha)? {
            return Err(Error::InvalidState {
                what: format!("vault object {sha} is still referenced"),
            });
        }
        let path = vault.object_path(sha);
        if fs.exists(&path)? {
            let meta = fs.metadata(&path)?;
            if meta.kind != EntryKind::File || meta.is_reparse_point {
                return Err(Error::OwnershipUncertain {
                    path: gmm_domain::RelPath::parse(&format!(
                        "objects/{}/{}",
                        &sha.as_str()[..2],
                        sha.as_str()
                    ))?,
                });
            }
            if meta.readonly {
                fs.set_readonly(&path, false)?;
            }
            fs.remove_file(&path)?;
            freed += size;
        }
        db.delete_vault_object_row(sha)?;
    }
    Ok(freed)
}
