//! Pass 1: validates every entry from metadata only, before anything is written.

use crate::error::{Error, Result};
use crate::limits::Limits;
use gmm_domain::RelPath;
use std::collections::HashMap;
use std::io::{Read, Seek};
use zip::ZipArchive;

const RATIO_MIN_SIZE: u64 = 1024 * 1024;
const S_IFMT: u32 = 0o170_000;
const S_IFREG: u32 = 0o100_000;
const S_IFDIR: u32 = 0o040_000;

/// A validated file entry.
pub(crate) struct Planned {
    pub index: usize,
    pub rel: RelPath,
    pub declared_size: u64,
}

/// Validates the archive and returns its file entries (directories are implicit).
pub(crate) fn plan<R: Read + Seek>(
    zip: &mut ZipArchive<R>,
    limits: &Limits,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<Planned>> {
    if zip.len() > limits.max_entries {
        return Err(Error::TooLarge {
            what: "entry count",
            limit: limits.max_entries as u64,
        });
    }
    let mut files: Vec<Planned> = Vec::new();
    let mut seen: HashMap<String, bool> = HashMap::new(); // key -> is_file
    let mut total: u64 = 0;
    for index in 0..zip.len() {
        if cancel() {
            return Err(Error::Cancelled);
        }
        // Raw access: metadata only, no decompression and no password needed.
        let entry = zip
            .by_index_raw(index)
            .map_err(|e| Error::Zip(e.to_string()))?;
        let name = entry
            .name()
            .map_err(|e| Error::Zip(e.to_string()))?
            .into_owned();
        let is_dir = entry.is_dir();
        let trimmed = if is_dir {
            name.trim_end_matches(['/', '\\'])
        } else {
            name.as_str()
        };
        let rel = RelPath::parse(trimmed).map_err(|_| Error::UnsafePath { name: name.clone() })?;
        if entry.encrypted() {
            return Err(Error::Encrypted { name });
        }
        if entry.is_symlink() {
            return Err(Error::UnsafeEntry {
                name,
                reason: "symbolic link",
            });
        }
        if let Some(mode) = entry.unix_mode() {
            let kind = mode & S_IFMT;
            let ok = if is_dir {
                kind == 0 || kind == S_IFDIR
            } else {
                kind == 0 || kind == S_IFREG
            };
            if !ok {
                return Err(Error::UnsafeEntry {
                    name,
                    reason: "special file type",
                });
            }
        }
        register(&mut seen, &rel, !is_dir, &name)?;
        if is_dir {
            continue;
        }
        let size = entry.size();
        if size > limits.max_entry_bytes {
            return Err(Error::TooLarge {
                what: "entry size",
                limit: limits.max_entry_bytes,
            });
        }
        total = total.saturating_add(size);
        if total > limits.max_total_bytes {
            return Err(Error::TooLarge {
                what: "total size",
                limit: limits.max_total_bytes,
            });
        }
        if size > RATIO_MIN_SIZE && size / entry.compressed_size().max(1) > limits.max_ratio {
            return Err(Error::SuspiciousCompression { name });
        }
        files.push(Planned {
            index,
            rel,
            declared_size: size,
        });
    }
    Ok(files)
}

/// Records the entry and all its ancestors; detects duplicates and file/folder conflicts.
fn register(
    seen: &mut HashMap<String, bool>,
    rel: &RelPath,
    is_file: bool,
    name: &str,
) -> Result<()> {
    let dup = || Error::DuplicateEntry {
        name: name.to_owned(),
    };
    let mut prefix = String::new();
    let comps: Vec<&str> = rel.components().collect();
    for (i, comp) in comps.iter().enumerate() {
        if i > 0 {
            prefix.push('/');
        }
        prefix.push_str(&comp.to_lowercase());
        let last = i + 1 == comps.len();
        match seen.get(&prefix) {
            // A file can neither be repeated nor used as a folder.
            Some(true) => return Err(dup()),
            // An existing folder: fine as ancestor or repeated folder entry, not as a file.
            Some(false) if last && is_file => return Err(dup()),
            Some(false) => {}
            None => {
                seen.insert(prefix.clone(), last && is_file);
            }
        }
    }
    Ok(())
}
