//! Discovery of GTA V installations from Steam, Epic and Rockstar.

mod epic;
mod rockstar;
mod steam;
pub mod vdf;

use crate::catalog::Catalogs;
use crate::os::OsProbe;
use gmm_domain::{DistributionPlatform, root_key};
use gmm_platform::{EntryKind, Fs};
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Largest metadata file discovery is willing to read.
const MAX_TEXT_BYTES: u64 = 4 * 1024 * 1024;

/// A candidate installation folder found by discovery (not yet validated).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredInstallation {
    /// Root folder of the game.
    pub root: PathBuf,
    /// Store it was found through.
    pub platform: DistributionPlatform,
    /// Where it was found, for example `"steam:libraryfolders"`.
    pub source: String,
}

/// Extra places to look in, besides the registry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiscoveryHints {
    /// Steam client folders.
    pub steam_roots: Vec<PathBuf>,
    /// Epic launcher manifest folders (`*.item`).
    pub epic_manifest_dirs: Vec<PathBuf>,
}

impl DiscoveryHints {
    /// Default Windows locations.
    pub fn windows_defaults() -> Self {
        Self {
            steam_roots: vec![PathBuf::from(r"C:\Program Files (x86)\Steam")],
            epic_manifest_dirs: vec![PathBuf::from(
                r"C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests",
            )],
        }
    }
}

