//! Known-mod signatures embedded in the binary (`catalog/mods/*.toml`).
//!
//! A signature names a mod and describes its files with anchored, case-insensitive globs:
//! every `files` pattern must match at least one file, every `any_of` group must have one
//! matching pattern, and `optional` patterns claim extra files when present. Signature data
//! are ASSUMPTIONS until checked on real installations (A-08); `verified` records that.

use std::collections::HashSet;
use std::str::FromStr;

use gmm_domain::{Category, Edition};
use serde::Deserialize;

use crate::catalog::Catalogs;
use crate::error::{Error, Result};
use crate::glob::Glob;

/// The embedded signature files, `(file name, contents)`. A test checks that every file of
/// `catalog/mods/` is listed here.
pub(crate) const SIGNATURE_FILES: &[(&str, &str)] = &[
    sig!("asi-loader"),
    sig!("enb"),
    sig!("lemonui"),
    sig!("menyoo"),
    sig!("native-trainer"),
    sig!("nativeui"),
    sig!("openiv-asi"),
    sig!("openiv-mods-folder"),
    sig!("rampage-trainer"),
    sig!("reshade"),
    sig!("script-hook-v"),
    sig!("script-hook-v-dotnet"),
    sig!("simple-trainer"),
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSignature {
    id: String,
    name: String,
    category: String,
    editions: Vec<String>,
    #[serde(default)]
    requires: Vec<String>,
    #[serde(default)]
    verified: bool,
    #[serde(default)]
    files: Vec<String>,
    #[serde(default)]
    any_of: Vec<Vec<String>>,
    #[serde(default)]
    optional: Vec<String>,
}

/// One known mod.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    /// Stable identifier (`[a-z0-9-]`), equal to the file stem.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Suggested category.
    pub category: Category,
    /// Editions the mod supports.
    pub editions: Vec<Edition>,
    /// Ids of the signatures this mod needs.
    pub requires: Vec<String>,
    /// True once the file names were checked on a real installation (A-08).
    pub verified: bool,
    /// Patterns that must each match at least one file.
    pub files: Vec<Glob>,
    /// Groups of which at least one pattern must match.
    pub any_of: Vec<Vec<Glob>>,
    /// Patterns that claim extra files when present.
    pub optional: Vec<Glob>,
}

impl Signature {
    /// Number of required constraints (`files` patterns plus `any_of` groups).
    pub fn required_count(&self) -> usize {
        self.files.len() + self.any_of.len()
    }

    /// Number of `files` patterns that name exactly one path.
    pub fn literal_count(&self) -> usize {
        self.files.iter().filter(|g| g.is_literal()).count()
    }

    /// Every pattern of the signature, required and optional.
    pub fn all_patterns(&self) -> impl Iterator<Item = &Glob> {
        self.files
            .iter()
            .chain(self.any_of.iter().flatten())
            .chain(&self.optional)
    }

    /// Parses one signature document; `stem` is its file name without `.toml`.
    pub fn parse(stem: &str, text: &str) -> Result<Self> {
        let bad = |what: String| Error::Catalog(format!("signature {stem}: {what}"));
        let raw: RawSignature = toml::from_str(text).map_err(|e| bad(e.to_string()))?;
        if raw.id != stem {
            return Err(bad(format!("id {:?} must equal the file name", raw.id)));
        }
        if !is_valid_id(&raw.id) {
            return Err(bad(
                "id must be lowercase letters, digits and hyphens".into()
            ));
        }
        if raw.name.trim().is_empty() {
            return Err(bad("name is empty".into()));
        }
        if raw.editions.is_empty() {
            return Err(bad("editions is empty".into()));
        }
        if raw.files.is_empty() && raw.any_of.is_empty() {
            return Err(bad(
                "needs at least one files pattern or any_of group".into()
            ));
        }
        if raw.any_of.iter().any(Vec::is_empty) {
            return Err(bad("an any_of group is empty".into()));
        }
        if raw.requires.iter().any(|r| r == &raw.id) {
            return Err(bad("a signature cannot require itself".into()));
        }
        let globs = |list: &[String]| {
            list.iter()
                .map(|g| Glob::new(g))
                .collect::<Result<Vec<_>>>()
        };
        Ok(Self {
            category: Category::from_str(&raw.category)?,
            editions: raw
                .editions
                .iter()
                .map(|e| Edition::from_str(e).map_err(Error::from))
                .collect::<Result<_>>()?,
            files: globs(&raw.files)?,
            any_of: raw.any_of.iter().map(|g| globs(g)).collect::<Result<_>>()?,
            optional: globs(&raw.optional)?,
            id: raw.id,
            name: raw.name.trim().to_owned(),
            requires: raw.requires,
            verified: raw.verified,
        })
    }
}

