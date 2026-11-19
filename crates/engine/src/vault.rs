//! Content-addressed vault: `objects/<sha[0..2]>/<sha256>`, written once and read-only.

use std::path::{Path, PathBuf};

use gmm_domain::{Sha256, TransactionId, tmp_name};
use gmm_platform::{EntryKind, Fs, sha256_file};

use crate::error::{Error, Result};

/// The vault of preserved mod files (one per application, shared by all installations).
#[derive(Debug, Clone)]
pub struct Vault {
    /// Vault root directory (outside every game folder).
    pub root: PathBuf,
}

impl Vault {
    /// Creates a handle; nothing is touched on disk until the first object is stored.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Path of the object with the given digest.
    pub fn object_path(&self, sha: &Sha256) -> PathBuf {
        let s = sha.as_str();
        self.root.join("objects").join(&s[..2]).join(s)
    }

    /// True when the object exists, is a regular file and its content hashes to `sha`.
    pub fn verify(&self, fs: &dyn Fs, sha: &Sha256) -> Result<bool> {
        let path = self.object_path(sha);
        if !fs.exists(&path)? {
            return Ok(false);
        }
        let meta = fs.metadata(&path)?;
        if meta.kind != EntryKind::File || meta.is_reparse_point {
            return Ok(false);
        }
        Ok(sha256_file(fs, &path)? == sha.as_str())
    }

    /// Copies `src` into the vault as the object `expected`.
    ///
    /// Returns `true` when the object was created, `false` when an identical verified object
    /// already existed. An existing object with wrong content is never overwritten.
    pub fn store_from(
        &self,
        fs: &dyn Fs,
        src: &Path,
        expected: &Sha256,
        tx: TransactionId,
    ) -> Result<bool> {
        let object = self.object_path(expected);
        if fs.exists(&object)? {
            return existing_object(self, fs, expected);
        }
        if let Some(shard) = object.parent()
            && !fs.exists(shard)?
        {
            fs.create_dir_all(shard)?;
        }
        let tmp = vault_tmp_path(self, expected, tx);
        fs.copy_new(src, &tmp)?;
        if sha256_file(fs, &tmp)? != expected.as_str() {
            fs.remove_file(&tmp)?;
            return Err(Error::HashMismatch {
                path: src.display().to_string(),
            });
        }
        match fs.rename_no_replace(&tmp, &object) {
            Ok(()) => {}
            Err(gmm_platform::Error::AlreadyExists { .. }) => {
                fs.remove_file(&tmp)?;
                return existing_object(self, fs, expected);
            }
            Err(e) => return Err(e.into()),
        }
        fs.set_readonly(&object, true)?;
        Ok(true)
    }
}

fn existing_object(vault: &Vault, fs: &dyn Fs, sha: &Sha256) -> Result<bool> {
    if vault.verify(fs, sha)? {
        Ok(false)
    } else {
        Err(Error::VaultCorrupt { sha: sha.clone() })
    }
}

/// Name of the temp file used while storing `sha` for transaction `tx` (same directory as the object).
pub(crate) fn vault_tmp_name(sha: &Sha256, tx: TransactionId) -> String {
    tmp_name(sha.as_str(), tx)
}

pub(crate) fn vault_tmp_path(vault: &Vault, sha: &Sha256, tx: TransactionId) -> PathBuf {
    vault
        .object_path(sha)
        .with_file_name(vault_tmp_name(sha, tx))
}
