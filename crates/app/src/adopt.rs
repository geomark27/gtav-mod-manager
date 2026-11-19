//! Adoption of existing game files as a mod: preview and execution.

use std::collections::{HashMap, HashSet};

use gmm_domain::{
    Category, Classification, Evidence, InstallationId, InventoryItem, ModId, RelPath,
};
use gmm_engine::plan_adopt;
use gmm_store::{Database, InventoryFilter};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::app::App;
use crate::error::{AppError, ErrorCode};
use crate::installations::{not_found, parse_id};
use crate::mods::{ModDto, check_mod_name, clean_notes, name_taken, read_mod_dto};
use crate::operation::{Operation, OperationKind, ProgressSink};

const MAX_SUGGESTED_NAME_CHARS: usize = 120;
const NAME_CONTAINERS: [&str; 3] = ["scripts", "plugins", "mods"];

/// A file that can be adopted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AdoptFileDto {
    /// Path relative to the installation root.
    pub rel_path: String,
    /// Size in bytes.
    #[ts(type = "number")]
    pub size: u64,
    /// Classification from the last scan (`candidate` or `unknown`).
    pub classification: Classification,
}

/// A selected file that cannot be adopted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BlockedFileDto {
    /// Path as selected.
    pub rel_path: String,
    /// Plain-English reason.
    pub reason: String,
}

/// What adopting a selection would do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AdoptPreviewDto {
    /// Files that would be adopted.
    pub files: Vec<AdoptFileDto>,
    /// Selected files that cannot be adopted.
    pub blocked: Vec<BlockedFileDto>,
    /// Total size of the adoptable files.
    #[ts(type = "number")]
    pub total_size: u64,
    /// True when at least one adoptable file is `unknown` (needs explicit confirmation).
    pub includes_unknown: bool,
    /// Suggested mod name.
    pub suggested_name: String,
    /// Suggested category.
    pub suggested_category: Category,
}

/// Request of `adopt_mod`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AdoptRequest {
    /// Installation the files belong to.
    pub installation_id: String,
    /// Selected files, relative to the installation root.
    pub rel_paths: Vec<String>,
    /// Name of the new mod.
    pub name: String,
    /// Category of the new mod.
    pub category: Category,
    /// Free-form notes.
    pub notes: Option<String>,
    /// The user confirmed that unrecognized files are theirs to adopt.
    pub confirm_unknown: bool,
}

/// Internal result of looking the selection up in the inventory.
struct Selection {
    files: Vec<(RelPath, InventoryItem)>,
    blocked: Vec<BlockedFileDto>,
}

fn block_reason(item: &InventoryItem) -> Option<&'static str> {
    use Classification as C;
    match item.classification {
        C::Original | C::OriginalModified => return Some("Part of the game"),
        C::Managed | C::Drifted => return Some("Already managed"),
        C::EngineArtifact => return Some("Left over from an interrupted operation"),
        C::Candidate | C::Unknown => {}
    }
    let has = |f: fn(&Evidence) -> bool| item.evidence.iter().any(f);
    if has(|e| matches!(e, Evidence::ReparsePoint)) {
        Some("Symbolic link or junction")
    } else if has(|e| matches!(e, Evidence::HardLinked)) {
        Some("Hard-linked file")
    } else if has(|e| matches!(e, Evidence::ManagedBy { .. })) {
        Some("Already managed")
    } else if has(Evidence::blocks_adoption) {
        Some("Part of the game")
    } else {
        None
    }
}

fn inventory_row(
    db: &Database,
    installation_id: InstallationId,
    rel: &RelPath,
) -> Result<Option<InventoryItem>, AppError> {
    let key = rel.key();
    let mut offset = 0;
    loop {
        let page = db.list_inventory(
            installation_id,
            &InventoryFilter {
                classifications: Vec::new(),
                search: Some(key.clone()),
                limit: 500,
                offset,
            },
        )?;
        if let Some(found) = page.items.iter().find(|i| i.rel_path.key() == key) {
            return Ok(Some(found.clone()));
        }
        offset += 500;
        if u64::from(offset) >= page.total {
            return Ok(None);
        }
    }
}

