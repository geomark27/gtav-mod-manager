//! Rockstar Games Launcher discovery from the registry.

use super::DiscoveredInstallation;
use crate::catalog::Catalogs;
use crate::os::{Hive, OsProbe};
use gmm_domain::DistributionPlatform;
use gmm_platform::Fs;
use std::path::PathBuf;

pub(super) fn discover(
    fs: &dyn Fs,
    os: &dyn OsProbe,
    catalogs: &Catalogs,
) -> Vec<DiscoveredInstallation> {
    let mut found = Vec::new();
    for catalog in catalogs.iter() {
        for key in &catalog.rockstar_registry_keys {
            let Some(folder) = os.registry_string(Hive::LocalMachine, key, "InstallFolder") else {
                continue;
            };
            if folder.trim().is_empty() {
                continue;
            }
            let root = PathBuf::from(folder);
            if matches!(fs.exists(&root), Ok(true)) {
                found.push(DiscoveredInstallation {
                    root,
                    platform: DistributionPlatform::Rockstar,
                    source: "rockstar:registry".to_owned(),
                });
            } else {
                tracing::warn!(key = %key, "Rockstar InstallFolder does not exist");
            }
        }
    }
    found
}
