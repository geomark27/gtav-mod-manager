//! Scanning of an installation folder into a classified inventory (architecture 6.2).
//!
//! Safety rule: when in doubt a file is `Unknown`, never `Candidate`, and never `Managed`.

use crate::baseline::is_engine_name;
use crate::catalog::Catalog;
use crate::error::{Error, Result};
use crate::os::OsProbe;
use crate::walk::{WalkEntry, walk};
use chrono::Utc;
use gmm_domain::{
    BaselineTrust, Category, Classification, Evidence, InstallationId, InventoryItem, ModId,
    ScanId, Sha256,
};
use gmm_platform::{Fs, sha256_reader};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

/// How often (in files) the progress callback fires when nothing is being hashed.
const PROGRESS_EVERY_FILES: u64 = 256;

/// A file that belongs to a confirmed mod.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedRef {
    /// Owning mod.
    pub mod_id: ModId,
    /// Expected content hash (vault copy).
    pub sha256: Sha256,
    /// Expected size.
    pub size: u64,
}

/// One baseline entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineRef {
    /// Size at capture time.
    pub size: u64,
    /// Hash at capture time.
    pub sha256: Sha256,
}

/// A previously computed hash, valid while size and mtime are unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedHash {
    /// Size when hashed.
    pub size: u64,
    /// Modification time when hashed, nanoseconds since the Unix epoch.
    pub mtime_ns: i64,
    /// The hash.
    pub sha256: Sha256,
}

/// Everything the scan needs to know about the installation. Keys are `RelPath::key()`.
#[derive(Debug, Clone)]
pub struct ScanContext {
    /// Files of confirmed mods.
    pub managed: HashMap<String, ManagedRef>,
    /// Baseline entries.
    pub baseline: HashMap<String, BaselineRef>,
    /// Whether the baseline proves originality.
    pub baseline_trust: BaselineTrust,
    /// Hash cache.
    pub hash_cache: HashMap<String, CachedHash>,
}

impl Default for ScanContext {
    fn default() -> Self {
        Self {
            managed: HashMap::new(),
            baseline: HashMap::new(),
            baseline_trust: BaselineTrust::None,
            hash_cache: HashMap::new(),
        }
    }
}

/// Progress snapshot handed to the callback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanProgress {
    /// Files visited so far.
    pub files_seen: u64,
    /// Bytes hashed so far.
    pub bytes_hashed: u64,
    /// File being processed.
    pub current: Option<String>,
}

/// Counters of a finished scan.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanStats {
    /// Files (items) found.
    pub files: u64,
    /// Bytes hashed (cache hits cost nothing).
    pub bytes_hashed: u64,
    /// Entries skipped because their name is not a valid `RelPath`.
    pub skipped_unparsable: u64,
    /// Items per classification (`Classification::as_str`).
    pub by_class: BTreeMap<String, u64>,
}

/// Result of [`scan`].
#[derive(Debug, Clone)]
pub struct ScanOutcome {
    /// One item per file, in deterministic order.
    pub items: Vec<InventoryItem>,
    /// Counters.
    pub stats: ScanStats,
}

impl ScanOutcome {
    /// Stamps every item with its installation and scan ids ([`scan`] does not know them and
    /// leaves a nil installation id and no scan id).
    #[must_use]
    pub fn with_owner(mut self, installation_id: InstallationId, scan_id: ScanId) -> Self {
        for item in &mut self.items {
            item.installation_id = installation_id;
            item.last_scan_id = Some(scan_id);
        }
        self
    }
}

struct Decision {
    class: Classification,
    evidence: Vec<Evidence>,
    category: Option<Category>,
}

struct Scanner<'a> {
    fs: &'a dyn Fs,
    os: &'a dyn OsProbe,
    catalog: &'a Catalog,
    ctx: &'a ScanContext,
    progress: &'a mut dyn FnMut(&ScanProgress) -> bool,
    state: ScanProgress,
}