fn select(
    db: &Database,
    installation_id: InstallationId,
    rel_paths: &[String],
) -> Result<Selection, AppError> {
    let mut seen = HashSet::new();
    let mut files = Vec::new();
    let mut blocked = Vec::new();
    for raw in rel_paths {
        let blocked_as = |reason: &str| BlockedFileDto {
            rel_path: raw.clone(),
            reason: reason.to_owned(),
        };
        let Ok(rel) = RelPath::parse(raw) else {
            blocked.push(blocked_as("Not a valid path inside the game folder"));
            continue;
        };
        if !seen.insert(rel.key()) {
            continue;
        }
        let Some(item) = inventory_row(db, installation_id, &rel)? else {
            blocked.push(blocked_as("Not found in the last scan — scan again"));
            continue;
        };
        match block_reason(&item) {
            Some(reason) => blocked.push(blocked_as(reason)),
            None => files.push((rel, item)),
        }
    }
    Ok(Selection { files, blocked })
}

fn stem(file_name: &str) -> &str {
    match file_name.rfind('.') {
        Some(i) if i > 0 => &file_name[..i],
        _ => file_name,
    }
}

fn common_prefix_len(a: &str, b: &str) -> usize {
    a.chars()
        .zip(b.chars())
        .take_while(|(x, y)| x.eq_ignore_ascii_case(y))
        .count()
}

/// Suggests a mod name: the folder under `scripts/`, `plugins/` or `mods/` shared by every
/// file, else the common file stem (or a common prefix of at least 3 characters), else the
/// first file's stem.
fn suggest_name(files: &[RelPath]) -> String {
    let folder = |rel: &RelPath| -> Option<String> {
        let parts: Vec<&str> = rel.components().collect();
        (parts.len() >= 3 && NAME_CONTAINERS.contains(&parts[0].to_lowercase().as_str()))
            .then(|| parts[1].to_owned())
    };
    let folders: Vec<Option<String>> = files.iter().map(folder).collect();
    if let Some(Some(first)) = folders.first()
        && folders
            .iter()
            .all(|f| f.as_deref().is_some_and(|f| f.eq_ignore_ascii_case(first)))
    {
        return first.chars().take(MAX_SUGGESTED_NAME_CHARS).collect();
    }
    let stems: Vec<&str> = files.iter().map(|f| stem(f.file_name())).collect();
    let Some(first) = stems.first().copied() else {
        return "New mod".to_owned();
    };
    if stems.iter().all(|s| s.eq_ignore_ascii_case(first)) {
        return first.chars().take(MAX_SUGGESTED_NAME_CHARS).collect();
    }
    let prefix = stems.iter().skip(1).fold(first.chars().count(), |n, s| {
        n.min(common_prefix_len(first, s))
    });
    let candidate: String = first
        .chars()
        .take(prefix)
        .collect::<String>()
        .trim_end_matches(['_', '-', '.', ' '])
        .to_owned();
    if candidate.chars().count() >= 3 {
        candidate
    } else {
        first.chars().take(MAX_SUGGESTED_NAME_CHARS).collect()
    }
}

/// Most frequent suggested category (ties resolve in declaration order), else uncategorized.
fn suggest_category(items: &[&InventoryItem]) -> Category {
    most_common_category(items.iter().filter_map(|i| i.suggested_category))
}

/// Most frequent category of `cats` (ties resolve in declaration order), else uncategorized.
pub(crate) fn most_common_category(cats: impl Iterator<Item = Category>) -> Category {
    let mut counts: HashMap<Category, usize> = HashMap::new();
    for c in cats {
        *counts.entry(c).or_default() += 1;
    }
    Category::ALL
        .iter()
        .copied()
        .filter(|c| counts.contains_key(c))
        .max_by_key(|c| {
            (
                counts[c],
                std::cmp::Reverse(Category::ALL.iter().position(|x| x == c)),
            )
        })
        .unwrap_or(Category::Uncategorized)
}

fn preview_of(sel: &Selection) -> AdoptPreviewDto {
    let rels: Vec<RelPath> = sel.files.iter().map(|(r, _)| r.clone()).collect();
    let items: Vec<&InventoryItem> = sel.files.iter().map(|(_, i)| i).collect();
    AdoptPreviewDto {
        files: sel
            .files
            .iter()
            .map(|(rel, item)| AdoptFileDto {
                rel_path: rel.as_str().to_owned(),
                size: item.size,
                classification: item.classification,
            })
            .collect(),
        blocked: sel.blocked.clone(),
        total_size: items.iter().map(|i| i.size).sum(),
        includes_unknown: items
            .iter()
            .any(|i| i.classification == Classification::Unknown),
        suggested_name: suggest_name(&rels),
        suggested_category: suggest_category(&items),
    }
}

