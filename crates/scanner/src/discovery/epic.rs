//! Epic Games Launcher discovery from `*.item` manifests.

use super::{DiscoveredInstallation, DiscoveryHints, read_text};
use crate::catalog::Catalogs;
use gmm_domain::DistributionPlatform;
use gmm_platform::{EntryKind, Fs};
use std::path::{Path, PathBuf};

fn has_gta_executable(fs: &dyn Fs, catalogs: &Catalogs, folder: &Path) -> bool {
    catalogs.iter().any(|c| {
        matches!(
            fs.metadata(&folder.join(&c.executable)),
            Ok(m) if m.kind == EntryKind::File
        )
    })
}

pub(super) fn discover(
    fs: &dyn Fs,
    catalogs: &Catalogs,
    hints: &DiscoveryHints,
) -> Vec<DiscoveredInstallation> {
    let mut found = Vec::new();
    for dir in &hints.epic_manifest_dirs {
        let entries = match fs.read_dir(dir) {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!(dir = %dir.display(), error = %e, "cannot read Epic manifest folder");
                continue;
            }
        };
        for entry in entries {
            let is_item = Path::new(&entry.file_name)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("item"));
            if !is_item {
                continue;
            }
            let Some(text) = read_text(fs, &entry.path) else {
                continue;
            };
            let json: serde_json::Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(path = %entry.path.display(), error = %e, "skipping malformed Epic manifest");
                    continue;
                }
            };
            let Some(loc) = json
                .get("InstallLocation")
                .and_then(serde_json::Value::as_str)
                .filter(|s| !s.trim().is_empty())
            else {
                continue;
            };
            let root = PathBuf::from(loc);
            if has_gta_executable(fs, catalogs, &root) {
                found.push(DiscoveredInstallation {
                    root,
                    platform: DistributionPlatform::Epic,
                    source: "epic:manifest".to_owned(),
                });
            }
        }
    }
    found
}