impl Scanner<'_> {
    fn notify(&mut self) -> Result<()> {
        if (self.progress)(&self.state) {
            Ok(())
        } else {
            Err(Error::Cancelled)
        }
    }

    fn cached(&self, e: &WalkEntry) -> Option<Sha256> {
        if e.meta.is_reparse_point {
            return None;
        }
        self.ctx
            .hash_cache
            .get(&e.rel.key())
            .filter(|c| c.size == e.meta.len && c.mtime_ns == e.meta.modified_ns)
            .map(|c| c.sha256.clone())
    }

    /// Cache-aware hash of a regular file; fills `slot`.
    fn hash(&mut self, e: &WalkEntry, slot: &mut Option<Sha256>) -> Result<Sha256> {
        if let Some(h) = slot {
            return Ok(h.clone());
        }
        let base = self.state.bytes_hashed;
        self.state.current = Some(e.rel.as_str().to_owned());
        let mut reader = self.fs.open_read(&e.abs)?;
        let mut last = 0u64;
        let state = &mut self.state;
        let progress = &mut *self.progress;
        let hex = sha256_reader(&mut reader, &mut |n| {
            last = n;
            state.bytes_hashed = base + n;
            progress(state)
        })?;
        state.bytes_hashed = base + last;
        self.notify()?;
        let h = Sha256::parse(&hex)?;
        *slot = Some(h.clone());
        Ok(h)
    }

    fn baseline_matches(
        &mut self,
        e: &WalkEntry,
        slot: &mut Option<Sha256>,
        size: u64,
        expected: &Sha256,
    ) -> Result<bool> {
        if e.meta.is_reparse_point || e.meta.len != size {
            return Ok(false);
        }
        Ok(self.hash(e, slot)? == *expected)
    }

    fn decide(&mut self, e: &WalkEntry, slot: &mut Option<Sha256>) -> Result<Decision> {
        let key = e.rel.key();
        let reparse = e.meta.is_reparse_point;
        let plain = |class, evidence| Decision {
            class,
            evidence,
            category: None,
        };

        // 1. Engine artifacts.
        if is_engine_name(e.rel.file_name()) {
            return Ok(plain(Classification::EngineArtifact, Vec::new()));
        }

        // 2. Managed.
        if let Some(m) = self.ctx.managed.get(&key) {
            let by = Evidence::ManagedBy { mod_id: m.mod_id };
            let intact = !reparse && e.meta.len == m.size && self.hash(e, slot)? == m.sha256;
            let class = if intact {
                Classification::Managed
            } else {
                Classification::Drifted
            };
            return Ok(plain(class, vec![by]));
        }

        // 3. Catalog originals and baseline.
        let trusted = self.ctx.baseline_trust == BaselineTrust::Trusted;
        let in_catalog = self.catalog.is_original(&e.rel);
        let ctx = self.ctx;
        let base = ctx.baseline.get(&key);
        if let (true, Some(b)) = (trusted, base) {
            let same = self.baseline_matches(e, slot, b.size, &b.sha256)?;
            let mut evidence = Vec::new();
            if in_catalog {
                evidence.push(Evidence::CatalogOriginal);
            }
            let (class, ev) = if same {
                (Classification::Original, Evidence::BaselineMatch)
            } else {
                (Classification::OriginalModified, Evidence::BaselineMismatch)
            };
            evidence.push(ev);
            return Ok(plain(class, evidence));
        }
        // An untrusted baseline is information only: it never changes the class.
        let mut info = Vec::new();
        if let Some(b) = base
            && self.baseline_matches(e, slot, b.size, &b.sha256)?
        {
            info.push(Evidence::BaselineMatch);
        }
        if in_catalog {
            let mut evidence = vec![Evidence::CatalogOriginal];
            evidence.extend(info);
            return Ok(plain(Classification::Original, evidence));
        }

        // 4. Trusted publisher signature.
        let is_pe = matches!(
            e.rel.extension_lower().as_deref(),
            Some("exe" | "dll" | "asi")
        );
        if is_pe
            && !reparse
            && let Some(publisher) = self.os.authenticode_publisher(&e.abs)
            && self.catalog.is_trusted_publisher(&publisher)
        {
            let mut evidence = vec![Evidence::SignedBy { publisher }];
            evidence.extend(info);
            return Ok(plain(Classification::Original, evidence));
        }

        // 5. Mod patterns (never for reparse points or hard-linked files).
        let hard_linked = e.meta.hard_links.is_some_and(|n| n > 1);
        if let Some(p) = self.catalog.mod_pattern(&e.rel) {
            let mut evidence = vec![Evidence::ModPattern {
                pattern: p.label.clone(),
            }];
            evidence.extend(info);
            if reparse || hard_linked {
                return Ok(plain(Classification::Unknown, evidence));
            }
            return Ok(Decision {
                class: Classification::Candidate,
                evidence,
                category: Some(p.category),
            });
        }

        // 6. Unknown.
        Ok(plain(Classification::Unknown, info))
    }

    fn item(&mut self, e: WalkEntry) -> Result<InventoryItem> {
        self.state.current = Some(e.rel.as_str().to_owned());
        let mut slot = self.cached(&e);
        let mut d = self.decide(&e, &mut slot)?;
        if e.meta.hard_links.is_some_and(|n| n > 1) {
            d.evidence.push(Evidence::HardLinked);
        }
        if e.meta.is_reparse_point {
            d.evidence.push(Evidence::ReparsePoint);
        }
        let now = Utc::now();
        Ok(InventoryItem {
            installation_id: InstallationId::from_uuid(uuid::Uuid::nil()),
            rel_path: e.rel,
            size: e.meta.len,
            mtime_ns: e.meta.modified_ns,
            sha256: slot,
            classification: d.class,
            suggested_category: d.category,
            evidence: d.evidence,
            first_seen_at: now,
            last_seen_at: now,
            last_scan_id: None,
        })
    }
}

/// Scans `root` and classifies every file.
///
/// `progress` is called after each hashed chunk, after each hashed file and at least every
/// 256 files; returning `false` aborts with [`Error::Cancelled`]. The folder is never
/// modified. Items carry a nil installation id; see [`ScanOutcome::with_owner`].
pub fn scan(
    fs: &dyn Fs,
    os: &dyn OsProbe,
    catalog: &Catalog,
    root: &Path,
    ctx: &ScanContext,
    progress: &mut dyn FnMut(&ScanProgress) -> bool,
) -> Result<ScanOutcome> {
    let mut scanner = Scanner {
        fs,
        os,
        catalog,
        ctx,
        progress,
        state: ScanProgress {
            files_seen: 0,
            bytes_hashed: 0,
            current: None,
        },
    };
    let mut items = Vec::new();
    let mut stats = ScanStats::default();
    let mut skipped = 0u64;
    walk(fs, root, &mut skipped, &mut |entry| {
        let item = scanner.item(entry)?;
        *stats
            .by_class
            .entry(item.classification.as_str().to_owned())
            .or_insert(0) += 1;
        items.push(item);
        scanner.state.files_seen += 1;
        if scanner
            .state
            .files_seen
            .is_multiple_of(PROGRESS_EVERY_FILES)
        {
            scanner.notify()?;
        }
        Ok(())
    })?;
    // The final notification only reports; cancelling a finished scan changes nothing.
    let _ = (scanner.progress)(&scanner.state);
    stats.files = items.len() as u64;
    stats.bytes_hashed = scanner.state.bytes_hashed;
    stats.skipped_unparsable = skipped;
    Ok(ScanOutcome { items, stats })
}

#[cfg(test)]
mod tests;
