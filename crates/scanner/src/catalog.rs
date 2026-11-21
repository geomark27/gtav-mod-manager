//! Per-edition catalogs embedded in the binary (GTA V Legacy and Enhanced, Red Dead Redemption 2).

use crate::error::{Error, Result};
use crate::glob::Glob;
use gmm_domain::{Category, Edition, RelPath};
use serde::Deserialize;
use std::str::FromStr;

const LEGACY_TOML: &str = include_str!("../catalog/legacy.toml");
const ENHANCED_TOML: &str = include_str!("../catalog/enhanced.toml");
const RDR2_TOML: &str = include_str!("../catalog/rdr2.toml");

#[derive(Deserialize)]
struct RawCatalog {
    edition: String,
    executable: String,
    required_files: Vec<String>,
    steam_app_id: u32,
    #[serde(default)]
    rockstar_registry_keys: Vec<String>,
    #[serde(default)]
    trusted_publishers: Vec<String>,
    #[serde(default)]
    originals: RawOriginals,
    #[serde(default)]
    mod_patterns: Vec<RawModPattern>,
}

#[derive(Deserialize, Default)]
struct RawOriginals {
    #[serde(default)]
    files: Vec<String>,
    #[serde(default)]
    globs: Vec<String>,
}

#[derive(Deserialize)]
struct RawModPattern {
    glob: String,
    category: String,
    label: String,
}

/// A catalog entry recognizing mod files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModPattern {
    /// Compiled pattern, anchored at the installation root.
    pub glob: Glob,
    /// Category suggested for matching files.
    pub category: Category,
    /// Human readable label, used as evidence.
    pub label: String,
}

/// Known facts about one edition. Values are ASSUMPTIONS verified on real installations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    /// Edition described.
    pub edition: Edition,
    /// Executable file name that identifies the edition (A-01).
    pub executable: String,
    /// Files that must exist (regular files) in a structurally valid installation (A-05).
    pub required_files: Vec<RelPath>,
    /// Steam application id (A-02).
    pub steam_app_id: u32,
    /// Registry keys under `HKLM` holding `InstallFolder` for Rockstar installs (A-04).
    pub rockstar_registry_keys: Vec<String>,
    /// Authenticode publishers whose signed PE files count as original (A-06).
    pub trusted_publishers: Vec<String>,
    /// Exact paths of original game files (A-05).
    pub original_files: Vec<RelPath>,
    /// Glob patterns of original game files (A-05).
    pub original_globs: Vec<Glob>,
    /// Patterns recognizing mods, first match wins.
    pub mod_patterns: Vec<ModPattern>,
}

impl Catalog {
    /// True when `path` is an original file of this edition (exact name or original glob).
    pub fn is_original(&self, path: &RelPath) -> bool {
        self.original_files.iter().any(|f| f == path)
            || self.original_globs.iter().any(|g| g.matches(path))
    }

    /// First mod pattern matching `path`.
    pub fn mod_pattern(&self, path: &RelPath) -> Option<&ModPattern> {
        self.mod_patterns.iter().find(|p| p.glob.matches(path))
    }

    /// True when `publisher` is on the trusted list (case-insensitive, trimmed).
    pub fn is_trusted_publisher(&self, publisher: &str) -> bool {
        let wanted = publisher.trim().to_lowercase();
        self.trusted_publishers
            .iter()
            .any(|p| p.trim().to_lowercase() == wanted)
    }