impl App {
    /// Shows what adopting the selected files would do, from the last scan's inventory.
    pub fn preview_adopt(
        &self,
        installation_id: &str,
        rel_paths: &[String],
    ) -> Result<AdoptPreviewDto, AppError> {
        let id = parse_id(installation_id)?;
        let db = self.lock_db()?;
        if db.get_installation(id)?.is_none() {
            return Err(not_found());
        }
        Ok(preview_of(&select(&db, id, rel_paths)?))
    }

    /// Adopts the selected files as a new enabled mod: the files are copied into the vault
    /// and stay in the game folder. Runs under the operation slot.
    pub fn adopt_mod(
        &self,
        req: AdoptRequest,
        sink: &dyn ProgressSink,
    ) -> Result<ModDto, AppError> {
        let id = parse_id(&req.installation_id)?;
        self.run_operation(OperationKind::Adopt, sink, |op| {
            self.adopt_inner(id, req, op)
        })
    }

    fn adopt_inner(
        &self,
        id: InstallationId,
        req: AdoptRequest,
        op: &Operation<'_>,
    ) -> Result<ModDto, AppError> {
        let name = check_mod_name(&req.name)?;
        let notes = clean_notes(req.notes);
        let inst = self.load_validated(id)?;
        let sel = {
            let db = self.lock_db()?;
            if db.list_mods(id)?.iter().any(|m| m.name == name) {
                return Err(name_taken());
            }
            select(&db, id, &req.rel_paths)?
        };
        if sel.files.is_empty() && sel.blocked.is_empty() {
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                "Select at least one file to adopt.",
            ));
        }
        if !sel.blocked.is_empty() {
            let details = sel
                .blocked
                .iter()
                .map(|b| format!("{}: {}", b.rel_path, b.reason))
                .collect::<Vec<_>>()
                .join("; ");
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                "Some of the selected files cannot be adopted.",
            )
            .with_hint("Remove them from the selection, or scan again if the folder changed.")
            .with_details(details));
        }
        let includes_unknown = sel
            .files
            .iter()
            .any(|(_, i)| i.classification == Classification::Unknown);
        if includes_unknown && !req.confirm_unknown {
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                "The selection includes files the app does not recognize.",
            )
            .with_hint("Confirm that these files are yours to adopt, or remove them."));
        }
        let rels: Vec<RelPath> = sel.files.into_iter().map(|(r, _)| r).collect();
        let mod_id = ModId::new();
        // Adoption copies files into the vault; it never writes to the game folder.
        let committed = self.commit_plan(op, &inst, false, |ctx| {
            plan_adopt(
                ctx,
                mod_id,
                &name,
                req.category,
                notes,
                &rels,
                includes_unknown,
            )
        })?;
        if committed.is_none() {
            return Err(AppError::new(
                ErrorCode::Internal,
                "The adoption had nothing to do.",
            ));
        }
        let db = self.lock_db()?;
        read_mod_dto(&db, mod_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rels(paths: &[&str]) -> Vec<RelPath> {
        paths.iter().map(|p| RelPath::parse(p).unwrap()).collect()
    }

    #[test]
    fn name_from_scripts_folder() {
        let r = rels(&["scripts/MyMod/a.dll", "scripts/mymod/data/b.ini"]);
        assert_eq!(suggest_name(&r), "MyMod");
        let r = rels(&["plugins/Cool/x.asi"]);
        assert_eq!(suggest_name(&r), "Cool");
    }

    #[test]
    fn name_from_common_stem_or_prefix() {
        assert_eq!(
            suggest_name(&rels(&["Trainer.asi", "Trainer.ini"])),
            "Trainer"
        );
        assert_eq!(
            suggest_name(&rels(&["Trainer.asi", "Trainer_cfg.xml"])),
            "Trainer"
        );
        assert_eq!(suggest_name(&rels(&["a.asi", "zzz.ini"])), "a");
        assert_eq!(
            suggest_name(&rels(&["scripts/x.dll", "scripts/Mod/y.dll"])),
            "x"
        );
        assert_eq!(suggest_name(&[]), "New mod");
    }

    #[test]
    fn stems() {
        assert_eq!(stem("a.b.asi"), "a.b");
        assert_eq!(stem(".hidden"), ".hidden");
        assert_eq!(stem("noext"), "noext");
    }
}