/// Reads a small UTF-8 text file through `Fs`; `None` when missing, too big or unreadable.
pub(crate) fn read_text(fs: &dyn Fs, path: &Path) -> Option<String> {
    let meta = fs.metadata(path).ok()?;
    if meta.kind != EntryKind::File || meta.len > MAX_TEXT_BYTES {
        return None;
    }
    let mut reader = fs.open_read(path).ok()?;
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take(MAX_TEXT_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    String::from_utf8(bytes).ok()
}

/// Finds candidate installations. Never fails as a whole: per-source problems are logged
/// and skipped. The result is deduplicated by [`root_key`] (first source wins: Steam, then
/// Epic, then Rockstar) and sorted by root.
pub fn discover(
    fs: &dyn Fs,
    os: &dyn OsProbe,
    catalogs: &Catalogs,
    hints: &DiscoveryHints,
) -> Vec<DiscoveredInstallation> {
    let mut all = Vec::new();
    all.extend(steam::discover(fs, os, catalogs, hints));
    all.extend(epic::discover(fs, catalogs, hints));
    all.extend(rockstar::discover(fs, os, catalogs));
    let mut seen = HashSet::new();
    all.retain(|d| seen.insert(root_key(&d.root)));
    all.sort_by(|a, b| a.root.cmp(&b.root));
    all
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::os::Hive;
    use crate::os::testing::FakeOs;
    use gmm_platform::StdFs;
    use std::fs as stdfs;

    fn catalogs() -> Catalogs {
        Catalogs::load().unwrap()
    }

    /// Builds `<base>/<lib>/steamapps/` with manifests for the given (app id, installdir).
    fn make_library(base: &Path, apps: &[(u32, &str)]) {
        let steamapps = base.join("steamapps");
        stdfs::create_dir_all(steamapps.join("common")).unwrap();
        for (id, dir) in apps {
            stdfs::write(
                steamapps.join(format!("appmanifest_{id}.acf")),
                format!(
                    "\"AppState\"\n{{\n\t\"appid\"\t\"{id}\"\n\t\"installdir\"\t\"{dir}\"\n}}\n"
                ),
            )
            .unwrap();
            stdfs::create_dir_all(steamapps.join("common").join(dir)).unwrap();
        }
    }

    fn write_library_folders(steam_root: &Path, libs: &[&Path]) {
        let mut text = String::from("\"libraryfolders\"\n{\n");
        for (i, l) in libs.iter().enumerate() {
            let p = l.to_string_lossy().replace('\\', "\\\\");
            text.push_str(&format!("\t\"{i}\"\n\t{{\n\t\t\"path\"\t\"{p}\"\n\t}}\n"));
        }
        text.push_str("}\n");
        stdfs::create_dir_all(steam_root.join("steamapps")).unwrap();
        stdfs::write(steam_root.join("steamapps/libraryfolders.vdf"), text).unwrap();
    }

    #[test]
    fn steam_finds_both_editions_in_two_libraries_via_registry() {
        let tmp = tempfile::TempDir::new().unwrap();
        let steam = tmp.path().join("Steam");
        let lib2 = tmp.path().join("Lib2");
        make_library(&steam, &[(271590, "Grand Theft Auto V"), (730, "CS")]);
        make_library(&lib2, &[(3240220, "GTA V Enhanced")]);
        write_library_folders(&steam, &[&steam, &lib2]);
        let os = FakeOs::new().registry(
            Hive::CurrentUser,
            "Software\\Valve\\Steam",
            "SteamPath",
            &steam.to_string_lossy(),
        );
        let found = discover(&StdFs::new(), &os, &catalogs(), &DiscoveryHints::default());
        assert_eq!(found.len(), 2);
        assert!(
            found
                .iter()
                .all(|d| d.platform == DistributionPlatform::Steam)
        );
        assert!(found.iter().all(|d| d.source == "steam:libraryfolders"));
        let roots: Vec<_> = found.iter().map(|d| d.root.clone()).collect();
        assert!(roots.contains(&steam.join("steamapps/common/Grand Theft Auto V")));
        assert!(roots.contains(&lib2.join("steamapps/common/GTA V Enhanced")));
        let mut sorted = roots.clone();
        sorted.sort();
        assert_eq!(roots, sorted);
    }

    #[test]
    fn steam_dedupes_registry_and_hint_roots() {
        let tmp = tempfile::TempDir::new().unwrap();
        let steam = tmp.path().join("Steam");
        make_library(&steam, &[(271590, "Grand Theft Auto V")]);
        write_library_folders(&steam, &[&steam]);
        let os = FakeOs::new().registry(
            Hive::CurrentUser,
            "Software\\Valve\\Steam",
            "SteamPath",
            &steam.to_string_lossy(),
        );
        let hints = DiscoveryHints {
            steam_roots: vec![steam.clone()],
            epic_manifest_dirs: vec![],
        };
        let found = discover(&StdFs::new(), &os, &catalogs(), &hints);
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn steam_skips_garbled_and_missing_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        let steam = tmp.path().join("Steam");
        let lib = tmp.path().join("Lib");
        make_library(&lib, &[(271590, "Grand Theft Auto V")]);
        // garbled manifest for Enhanced, installdir with traversal for nothing else
        stdfs::write(
            lib.join("steamapps/appmanifest_3240220.acf"),
            "\"AppState\" {",
        )
        .unwrap();
        write_library_folders(&steam, &[&tmp.path().join("NoSuchLib"), &lib]);
        let hints = DiscoveryHints {
            steam_roots: vec![steam, tmp.path().join("NoSteam")],
            epic_manifest_dirs: vec![],
        };
        let found = discover(&StdFs::new(), &FakeOs::new(), &catalogs(), &hints);
        assert_eq!(found.len(), 1);
        assert!(found[0].root.ends_with("Grand Theft Auto V"));
    }

    #[test]
    fn steam_garbled_library_folders_is_skipped() {
        let tmp = tempfile::TempDir::new().unwrap();
        let steam = tmp.path().join("Steam");
        stdfs::create_dir_all(steam.join("steamapps")).unwrap();
        stdfs::write(steam.join("steamapps/libraryfolders.vdf"), "\"x\" {{{").unwrap();
        let hints = DiscoveryHints {
            steam_roots: vec![steam],
            epic_manifest_dirs: vec![],
        };
        assert!(discover(&StdFs::new(), &FakeOs::new(), &catalogs(), &hints).is_empty());
    }

    #[test]
    fn steam_rejects_installdir_traversal() {
        let tmp = tempfile::TempDir::new().unwrap();
        let steam = tmp.path().join("Steam");
        stdfs::create_dir_all(steam.join("steamapps/common")).unwrap();
        stdfs::write(
            steam.join("steamapps/appmanifest_271590.acf"),
            "\"AppState\" { \"installdir\" \"..\" }",
        )
        .unwrap();
        write_library_folders(&steam, &[&steam]);
        let hints = DiscoveryHints {
            steam_roots: vec![steam],
            epic_manifest_dirs: vec![],
        };
        assert!(discover(&StdFs::new(), &FakeOs::new(), &catalogs(), &hints).is_empty());
    }

    #[test]
    fn epic_reads_item_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        let manifests = tmp.path().join("Manifests");
        stdfs::create_dir_all(&manifests).unwrap();
        let gta = tmp.path().join("Epic/GTAV");
        let other = tmp.path().join("Epic/Other");
        stdfs::create_dir_all(&gta).unwrap();
        stdfs::create_dir_all(&other).unwrap();
        stdfs::write(gta.join("GTA5.exe"), b"x").unwrap();
        stdfs::write(other.join("game.exe"), b"x").unwrap();
        let item = |loc: &Path| {
            format!(
                "{{\"DisplayName\":\"g\",\"InstallLocation\":{}}}",
                serde_json::to_string(&loc.to_string_lossy()).unwrap()
            )
        };
        stdfs::write(manifests.join("a.item"), item(&gta)).unwrap();
        stdfs::write(manifests.join("b.item"), item(&other)).unwrap();
        stdfs::write(manifests.join("c.item"), "{ not json").unwrap();
        stdfs::write(manifests.join("d.item"), "{\"InstallLocation\": 5}").unwrap();
        stdfs::write(manifests.join("e.txt"), item(&gta)).unwrap();
        let hints = DiscoveryHints {
            steam_roots: vec![],
            epic_manifest_dirs: vec![manifests, tmp.path().join("NoSuchDir")],
        };
        let found = discover(&StdFs::new(), &FakeOs::new(), &catalogs(), &hints);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].root, gta);
        assert_eq!(found[0].platform, DistributionPlatform::Epic);
        assert_eq!(found[0].source, "epic:manifest");
    }

    #[test]
    fn rockstar_uses_registry_and_requires_existing_folder() {
        let tmp = tempfile::TempDir::new().unwrap();
        let legacy = tmp.path().join("RS/GTAV");
        stdfs::create_dir_all(&legacy).unwrap();
        let c = catalogs();
        let lkey = &c.get(gmm_domain::Edition::Legacy).rockstar_registry_keys[0];
        let ekey = &c.get(gmm_domain::Edition::Enhanced).rockstar_registry_keys[0];
        let os = FakeOs::new()
            .registry(
                Hive::LocalMachine,
                lkey,
                "InstallFolder",
                &legacy.to_string_lossy(),
            )
            .registry(
                Hive::LocalMachine,
                ekey,
                "InstallFolder",
                &tmp.path().join("RS/Missing").to_string_lossy(),
            );
        let found = discover(&StdFs::new(), &os, &c, &DiscoveryHints::default());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].root, legacy);
        assert_eq!(found[0].platform, DistributionPlatform::Rockstar);
        assert_eq!(found[0].source, "rockstar:registry");
    }

    #[test]
    fn dedupes_across_sources_first_wins() {
        let tmp = tempfile::TempDir::new().unwrap();
        let gta = tmp.path().join("GTAV");
        stdfs::create_dir_all(&gta).unwrap();
        stdfs::write(gta.join("GTA5.exe"), b"x").unwrap();
        let manifests = tmp.path().join("M");
        stdfs::create_dir_all(&manifests).unwrap();
        stdfs::write(
            manifests.join("a.item"),
            format!(
                "{{\"InstallLocation\":{}}}",
                serde_json::to_string(&gta.to_string_lossy()).unwrap()
            ),
        )
        .unwrap();
        let c = catalogs();
        let key = &c.get(gmm_domain::Edition::Legacy).rockstar_registry_keys[0];
        let os = FakeOs::new().registry(
            Hive::LocalMachine,
            key,
            "InstallFolder",
            &gta.to_string_lossy().to_uppercase(),
        );
        let hints = DiscoveryHints {
            steam_roots: vec![],
            epic_manifest_dirs: vec![manifests],
        };
        let found = discover(&StdFs::new(), &os, &c, &hints);
        // The upper-cased path does not exist on a case-sensitive fs, so only Epic remains
        // there; on a case-insensitive fs both collapse into one. Either way: one entry.
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].platform, DistributionPlatform::Epic);
    }

    #[test]
    fn windows_defaults_are_set() {
        let h = DiscoveryHints::windows_defaults();
        assert_eq!(h.steam_roots.len(), 1);
        assert_eq!(h.epic_manifest_dirs.len(), 1);
    }
}