    /// Parses and validates one catalog document.
    pub fn parse(text: &str) -> Result<Self> {
        let raw: RawCatalog = toml::from_str(text).map_err(|e| Error::Catalog(e.to_string()))?;
        let edition = Edition::from_str(&raw.edition)?;
        if raw.executable != edition.executable_name() {
            return Err(Error::Catalog(format!(
                "executable {:?} does not match edition {}",
                raw.executable,
                edition.as_str()
            )));
        }
        let required_files = raw
            .required_files
            .iter()
            .map(|p| RelPath::parse(p).map_err(Error::from))
            .collect::<Result<Vec<_>>>()?;
        let exe_key = RelPath::parse(&raw.executable)?.key();
        if !required_files.iter().any(|p| p.key() == exe_key) {
            return Err(Error::Catalog(format!(
                "required_files of {} must include the executable",
                edition.as_str()
            )));
        }
        let original_files = raw
            .originals
            .files
            .iter()
            .map(|p| RelPath::parse(p).map_err(Error::from))
            .collect::<Result<Vec<_>>>()?;
        let original_globs = raw
            .originals
            .globs
            .iter()
            .map(|g| Glob::new(g))
            .collect::<Result<Vec<_>>>()?;
        let mod_patterns = raw
            .mod_patterns
            .iter()
            .map(|m| {
                Ok(ModPattern {
                    glob: Glob::new(&m.glob)?,
                    category: Category::from_str(&m.category)?,
                    label: m.label.clone(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let catalog = Self {
            edition,
            executable: raw.executable,
            required_files,
            steam_app_id: raw.steam_app_id,
            rockstar_registry_keys: raw.rockstar_registry_keys,
            trusted_publishers: raw.trusted_publishers,
            original_files,
            original_globs,
            mod_patterns,
        };
        for f in &catalog.original_files {
            if let Some(p) = catalog.mod_pattern(f) {
                return Err(Error::Catalog(format!(
                    "original file {f} also matches mod pattern {:?}",
                    p.label
                )));
            }
        }
        Ok(catalog)
    }
}

/// Catalogs of every supported edition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalogs {
    /// One catalog per edition, in [`Edition::ALL`] order.
    list: Vec<Catalog>,
}

impl Catalogs {
    /// Loads the embedded catalogs.
    pub fn load() -> Result<Self> {
        let list = [LEGACY_TOML, ENHANCED_TOML, RDR2_TOML]
            .into_iter()
            .map(Catalog::parse)
            .collect::<Result<Vec<_>>>()?;
        let editions: Vec<Edition> = list.iter().map(|c| c.edition).collect();
        if editions != Edition::ALL {
            return Err(Error::Catalog(format!(
                "embedded catalogs {editions:?} do not match the editions {:?}",
                Edition::ALL
            )));
        }
        Ok(Self { list })
    }

    /// Catalog of an edition. `load` guarantees one catalog per edition, in
    /// [`Edition::ALL`] order, so the index is always in range.
    pub fn get(&self, edition: Edition) -> &Catalog {
        let index = Edition::ALL
            .iter()
            .position(|e| *e == edition)
            .unwrap_or_default();
        &self.list[index]
    }

    /// Iterates over every catalog, in [`Edition::ALL`] order.
    pub fn iter(&self) -> impl Iterator<Item = &Catalog> {
        self.list.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gmm_domain::Game;

    #[test]
    fn loads_every_edition() {
        let c = Catalogs::load().unwrap();
        assert_eq!(c.get(Edition::Legacy).steam_app_id, 271590);
        assert_eq!(c.get(Edition::Enhanced).steam_app_id, 3240220);
        assert_eq!(c.get(Edition::Rdr2).steam_app_id, 1174180);
        let editions: Vec<Edition> = c.iter().map(|cat| cat.edition).collect();
        assert_eq!(editions, Edition::ALL);
        for edition in Edition::ALL {
            assert_eq!(c.get(edition).edition, edition);
        }
        for cat in c.iter() {
            assert_eq!(cat.executable, cat.edition.executable_name());
            let required = if cat.edition.game() == Game::Gtav {
                5
            } else {
                2
            };
            assert_eq!(cat.required_files.len(), required);
            assert_eq!(cat.rockstar_registry_keys.len(), 1);
            assert!(cat.rockstar_registry_keys[0].contains('\\'));
        }
        assert!(
            c.get(Edition::Enhanced).rockstar_registry_keys[0]
                .ends_with("Grand Theft Auto V Enhanced")
        );
        assert!(c.get(Edition::Rdr2).rockstar_registry_keys[0].ends_with("Red Dead Redemption 2"));
    }

    #[test]
    fn required_files_are_valid_and_include_executable() {
        let c = Catalogs::load().unwrap();
        for cat in c.iter() {
            assert!(
                cat.required_files
                    .iter()
                    .any(|p| p.as_str() == cat.executable)
            );
            if cat.edition.game() == Game::Gtav {
                assert!(
                    cat.required_files
                        .iter()
                        .any(|p| p.as_str() == "update/update.rpf")
                );
            }
        }
    }

    #[test]
    fn inconsistent_executable_is_rejected() {
        let bad = LEGACY_TOML.replace("executable = \"GTA5.exe\"", "executable = \"Other.exe\"");
        assert!(matches!(Catalog::parse(&bad), Err(Error::Catalog(_))));
    }

    #[test]
    fn unknown_edition_and_bad_paths_are_rejected() {
        let bad = LEGACY_TOML.replace("edition = \"legacy\"", "edition = \"gold\"");
        assert!(Catalog::parse(&bad).is_err());
        let bad = LEGACY_TOML.replace("\"common.rpf\"", "\"../common.rpf\"");
        assert!(Catalog::parse(&bad).is_err());
        assert!(Catalog::parse("not = [toml").is_err());
    }

    #[test]
    fn patterns_compile_and_publishers_present() {
        let c = Catalogs::load().unwrap();
        for cat in c.iter() {
            let patterns = if cat.edition.game() == Game::Gtav {
                17
            } else {
                11
            };
            assert_eq!(cat.mod_patterns.len(), patterns);
            assert_eq!(cat.trusted_publishers.len(), 2);
            assert!(!cat.original_files.is_empty() && !cat.original_globs.is_empty());
            assert!(cat.is_trusted_publisher("  rockstar games, inc. "));
            assert!(!cat.is_trusted_publisher("Evil Corp"));
        }
    }

    #[test]
    fn originals_never_match_mod_patterns_and_cover_required_files() {
        let c = Catalogs::load().unwrap();
        for cat in c.iter() {
            for f in &cat.original_files {
                assert!(cat.mod_pattern(f).is_none(), "{f}");
                assert!(cat.is_original(f));
            }
            for f in &cat.required_files {
                assert!(cat.is_original(f), "{f}");
            }
            for other in Edition::ALL.into_iter().filter(|e| *e != cat.edition) {
                let other_exe = RelPath::parse(other.executable_name()).unwrap();
                assert!(
                    !cat.is_original(&other_exe),
                    "{other_exe} in {}",
                    cat.edition
                );
            }
        }
    }

    #[test]
    fn original_matching_mod_pattern_is_rejected() {
        let bad = LEGACY_TOML.replace("\"steam_api64.dll\"", "\"trainer.asi\"");
        assert!(matches!(Catalog::parse(&bad), Err(Error::Catalog(_))));
    }

    #[test]
    fn bad_glob_or_category_is_rejected() {
        let bad = LEGACY_TOML.replace("\"scripts/**\"", "\"scripts//**\"");
        assert!(matches!(Catalog::parse(&bad), Err(Error::Glob { .. })));
        let bad = LEGACY_TOML.replace("category = \"graphics_textures\"", "category = \"nope\"");
        assert!(Catalog::parse(&bad).is_err());
    }

    #[test]
    fn executable_must_be_required() {
        let bad = LEGACY_TOML.replace("    \"GTA5.exe\",\n", "");
        assert!(matches!(Catalog::parse(&bad), Err(Error::Catalog(_))));
    }
}
