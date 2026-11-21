//! Grouping heuristics for candidate files no signature recognized (Phase 4 R2).
//!
//! Files are grouped under a few base folders (`""` = the game root, `scripts`, `plugins`,
//! `mods`, and `lml` for Red Dead Redemption 2's Lenny's Mod Loader):
//! - a file inside a first-level folder of the base belongs to that folder's group
//!   (`scripts/Foo/a.dll` → "Foo");
//! - a file directly in the base belongs to the group of its stem, which also takes a folder
//!   of the same name (`x.asi` + `x.ini` + `x/…` → "X").
//!
//! Every proposal has [`Confidence::Low`] and is meant to be edited before adoption.

use std::collections::BTreeMap;

use gmm_domain::{Category, RelPath};
use serde::Serialize;

use super::matcher::Confidence;
use crate::catalog::Catalog;

/// Base folders whose first-level entries are grouped. (`update/**` is original: never grouped.)
const BASES: [&str; 4] = ["scripts", "plugins", "mods", "lml"];

/// A proposed mod made of related unrecognized files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProposedGroup {
    /// Suggested name (title-cased folder or file stem).
    pub name: String,
    /// Most common category of the catalog mod patterns among its files.
    pub category: Category,
    /// Files, ordered.
    pub files: Vec<RelPath>,
    /// Always [`Confidence::Low`].
    pub confidence: Confidence,
}

/// Splits a path into `(base, rest)`; `base` is `""` (the game root) when no base folder applies.
fn split_base(path: &RelPath) -> (String, Vec<&str>) {
    let parts: Vec<&str> = path.components().collect();
    for base in BASES {
        let depth = base.split('/').count();
        let head = parts
            .iter()
            .take(depth)
            .map(|s| s.to_lowercase())
            .collect::<Vec<_>>();
        if parts.len() > depth && head.join("/") == base {
            return (base.to_owned(), parts[depth..].to_vec());
        }
    }
    (String::new(), parts)
}

/// File name without its last extension.
fn stem(name: &str) -> &str {
    match name.rfind('.') {
        Some(i) if i > 0 => &name[..i],
        _ => name,
    }
}

/// `"my_cool-mod"` → `"My Cool Mod"`; letters after the first of each word keep their case.
pub fn title_case(raw: &str) -> String {
    raw.split(['_', '-', ' '])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Maximum length of a suggested name, in characters (the mod name limit).
const MAX_NAME_CHARS: usize = 120;

/// Title-cased name, falling back to the raw text when nothing is left, at most 120 characters.
fn suggested_name(display: &str) -> String {
    let titled = title_case(display);
    let name = if titled.is_empty() {
        display.to_owned()
    } else {
        titled
    };
    name.chars().take(MAX_NAME_CHARS).collect()
}

/// Groups `files` (already unrecognized, never originals) into proposed mods, ordered by name.
pub fn group_candidates(catalog: &Catalog, files: &[RelPath]) -> Vec<ProposedGroup> {
    // key = (base, lowercased folder or stem) → (display name, named after a file stem, files)
    let mut groups: BTreeMap<(String, String), (String, bool, Vec<RelPath>)> = BTreeMap::new();
    for file in files.iter().filter(|f| !catalog.is_original(f)) {
        let (base, rest) = split_base(file);
        let (display, from_stem) = match rest.as_slice() {
            [only] => (stem(only), true),
            [folder, ..] => (*folder, false),
            [] => continue,
        };
        let entry = groups
            .entry((base, display.to_lowercase()))
            .or_insert_with(|| (display.to_owned(), from_stem, Vec::new()));
        // Prefer the spelling of a file stem over a folder name ("Foo.asi" over "foo/").
        if from_stem && !entry.1 {
            entry.0 = display.to_owned();
            entry.1 = true;
        }
        entry.2.push(file.clone());
    }
    let mut out: Vec<ProposedGroup> = groups
        .into_values()
        .map(|(display, _, mut files)| {
            files.sort();
            ProposedGroup {
                name: suggested_name(&display),
                category: common_category(catalog, &files),
                files,
                confidence: Confidence::Low,
            }
        })
        .collect();
    out.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.files.cmp(&b.files))
    });
    out
}

