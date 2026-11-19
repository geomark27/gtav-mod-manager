//! Steam discovery: registry/hint roots, `libraryfolders.vdf`, `appmanifest_<id>.acf`.

use super::vdf::{self, Vdf};
use super::{DiscoveredInstallation, DiscoveryHints, read_text};
use crate::catalog::Catalogs;
use crate::os::{Hive, OsProbe};
use gmm_domain::{DistributionPlatform, root_key};
use gmm_platform::Fs;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const SOURCE: &str = "steam:libraryfolders";

fn parse_file(fs: &dyn Fs, path: &Path) -> Option<Vdf> {
    let text = read_text(fs, path)?;
    match vdf::parse(&text) {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "skipping garbled Steam file");
            None
        }
    }
}

fn is_plain_dir_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\', ':', '\0'])
}

pub(super) fn discover(
    fs: &dyn Fs,
    os: &dyn OsProbe,
    catalogs: &Catalogs,
    hints: &DiscoveryHints,
) -> Vec<DiscoveredInstallation> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(p) = os.registry_string(Hive::CurrentUser, r"Software\Valve\Steam", "SteamPath")
        && !p.trim().is_empty()
    {
        roots.push(PathBuf::from(p));
    }
    roots.extend(hints.steam_roots.iter().cloned());

    let mut seen_roots = HashSet::new();
    roots.retain(|r| seen_roots.insert(root_key(r)));

    let mut found = Vec::new();
    for root in roots {
        let vdf_path = root.join("steamapps").join("libraryfolders.vdf");
        let Some(doc) = parse_file(fs, &vdf_path) else {
            continue;
        };
        let Some(folders) = doc.get("libraryfolders") else {
            tracing::warn!(path = %vdf_path.display(), "libraryfolders.vdf has no libraryfolders block");
            continue;
        };
        for (_, entry) in folders.entries() {
            let Some(lib) = entry.get("path").and_then(Vdf::as_str) else {
                continue;
            };
            let lib = PathBuf::from(lib);
            for catalog in catalogs.iter() {
                let steamapps = lib.join("steamapps");
                let manifest = steamapps.join(format!("appmanifest_{}.acf", catalog.steam_app_id));
                let Some(acf) = parse_file(fs, &manifest) else {
                    continue;
                };
                let Some(dir) = acf
                    .get("AppState")
                    .and_then(|s| s.get("installdir"))
                    .and_then(Vdf::as_str)
                else {
                    continue;
                };
                if !is_plain_dir_name(dir) {
                    tracing::warn!(path = %manifest.display(), "ignoring unsafe installdir");
                    continue;
                }
                let game_root = steamapps.join("common").join(dir);
                if !matches!(fs.exists(&game_root), Ok(true)) {
                    tracing::warn!(path = %game_root.display(), "Steam install folder does not exist");
                    continue;
                }
                found.push(DiscoveredInstallation {
                    root: game_root,
                    platform: DistributionPlatform::Steam,
                    source: SOURCE.to_owned(),
                });
            }
        }
    }
    found
}
