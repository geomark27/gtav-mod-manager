//! Matching scanned files against the known-mod signatures (Phase 4 R1).
//!
//! Input: the candidate/unknown files of a scan. Output: named mods, the files several
//! signatures claim equally (ambiguous), and the files no signature claims. A file belongs to
//! at most one recognized mod: the most specific signature wins it; on a tie nobody does.
//! Original files of the catalog are never claimed (D-14).

use std::collections::{BTreeMap, BTreeSet};

use gmm_domain::{Category, Edition, RelPath};
use serde::Serialize;

use super::signature::{Signature, Signatures};
use crate::catalog::Catalog;

/// How sure a proposal is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    /// A signature with at least one exact required file name matched.
    High,
    /// A signature matched only through wildcard patterns.
    Medium,
    /// A grouping heuristic, not a signature.
    Low,
}

/// A mod recognized by a signature.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecognizedMod {
    /// Id of the signature.
    pub signature_id: String,
    /// Display name.
    pub name: String,
    /// Suggested category.
    pub category: Category,
    /// Files it claims, ordered.
    pub files: Vec<RelPath>,
    /// `High` or `Medium`.
    pub confidence: Confidence,
    /// Optional patterns that matched no file.
    pub missing_optional: Vec<String>,
    /// Ids of the signatures it needs.
    pub requires: Vec<String>,
    /// False when the signature does not list the installation's edition.
    pub edition_supported: bool,
    /// True once the signature's file names were checked on a real installation (A-08).
    pub verified: bool,
}

/// A file that two or more equally specific signatures claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AmbiguousFile {
    /// The file.
    pub path: RelPath,
    /// The competing signature ids, ordered.
    pub signature_ids: Vec<String>,
}

/// Result of [`recognize`].
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct Recognition {
    /// Recognized mods, ordered by name.
    pub mods: Vec<RecognizedMod>,
    /// Files claimed by equally specific signatures; they belong to none.
    pub ambiguous: Vec<AmbiguousFile>,
    /// Files no recognized mod claims (input for grouping), ordered.
    pub unrecognized: Vec<RelPath>,
}

/// Specificity of a signature: more required constraints, then more exact names, wins.
fn specificity(sig: &Signature) -> (usize, usize) {
    (sig.required_count(), sig.literal_count())
}

fn satisfied(sig: &Signature, files: &[&RelPath]) -> bool {
    sig.is_satisfied_by(files)
}

/// Matches `items` (candidate and unknown files of a scan) against the signatures.
pub fn recognize(
    signatures: &Signatures,
    catalog: &Catalog,
    edition: Edition,
    items: &[RelPath],
) -> Recognition {
    let available: BTreeSet<&RelPath> = items.iter().filter(|p| !catalog.is_original(p)).collect();
    let pool: Vec<&RelPath> = available.iter().copied().collect();

    // 1. Every signature whose required constraints are met, with every file it would claim.
    let mut claims: BTreeMap<&RelPath, Vec<&Signature>> = BTreeMap::new();
    let mut matched: Vec<&Signature> = Vec::new();
    // Signatures of another game are never proposed; another edition of the same game is
    // proposed with `edition_supported = false`.
    for sig in signatures.iter().filter(|s| s.is_for_game(edition.game())) {
        if !satisfied(sig, &pool) {
            continue;
        }
        matched.push(sig);
        for f in &pool {
            if sig.all_patterns().any(|g| g.matches(f)) {
                claims.entry(f).or_default().push(sig);
            }
        }
    }

    // 2. Each file goes to its most specific claimant; ties make it ambiguous.
    let mut won: BTreeMap<&str, Vec<&RelPath>> = BTreeMap::new();
    let mut ambiguous = Vec::new();
    for (file, sigs) in &claims {
        let best = sigs
            .iter()
            .map(|s| specificity(s))
            .max()
            .unwrap_or_default();
        let top: Vec<&&Signature> = sigs.iter().filter(|s| specificity(s) == best).collect();
        match top.as_slice() {
            [only] => won.entry(only.id.as_str()).or_default().push(file),
            _ => ambiguous.push(AmbiguousFile {
                path: (*file).clone(),
                signature_ids: top.iter().map(|s| s.id.clone()).collect(),
            }),
        }
    }

    // 3. A signature that lost a required file to another one is not recognized.
    let mut mods = Vec::new();
    let mut claimed: BTreeSet<&RelPath> = ambiguous_paths(&ambiguous, &available);
    for sig in matched {
        let Some(files) = won.get(sig.id.as_str()) else {
            continue;
        };
        if !satisfied(sig, files) {
            continue;
        }
        claimed.extend(files.iter().copied());
        mods.push(RecognizedMod {
            signature_id: sig.id.clone(),
            name: sig.name.clone(),
            category: sig.category,
            files: files.iter().map(|f| (*f).clone()).collect(),
            confidence: if sig.literal_count() > 0 {
                Confidence::High
            } else {
                Confidence::Medium
            },
            missing_optional: sig
                .optional
                .iter()
                .filter(|g| !files.iter().any(|f| g.matches(f)))
                .map(|g| g.as_str().to_owned())
                .collect(),
            requires: sig.requires.clone(),
            edition_supported: sig.editions.contains(&edition),
            verified: sig.verified,
        });
    }
    mods.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.signature_id.cmp(&b.signature_id))
    });
    Recognition {
        mods,
        ambiguous,
        unrecognized: available
            .into_iter()
            .filter(|f| !claimed.contains(f))
            .cloned()
            .collect(),
    }
}