/// The category the catalog mod patterns suggest most often (ties: first in `Category` order),
/// `Uncategorized` when none applies.
fn common_category(catalog: &Catalog, files: &[RelPath]) -> Category {
    let mut counts: BTreeMap<usize, usize> = BTreeMap::new();
    for f in files {
        if let Some(p) = catalog.mod_pattern(f)
            && let Some(i) = Category::ALL.iter().position(|c| *c == p.category)
        {
            *counts.entry(i).or_default() += 1;
        }
    }
    counts
        .iter()
        .max_by(|a, b| a.1.cmp(b.1).then_with(|| b.0.cmp(a.0)))
        .and_then(|(i, _)| Category::ALL.get(*i).copied())
        .unwrap_or(Category::Uncategorized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalogs;
    use gmm_domain::Edition;

    fn group(files: &[&str]) -> Vec<ProposedGroup> {
        let catalogs = Catalogs::load().unwrap();
        let files: Vec<RelPath> = files.iter().map(|f| RelPath::parse(f).unwrap()).collect();
        group_candidates(catalogs.get(Edition::Legacy), &files)
    }

    fn summary(groups: &[ProposedGroup]) -> Vec<(String, Vec<String>)> {
        groups
            .iter()
            .map(|g| {
                (
                    g.name.clone(),
                    g.files.iter().map(|f| f.as_str().to_owned()).collect(),
                )
            })
            .collect()
    }

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| (*x).to_owned()).collect()
    }

    #[test]
    fn folders_under_bases_become_groups() {
        let g = group(&[
            "scripts/CoolMod/a.dll",
            "scripts/CoolMod/data/b.xml",
            "plugins/fast_cars/x.ini",
            "mods/mycar/dlc.rpf",
        ]);
        assert_eq!(
            summary(&g),
            [
                (
                    "CoolMod".to_owned(),
                    s(&["scripts/CoolMod/a.dll", "scripts/CoolMod/data/b.xml"])
                ),
                ("Fast Cars".to_owned(), s(&["plugins/fast_cars/x.ini"])),
                ("Mycar".to_owned(), s(&["mods/mycar/dlc.rpf"])),
            ]
        );
        assert!(g.iter().all(|x| x.confidence == Confidence::Low));
    }

    #[test]
    fn each_lennys_mod_loader_folder_is_one_group() {
        let catalogs = Catalogs::load().unwrap();
        let files: Vec<RelPath> = [
            "lml/BetterHorses/install.xml",
            "lml/BetterHorses/stream/horse.ymt",
            "lml/dark_nights/install.xml",
        ]
        .iter()
        .map(|f| RelPath::parse(f).unwrap())
        .collect();
        let g = group_candidates(catalogs.get(Edition::Rdr2), &files);
        assert_eq!(
            summary(&g),
            [
                (
                    "BetterHorses".to_owned(),
                    s(&[
                        "lml/BetterHorses/install.xml",
                        "lml/BetterHorses/stream/horse.ymt"
                    ])
                ),
                (
                    "Dark Nights".to_owned(),
                    s(&["lml/dark_nights/install.xml"])
                ),
            ]
        );
        assert!(g.iter().all(|x| x.category == Category::Uncategorized));
    }

    #[test]
    fn stem_pairs_and_same_named_folders_join() {
        let g = group(&[
            "trainer/config.xml",
            "Trainer.asi",
            "trainer.ini",
            "scripts/alpha.dll",
            "scripts/alpha.ini",
            "scripts/alpha/data.bin",
            "loose.asi",
        ]);
        assert_eq!(
            summary(&g),
            [
                (
                    "Alpha".to_owned(),
                    s(&[
                        "scripts/alpha.dll",
                        "scripts/alpha.ini",
                        "scripts/alpha/data.bin"
                    ])
                ),
                ("Loose".to_owned(), s(&["loose.asi"])),
                (
                    "Trainer".to_owned(),
                    s(&["Trainer.asi", "trainer.ini", "trainer/config.xml"])
                ),
            ]
        );
    }

    #[test]
    fn the_same_name_in_different_bases_stays_apart() {
        let g = group(&["foo.asi", "scripts/foo.dll"]);
        assert_eq!(g.len(), 2);
    }

    #[test]
    fn category_comes_from_the_catalog_patterns() {
        let g = group(&["Trainer.asi", "trainer.ini"]);
        assert_eq!(g[0].category, Category::ScriptsGameplay);
        let g = group(&["notes/readme.txt"]);
        assert_eq!(g[0].category, Category::Uncategorized);
    }

    #[test]
    fn originals_are_skipped() {
        assert!(group(&["GTA5.exe", "update/update.rpf"]).is_empty());
    }

    #[test]
    fn helpers() {
        assert_eq!(title_case("my_cool-mod"), "My Cool Mod");
        assert_eq!(title_case("CoolMod"), "CoolMod");
        assert_eq!(title_case("__"), "");
        assert_eq!(stem("a.b.asi"), "a.b");
        assert_eq!(stem(".hidden"), ".hidden");
        assert_eq!(stem("noext"), "noext");
        assert_eq!(suggested_name("__"), "__");
        assert_eq!(suggested_name(&"a".repeat(200)).chars().count(), 120);
    }
}