fn is_valid_id(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('-')
        && !id.ends_with('-')
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Every known-mod signature, ordered by id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signatures {
    list: Vec<Signature>,
}

impl Signatures {
    /// Loads and validates the embedded signatures against both edition catalogs.
    pub fn load(catalogs: &Catalogs) -> Result<Self> {
        Self::from_documents(SIGNATURE_FILES, catalogs)
    }

    /// Parses `(file stem, contents)` documents. Ids must be unique, `requires` must name
    /// existing signatures, and no pattern may match an original file of either catalog
    /// (catalog originals always win, D-14).
    pub fn from_documents(docs: &[(&str, &str)], catalogs: &Catalogs) -> Result<Self> {
        let mut list = docs
            .iter()
            .map(|(stem, text)| Signature::parse(stem, text))
            .collect::<Result<Vec<_>>>()?;
        list.sort_by(|a, b| a.id.cmp(&b.id));
        let ids: HashSet<&str> = list.iter().map(|s| s.id.as_str()).collect();
        if ids.len() != list.len() {
            return Err(Error::Catalog("duplicate signature id".into()));
        }
        for sig in &list {
            if let Some(missing) = sig.requires.iter().find(|r| !ids.contains(r.as_str())) {
                return Err(Error::Catalog(format!(
                    "signature {} requires unknown signature {missing}",
                    sig.id
                )));
            }
            for catalog in catalogs.iter() {
                for original in &catalog.original_files {
                    if let Some(g) = sig.all_patterns().find(|g| g.matches(original)) {
                        return Err(Error::Catalog(format!(
                            "signature {} pattern {:?} matches original file {original}",
                            sig.id,
                            g.as_str()
                        )));
                    }
                }
            }
        }
        Ok(Self { list })
    }

    /// Every signature, ordered by id.
    pub fn iter(&self) -> impl Iterator<Item = &Signature> {
        self.list.iter()
    }

    /// The signature with this id.
    pub fn get(&self, id: &str) -> Option<&Signature> {
        self.list.iter().find(|s| s.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalogs() -> Catalogs {
        Catalogs::load().unwrap()
    }

    const OK: &str = r#"
        id = "x"
        name = " X Mod "
        category = "vehicles"
        editions = ["legacy"]
        files = ["x.asi"]
        any_of = [["a.dll", "b.dll"]]
        optional = ["x/**"]
    "#;

    #[test]
    fn embedded_signatures_load() {
        let s = Signatures::load(&catalogs()).unwrap();
        assert_eq!(s.iter().count(), SIGNATURE_FILES.len());
        let shv = s.get("script-hook-v").unwrap();
        assert_eq!(shv.name, "Script Hook V");
        assert_eq!(shv.literal_count(), 1);
        assert!(
            s.get("rampage-trainer")
                .unwrap()
                .requires
                .contains(&"script-hook-v-dotnet".into())
        );
        // Sorted by id.
        let ids: Vec<&str> = s.iter().map(|x| x.id.as_str()).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
    }

    #[test]
    fn every_signature_file_is_embedded() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("catalog/mods");
        let mut on_disk: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".toml"))
            .map(|n| n.trim_end_matches(".toml").to_owned())
            .collect();
        on_disk.sort();
        let embedded: Vec<String> = SIGNATURE_FILES
            .iter()
            .map(|(s, _)| (*s).to_owned())
            .collect();
        assert_eq!(on_disk, embedded);
    }

    #[test]
    fn parses_and_trims() {
        let s = Signature::parse("x", OK).unwrap();
        assert_eq!(s.name, "X Mod");
        assert_eq!(s.required_count(), 2);
        assert_eq!(s.all_patterns().count(), 4);
        assert!(!s.verified);
    }

    #[test]
    fn rejects_bad_documents() {
        let cases = [
            OK.replace("id = \"x\"", "id = \"y\""),
            OK.replace("name = \" X Mod \"", "name = \"  \""),
            OK.replace("editions = [\"legacy\"]", "editions = []"),
            OK.replace("editions = [\"legacy\"]", "editions = [\"gold\"]"),
            OK.replace("category = \"vehicles\"", "category = \"cars\""),
            OK.replace("files = [\"x.asi\"]", "files = []")
                .replace("any_of = [[\"a.dll\", \"b.dll\"]]", ""),
            OK.replace("any_of = [[\"a.dll\", \"b.dll\"]]", "any_of = [[]]"),
            OK.replace("x/**", "x//**"),
            format!("{OK}\nrequires = [\"x\"]"),
            format!("{OK}\nunknown_key = 1"),
        ];
        for doc in &cases {
            assert!(Signature::parse("x", doc).is_err(), "{doc}");
        }
        let bad_id = OK.replace("id = \"x\"", "id = \"X_1\"");
        assert!(Signature::parse("X_1", &bad_id).is_err());
    }

    #[test]
    fn rejects_unknown_requirements_duplicates_and_originals() {
        let c = catalogs();
        let req = format!("{OK}\nrequires = [\"nope\"]");
        assert!(Signatures::from_documents(&[("x", &req)], &c).is_err());
        assert!(Signatures::from_documents(&[("x", OK), ("x", OK)], &c).is_err());
        let original = OK.replace("optional = [\"x/**\"]", "optional = [\"*.exe\"]");
        assert!(matches!(
            Signatures::from_documents(&[("x", &original)], &c),
            Err(Error::Catalog(m)) if m.contains("original file")
        ));
        assert!(Signatures::from_documents(&[("x", OK)], &c).is_ok());
    }
}