fn ambiguous_paths<'a>(
    ambiguous: &[AmbiguousFile],
    available: &BTreeSet<&'a RelPath>,
) -> BTreeSet<&'a RelPath> {
    available
        .iter()
        .copied()
        .filter(|p| ambiguous.iter().any(|a| &a.path == *p))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalogs;

    fn rp(s: &str) -> RelPath {
        RelPath::parse(s).unwrap()
    }

    fn paths(list: &[&str]) -> Vec<RelPath> {
        list.iter().map(|s| rp(s)).collect()
    }

    fn run(edition: Edition, files: &[&str]) -> Recognition {
        let catalogs = Catalogs::load().unwrap();
        let sigs = Signatures::load(&catalogs).unwrap();
        recognize(&sigs, catalogs.get(edition), edition, &paths(files))
    }

    fn find<'a>(r: &'a Recognition, id: &str) -> &'a RecognizedMod {
        r.mods
            .iter()
            .find(|m| m.signature_id == id)
            .expect("signature not recognized")
    }

    fn names(m: &RecognizedMod) -> Vec<&str> {
        m.files.iter().map(RelPath::as_str).collect()
    }

    #[test]
    fn rampage_setup_is_split_into_named_mods() {
        let r = run(
            Edition::Enhanced,
            &[
                "ScriptHookV.dll",
                "dinput8.dll",
                "ScriptHookVDotNet.asi",
                "ScriptHookVDotNet2.dll",
                "ScriptHookVDotNet3.dll",
                "ScriptHookVDotNet.ini",
                "scripts/RampageTrainer.dll",
                "scripts/Rampage/settings.xml",
                "scripts/LemonUI.SHVDN3.dll",
                "scripts/other.dll",
            ],
        );
        assert_eq!(names(find(&r, "script-hook-v")), ["ScriptHookV.dll"]);
        assert_eq!(names(find(&r, "asi-loader")), ["dinput8.dll"]);
        assert_eq!(
            names(find(&r, "script-hook-v-dotnet")),
            [
                "ScriptHookVDotNet.asi",
                "ScriptHookVDotNet.ini",
                "ScriptHookVDotNet2.dll",
                "ScriptHookVDotNet3.dll"
            ]
        );
        let rampage = find(&r, "rampage-trainer");
        assert_eq!(
            names(rampage),
            ["scripts/Rampage/settings.xml", "scripts/RampageTrainer.dll"]
        );
        assert_eq!(rampage.requires, ["script-hook-v-dotnet"]);
        assert_eq!(rampage.confidence, Confidence::Medium, "wildcard only");
        assert!(rampage.edition_supported);
        assert_eq!(find(&r, "script-hook-v").confidence, Confidence::High);
        assert_eq!(names(find(&r, "lemonui")), ["scripts/LemonUI.SHVDN3.dll"]);
        assert_eq!(r.unrecognized, paths(&["scripts/other.dll"]));
        assert!(r.ambiguous.is_empty());
        // Ordered by name.
        let order: Vec<&str> = r.mods.iter().map(|m| m.name.as_str()).collect();
        let mut sorted = order.clone();
        sorted.sort_by_key(|n| n.to_lowercase());
        assert_eq!(order, sorted);
    }

    #[test]
    fn matching_is_case_insensitive_and_keeps_the_file_spelling() {
        let r = run(
            Edition::Legacy,
            &["scripthookv.DLL", "MENYOO.asi", "menyoostuff/a.xml"],
        );
        assert_eq!(names(find(&r, "script-hook-v")), ["scripthookv.DLL"]);
        assert_eq!(
            names(find(&r, "menyoo")),
            ["MENYOO.asi", "menyoostuff/a.xml"]
        );
    }

    #[test]
    fn required_files_must_all_be_present() {
        // Menyoo needs both its plugin and its folder.
        let r = run(Edition::Legacy, &["Menyoo.asi"]);
        assert!(r.mods.is_empty());
        assert_eq!(r.unrecognized, paths(&["Menyoo.asi"]));
        // ReShade needs ReShade.ini and one of its proxies.
        let r = run(Edition::Legacy, &["ReShade.ini"]);
        assert!(r.mods.is_empty());
        let r = run(
            Edition::Legacy,
            &["ReShade.ini", "dxgi.dll", "reshade-shaders/a.fx"],
        );
        let m = find(&r, "reshade");
        assert_eq!(
            names(m),
            ["dxgi.dll", "reshade-shaders/a.fx", "ReShade.ini"]
        );
        assert!(m.missing_optional.contains(&"ReShade.log".to_owned()));
    }

    #[test]
    fn the_more_specific_signature_wins_a_shared_file() {
        // ENB (2 exact names) and ReShade (1 exact name + a group) both want d3d11.dll.
        let r = run(
            Edition::Legacy,
            &["enblocal.ini", "d3d11.dll", "ReShade.ini"],
        );
        assert_eq!(names(find(&r, "enb")), ["d3d11.dll", "enblocal.ini"]);
        assert!(r.mods.iter().all(|m| m.signature_id != "reshade"));
        assert_eq!(r.unrecognized, paths(&["ReShade.ini"]));
    }

    #[test]
    fn equally_specific_claims_are_ambiguous() {
        let catalogs = Catalogs::load().unwrap();
        let a = "id = \"a\"\nname = \"A\"\ncategory = \"vehicles\"\neditions = [\"legacy\"]\nfiles = [\"a.asi\"]\noptional = [\"shared.ini\"]";
        let b = "id = \"b\"\nname = \"B\"\ncategory = \"vehicles\"\neditions = [\"legacy\"]\nfiles = [\"b.asi\"]\noptional = [\"shared.ini\"]";
        let sigs = Signatures::from_documents(&[("a", a), ("b", b)], &catalogs).unwrap();
        let r = recognize(
            &sigs,
            catalogs.get(Edition::Legacy),
            Edition::Legacy,
            &paths(&["a.asi", "b.asi", "shared.ini"]),
        );
        assert_eq!(r.mods.len(), 2);
        assert_eq!(
            r.ambiguous,
            [AmbiguousFile {
                path: rp("shared.ini"),
                signature_ids: vec!["a".into(), "b".into()],
            }]
        );
        assert!(
            r.unrecognized.is_empty(),
            "ambiguous files are not unrecognized"
        );
        assert!(r.mods.iter().all(|m| m.files.len() == 1));
    }

    #[test]
    fn original_files_are_never_claimed() {
        let r = run(
            Edition::Legacy,
            &["GTA5.exe", "update/update.rpf", "ScriptHookV.dll"],
        );
        assert_eq!(r.mods.len(), 1);
        assert!(
            r.unrecognized.is_empty(),
            "originals are dropped from the input"
        );
    }

    #[test]
    fn edition_support_is_reported_not_enforced() {
        let r = run(Edition::Enhanced, &["OpenIV.asi", "dinput8.dll"]);
        assert!(!find(&r, "openiv-asi").edition_supported);
        assert!(find(&r, "asi-loader").edition_supported);
        let r = run(Edition::Legacy, &["OpenIV.asi"]);
        assert!(find(&r, "openiv-asi").edition_supported);
    }

    #[test]
    fn signatures_of_another_game_are_never_proposed() {
        let r = run(
            Edition::Rdr2,
            &["dinput8.dll", "ScriptHookV.dll", "Menyoo.asi", "OpenIV.asi"],
        );
        assert!(r.mods.is_empty(), "{:?}", r.mods);
        assert!(r.ambiguous.is_empty());
    }

    #[test]
    fn openiv_mods_folder_is_one_item_with_medium_confidence() {
        let r = run(
            Edition::Legacy,
            &["mods/update/update.rpf", "mods/x64a.rpf", "OpenIV.asi"],
        );
        let m = find(&r, "openiv-mods-folder");
        assert_eq!(names(m), ["mods/update/update.rpf", "mods/x64a.rpf"]);
        assert_eq!(m.confidence, Confidence::Medium);
    }

    #[test]
    fn similar_names_do_not_match() {
        let r = run(
            Edition::Legacy,
            &[
                "ScriptHookV.dll.bak",
                "my_dinput8.dll",
                "scripts/NativeUI2.dll",
                "trainerv2.asi",
            ],
        );
        assert!(r.mods.is_empty(), "{r:#?}");
        assert_eq!(r.unrecognized.len(), 4);
    }

    #[test]
    fn empty_input() {
        assert_eq!(run(Edition::Legacy, &[]), Recognition::default());
    }
}
