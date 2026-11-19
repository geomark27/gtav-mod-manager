//! Execution context and path safety helpers (invariants I-01 and I-05).

use std::path::{Path, PathBuf};

use gmm_domain::{InstallationId, RelPath};
use gmm_platform::{EntryKind, FileMeta, Fs};
use gmm_store::Database;

use crate::error::{Error, Result};
use crate::vault::Vault;

/// Everything an operation needs. Built per installation by the caller.
pub struct EngineContext<'a> {
    /// File system used for every effect (real, or fault-injecting in tests).
    pub fs: &'a dyn Fs,
    /// Application database (journal, mods, vault registry).
    pub db: &'a Database,
    /// The vault.
    pub vault: &'a Vault,
    /// Root directory of the game installation.
    pub root: &'a Path,
    /// The installation operated on.
    pub installation_id: InstallationId,
    /// True for paths the scanner catalog lists as original game files; the engine refuses them.
    pub is_protected: &'a dyn Fn(&RelPath) -> bool,
}

impl EngineContext<'_> {
    /// Refuses protected paths (defense in depth for I-01).
    pub(crate) fn ensure_not_protected(&self, rel: &RelPath) -> Result<()> {
        if (self.is_protected)(rel) {
            Err(Error::OwnershipUncertain { path: rel.clone() })
        } else {
            Ok(())
        }
    }

    /// Resolves `rel` under the root, see [`resolve`].
    pub(crate) fn resolve(&self, rel: &RelPath) -> Result<PathBuf> {
        resolve(self.fs, self.root, rel)
    }
}

/// Joins `rel` to `root` component by component (I-05).
///
/// Every ancestor that exists must be a real directory (not a reparse point or symlink), else
/// `PathOutsideRoot`. The last component is not inspected here, see [`inspect_file`].
pub fn resolve(fs: &dyn Fs, root: &Path, rel: &RelPath) -> Result<PathBuf> {
    let comps: Vec<&str> = rel.components().collect();
    let mut cur = root.to_path_buf();
    let mut exists = true;
    for (i, comp) in comps.iter().enumerate() {
        cur.push(comp);
        if i + 1 == comps.len() || !exists {
            continue;
        }
        if !fs.exists(&cur)? {
            exists = false;
            continue;
        }
        let meta = fs.metadata(&cur)?;
        if meta.is_reparse_point || meta.kind != EntryKind::Dir {
            return Err(Error::PathOutsideRoot { path: rel.clone() });
        }
    }
    Ok(cur)
}

/// Inspects an existing target: `None` when absent; otherwise it must be a regular file that is
/// not a reparse point and has a single hard link, else `OwnershipUncertain`.
pub(crate) fn inspect_file(fs: &dyn Fs, path: &Path, rel: &RelPath) -> Result<Option<FileMeta>> {
    if !fs.exists(path)? {
        return Ok(None);
    }
    let meta = fs.metadata(path)?;
    if meta.kind != EntryKind::File || meta.is_reparse_point || meta.hard_links != Some(1) {
        return Err(Error::OwnershipUncertain { path: rel.clone() });
    }
    Ok(Some(meta))
}
