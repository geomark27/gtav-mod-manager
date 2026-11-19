//! Extraction limits.

use serde::{Deserialize, Serialize};

/// Limits applied while staging an archive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limits {
    /// Maximum number of entries.
    pub max_entries: usize,
    /// Maximum total uncompressed bytes.
    pub max_total_bytes: u64,
    /// Maximum uncompressed bytes of a single entry.
    pub max_entry_bytes: u64,
    /// Maximum uncompressed/compressed ratio (checked for entries above 1 MiB).
    pub max_ratio: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_entries: 20_000,
            max_total_bytes: 8 * 1024 * 1024 * 1024,
            max_entry_bytes: 4 * 1024 * 1024 * 1024,
            max_ratio: 200,
        }
    }
}
