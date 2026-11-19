//! Managed entries: files that belong to mods, for the scanner's classification.

use gmm_domain::{InstallationId, ModId, RelPath, Sha256};

use crate::database::Database;
use crate::error::{Error, Result};

/// A file entry of a mod of an installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedEntryRow {
    /// Owning mod.
    pub mod_id: ModId,
    /// Path relative to the installation root.
    pub rel_path: RelPath,
    /// Expected content hash.
    pub sha256: Sha256,
    /// Expected size in bytes.
    pub size: u64,
}

fn corrupt(column: &'static str, value: &str) -> Error {
    Error::Corrupt {
        table: "mod_entries",
        column,
        value: value.to_owned(),
    }
}

impl Database {
    /// Returns every mod entry of an installation (empty until mods exist), ordered by path key.
    pub fn managed_entries(&self, installation_id: InstallationId) -> Result<Vec<ManagedEntryRow>> {
        let mut stmt = self.conn().prepare(
            "SELECT e.mod_id, e.rel_path, e.sha256, e.size FROM mod_entries e \
             JOIN mods m ON m.id = e.mod_id \
             WHERE m.installation_id = ?1 ORDER BY e.path_key, e.mod_id",
        )?;
        let rows = stmt.query_map([installation_id.to_string()], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (mod_id, rel, sha, size) = row?;
            out.push(ManagedEntryRow {
                mod_id: mod_id.parse().map_err(|_| corrupt("mod_id", &mod_id))?,
                rel_path: RelPath::parse(&rel).map_err(|_| corrupt("rel_path", &rel))?,
                sha256: Sha256::parse(&sha).map_err(|_| corrupt("sha256", &sha))?,
                size: u64::try_from(size).map_err(|_| corrupt("size", &size.to_string()))?,
            });
        }
        Ok(out)
    }
}
