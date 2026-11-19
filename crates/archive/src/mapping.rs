//! Proposes how staged archive contents map onto the game folder.

use crate::stage::StagedEntry;
use gmm_domain::RelPath;
use std::path::PathBuf;

const KNOWN_FOLDERS: [&str; 5] = ["scripts", "plugins", "mods", "update", "x64"];

fn is_marker(rel: &RelPath) -> bool {
    let name = rel.file_name().to_lowercase();
    name == "scripthookv.dll"
        || name == "dinput8.dll"
        || rel.extension_lower().as_deref() == Some("asi")
}

/// Suggests the folder of the archive that corresponds to the game root.
///
/// Marker files (`ScriptHookV.dll`, `dinput8.dll`, `*.asi`) win: the root is the closest
/// common parent of all markers. Otherwise single wrapper folders that are not known game
/// folders are peeled off repeatedly. `None` means the archive root is the game root.
pub fn suggest_root(entries: &[StagedEntry]) -> Option<RelPath> {
    if entries.is_empty() {
        return None;
    }
    let markers: Vec<Vec<&str>> = entries
        .iter()
        .filter(|e| is_marker(&e.rel_path))
        .map(|e| {
            let mut c: Vec<&str> = e.rel_path.components().collect();
            c.pop();
            c
        })
        .collect();
    if let Some((first, rest)) = markers.split_first() {
        let mut common = first.len();
        for other in rest {
            common = first
                .iter()
                .zip(other)
                .take(common)
                .take_while(|(a, b)| a.to_lowercase() == b.to_lowercase())
                .count();
        }
        let mut end = common;
        if let Some(i) = first[..common]
            .iter()
            .position(|c| KNOWN_FOLDERS.contains(&c.to_lowercase().as_str()))
        {
            end = i;
        }
        return RelPath::parse(&first[..end].join("/")).ok();
    }
    let first: Vec<&str> = entries[0].rel_path.components().collect();
    let mut depth = 0usize;
    while let Some(&candidate) = first.get(depth) {
        let lower = candidate.to_lowercase();
        let wraps_all = !KNOWN_FOLDERS.contains(&lower.as_str())
            && entries.iter().all(|e| {
                let mut c = e.rel_path.components().skip(depth);
                let head_matches = c.next().is_some_and(|h| h.to_lowercase() == lower);
                head_matches && c.next().is_some()
            });
        if !wraps_all {
            break;
        }
        depth += 1;
    }
    if depth == 0 {
        return None;
    }
    RelPath::parse(&first[..depth].join("/")).ok()
}

/// Maps staged entries to game-relative targets, stripping `root`. Entries outside `root`
/// are omitted. With `root == None` every entry maps to its own path.
pub fn map_to_game(entries: &[StagedEntry], root: Option<&RelPath>) -> Vec<(RelPath, PathBuf)> {
    entries
        .iter()
        .filter_map(|e| match root {
            None => Some((e.rel_path.clone(), e.staged_path.clone())),
            Some(r) => {
                if !e.rel_path.is_within(r) {
                    return None;
                }
                let skip = r.components().count();
                let rest: Vec<&str> = e.rel_path.components().skip(skip).collect();
                RelPath::parse(&rest.join("/"))
                    .ok()
                    .map(|t| (t, e.staged_path.clone()))
            }
        })
        .collect()
}
