//! Import of staged files into the vault (no game-folder access).

use std::path::PathBuf;

use gmm_domain::{RelPath, Sha256, TransactionId};
use gmm_platform::{EntryKind, Fs, sha256_file};

use crate::error::{Error, Result};
use crate::vault::Vault;

/// Hashes every staged file and stores it in the vault.
///
/// `files` are `(target path inside the installation, staged source)` pairs. Each source must
/// be a regular file (not a link). Returns `(target, sha256, size)` per file, in order, ready
/// for `plan_register_imported`. The game folder is never touched. Objects stored before the
/// import is registered are harmless orphans if it is abandoned; vault garbage collection
/// removes them. `tx_hint` only names the temp files of the vault copy.
pub fn import_to_vault(
    fs: &dyn Fs,
    vault: &Vault,
    files: &[(RelPath, PathBuf)],
    tx_hint: TransactionId,
) -> Result<Vec<(RelPath, Sha256, u64)>> {
    let mut out = Vec::with_capacity(files.len());
    for (target, src) in files {
        if !fs.exists(src)? {
            return Err(Error::NotFound {
                what: format!("staged file for {target}"),
            });
        }
        let meta = fs.metadata(src)?;
        if meta.kind != EntryKind::File || meta.is_reparse_point {
            return Err(Error::InvalidInput {
                what: format!("the staged source of {target} is not a regular file"),
            });
        }
        let sha = Sha256::parse(&sha256_file(fs, src)?)?;
        vault.store_from(fs, src, &sha, tx_hint)?;
        out.push((target.clone(), sha, meta.len));
    }
    Ok(out)
}
