//! Import of a mod from a ZIP archive: stage, preview, remap, confirm and discard.
//!
//! The archive is extracted into a private staging folder under the app data directory
//! (never into a game folder). Confirming copies the files into the vault and registers a
//! **disabled** mod; enabling it later is the normal toggle path.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{MutexGuard, PoisonError};

use gmm_archive::{Limits, StagedArchive, map_to_game, stage_zip, suggest_root};
use gmm_domain::{
    Category, Classification, Installation, InstallationId, ModId, ModState, RelPath,
    TransactionId, parse_artifact,
};
use gmm_engine::{import_to_vault, plan_register_imported, resolve};
use gmm_platform::{AppDirs, Fs, StdFs};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::adopt::most_common_category;
use crate::app::App;
use crate::error::{AppError, ErrorCode};
use crate::guards::GuardNeeds;
use crate::installations::parse_id;
use crate::mods::{ModDto, check_mod_name, clean_notes, name_taken, read_mod_dto};
use crate::operation::{OperationKind, ProgressSink};

const MAX_SUGGESTED_NAME_CHARS: usize = 120;
const STAGING_DIR: &str = "staging";
const STAGING_PREFIX: &str = "stage-";

/// An archive staged for import. Only one exists at a time.
#[derive(Debug, Clone)]
pub(crate) struct StagedImport {
    id: String,
    installation_id: InstallationId,
    archive_name: String,
    archive: StagedArchive,
}

/// A file of the archive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImportFileDto {
    /// Path inside the archive.
    pub archive_path: String,
    /// Size in bytes.
    #[ts(type = "number")]
    pub size: u64,
}

/// What currently sits at an import target path (from the last scan), or `absent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ImportTargetClass {
    /// The path is not in the inventory.
    Absent,
    /// Original game file.
    Original,
    /// Original game file whose content differs from the baseline.
    OriginalModified,
    /// Probably a mod file.
    Candidate,
    /// Not recognized.
    Unknown,
    /// Owned by a managed mod.
    Managed,
    /// Owned by a managed mod but changed outside the manager.
    Drifted,
    /// Left over from an interrupted operation.
    EngineArtifact,
}

impl From<Classification> for ImportTargetClass {
    fn from(c: Classification) -> Self {
        match c {
            Classification::Original => Self::Original,
            Classification::OriginalModified => Self::OriginalModified,
            Classification::Candidate => Self::Candidate,
            Classification::Unknown => Self::Unknown,
            Classification::Managed => Self::Managed,
            Classification::Drifted => Self::Drifted,
            Classification::EngineArtifact => Self::EngineArtifact,
        }
    }
}

/// An archive file mapped onto the game folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImportMappedDto {
    /// Path relative to the game folder.
    pub target_path: String,
    /// Size in bytes.
    #[ts(type = "number")]
    pub size: u64,
    /// What the last scan found at the target.
    pub classification: ImportTargetClass,
    /// The target exists in the game folder or belongs to an enabled mod.
    pub conflict: bool,
    /// The target is an original game file or an engine artifact; the import is refused.
    pub protected: bool,
}

/// Preview of an archive staged for import.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImportPreviewDto {
    /// Identifier to pass to `remap_import`, `confirm_import` and `discard_import`.
    pub staging_id: String,
    /// File name of the archive.
    pub archive_name: String,
    /// Every file of the archive.
    pub files: Vec<ImportFileDto>,
    /// Archive folder that corresponds to the game folder, as suggested.
    pub suggested_root: Option<String>,
    /// Archive folder used for `mapped` (`null` = the archive root).
    pub root: Option<String>,
    /// Files mapped onto the game folder using `root`.
    pub mapped: Vec<ImportMappedDto>,
    /// Suggested mod name (the archive file name without extension).
    pub suggested_name: String,
    /// Suggested category.
    pub suggested_category: Category,
}

/// Request of `confirm_import`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConfirmImportRequest {
    /// Staged import to confirm.
    pub staging_id: String,
    /// Archive folder that corresponds to the game folder (`null` = the archive root).
    pub root: Option<String>,
    /// Name of the new mod.
    pub name: String,
    /// Category of the new mod.
    pub category: Category,
    /// Free-form notes.
    pub notes: Option<String>,
    /// Enable the mod right after importing it.
    pub enable_now: bool,
}

fn staging_root(dirs: &AppDirs) -> PathBuf {
    dirs.data_dir.join(STAGING_DIR)
}

fn staging_not_found() -> AppError {
    AppError::new(ErrorCode::NotFound, "The staged import was not found.")
        .with_hint("Choose the archive again.")
}

/// Deletes one staging folder this app created. Anything outside the staging root, and
/// anything that is not a plain directory, is left alone.
fn remove_staging_dir(dirs: &AppDirs, dir: &Path) {
    if dir.parent() != Some(staging_root(dirs).as_path()) {
        tracing::warn!(dir = %dir.display(), "refusing to delete a folder outside the staging root");
        return;
    }
    match std::fs::symlink_metadata(dir) {
        Ok(meta) if meta.is_dir() => {
            if let Err(e) = std::fs::remove_dir_all(dir) {
                tracing::warn!(dir = %dir.display(), error = %e, "could not delete the staging folder");
            }
        }
        Ok(_) => tracing::warn!(dir = %dir.display(), "staging entry is not a folder; left alone"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            tracing::warn!(dir = %dir.display(), error = %e, "could not inspect the staging folder")
        }
    }
}

/// Deletes staging folders left by a previous run (they only hold copies made by the app).
pub(crate) fn remove_leftover_staging(dirs: &AppDirs) {
    let root = staging_root(dirs);
    let Ok(entries) = std::fs::read_dir(&root) else {
        return;
    };
    for entry in entries.flatten() {
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with(STAGING_PREFIX)
        {
            remove_staging_dir(dirs, &entry.path());
        }
    }
}

fn parse_root(root: Option<&str>) -> Result<Option<RelPath>, AppError> {
    match root.map(str::trim).filter(|r| !r.is_empty()) {
        None => Ok(None),
        Some(r) => RelPath::parse(r).map(Some).map_err(|e| {
            AppError::new(
                ErrorCode::InvalidInput,
                "The folder is not a valid path inside the archive.",
            )
            .with_details(e.to_string())
        }),
    }
}

fn suggested_name(archive_path: &Path) -> String {
    let stem = archive_path
        .file_stem()
        .map(|s| s.to_string_lossy().trim().to_owned())
        .unwrap_or_default();
    if stem.is_empty() {
        "New mod".to_owned()
    } else {
        stem.chars().take(MAX_SUGGESTED_NAME_CHARS).collect()
    }
}

impl App {
    fn staged_lock(&self) -> MutexGuard<'_, Option<StagedImport>> {
        self.staged.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Inventory class of each wanted path key, from the last scan.
    fn inventory_classes(
        &self,
        id: InstallationId,
        keys: &HashSet<String>,
    ) -> Result<HashMap<String, Classification>, AppError> {
        let db = self.lock_db()?;
        let mut found = HashMap::new();
        if keys.is_empty() {
            return Ok(found);
        }
        let mut offset = 0u32;
        loop {
            let page = db.list_inventory(
                id,
                &gmm_store::InventoryFilter {
                    classifications: Vec::new(),
                    search: None,
                    limit: gmm_store::MAX_PAGE_SIZE,
                    offset,
                },
            )?;
            for item in &page.items {
                let key = item.rel_path.key();
                if keys.contains(&key) {
                    found.insert(key, item.classification);
                }
            }
            offset = offset.saturating_add(gmm_store::MAX_PAGE_SIZE);
            if u64::from(offset) >= page.total || page.items.is_empty() {
                return Ok(found);
            }
        }
    }

    fn import_preview(
        &self,
        inst: &Installation,
        staged: &StagedImport,
        root: Option<&RelPath>,
    ) -> Result<ImportPreviewDto, AppError> {
        let mapped = map_to_game(&staged.archive.entries, root);
        let sizes: HashMap<&Path, u64> = staged
            .archive
            .entries
            .iter()
            .map(|e| (e.staged_path.as_path(), e.size))
            .collect();
        let keys: HashSet<String> = mapped.iter().map(|(t, _)| t.key()).collect();
        let classes = self.inventory_classes(inst.id, &keys)?;
        let owned: HashSet<String> = {
            let db = self.lock_db()?;
            db.list_entries_for_installation(inst.id)?
                .into_iter()
                .filter(|(_, state, _)| *state != ModState::Disabled)
                .map(|(_, _, e)| e.rel_path.key())
                .collect()
        };
        let catalog = self.catalogs.get(inst.edition);
        let fs = StdFs::new();
        let mut categories = Vec::new();
        let mapped_dtos = mapped
            .iter()
            .map(|(target, staged_path)| {
                if let Some(p) = catalog.mod_pattern(target) {
                    categories.push(p.category);
                }
                // A target that cannot even be resolved (a link in the way) counts as taken.
                let on_disk = match resolve(&fs, &inst.root_path, target) {
                    Ok(path) => fs.exists(&path).unwrap_or(true),
                    Err(_) => true,
                };
                ImportMappedDto {
                    target_path: target.as_str().to_owned(),
                    size: sizes
                        .get(staged_path.as_path())
                        .copied()
                        .unwrap_or_default(),
                    classification: classes
                        .get(&target.key())
                        .map_or(ImportTargetClass::Absent, |c| (*c).into()),
                    conflict: on_disk || owned.contains(&target.key()),
                    protected: catalog.is_original(target)
                        || parse_artifact(target.file_name()).is_some(),
                }
            })
            .collect();
        Ok(ImportPreviewDto {
            staging_id: staged.id.clone(),
            archive_name: staged.archive_name.clone(),
            files: staged
                .archive
                .entries
                .iter()
                .map(|e| ImportFileDto {
                    archive_path: e.rel_path.as_str().to_owned(),
                    size: e.size,
                })
                .collect(),
            suggested_root: suggest_root(&staged.archive.entries).map(|r| r.as_str().to_owned()),
            root: root.map(|r| r.as_str().to_owned()),
            mapped: mapped_dtos,
            suggested_name: suggested_name(Path::new(&staged.archive_name)),
            suggested_category: most_common_category(categories.into_iter()),
        })
    }

    /// Stages a ZIP archive and previews how it maps onto the installation. Any previously
    /// staged archive is discarded. Cancellable; emits `import` progress.
    pub fn stage_import(
        &self,
        installation_id: &str,
        zip_path: &str,
        sink: &dyn ProgressSink,
    ) -> Result<ImportPreviewDto, AppError> {
        let id = parse_id(installation_id)?;
        let zip = PathBuf::from(zip_path);
        self.run_operation(OperationKind::Import, sink, |op| {
            let inst = self.load_validated(id)?;
            op.report(0, 0, Some("Reading the archive"));
            let archive = stage_zip(
                &StdFs::new(),
                &zip,
                &staging_root(&self.dirs),
                &Limits::default(),
                &|| op.is_cancelled(),
            )?;
            let archive_name = zip
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let staged = StagedImport {
                id: uuid::Uuid::now_v7().to_string(),
                installation_id: id,
                archive_name,
                archive,
            };
            if staged.archive.entries.is_empty() {
                remove_staging_dir(&self.dirs, &staged.archive.dir);
                return Err(AppError::new(
                    ErrorCode::InvalidInput,
                    "The archive contains no files.",
                ));
            }
            op.report(staged.archive.entries.len() as u64, 0, None);
            let root = suggest_root(&staged.archive.entries);
            let preview = match self.import_preview(&inst, &staged, root.as_ref()) {
                Ok(p) => p,
                Err(e) => {
                    remove_staging_dir(&self.dirs, &staged.archive.dir);
                    return Err(e);
                }
            };
            let previous = self.staged_lock().replace(staged);
            if let Some(old) = previous {
                remove_staging_dir(&self.dirs, &old.archive.dir);
            }
            Ok(preview)
        })
    }

    /// Recomputes the mapping of the staged archive for another root folder.
    pub fn remap_import(
        &self,
        staging_id: &str,
        root: Option<&str>,
    ) -> Result<ImportPreviewDto, AppError> {
        let root = parse_root(root)?;
        let staged = {
            let guard = self.staged_lock();
            match guard.as_ref() {
                Some(s) if s.id == staging_id => s.clone(),
                _ => return Err(staging_not_found()),
            }
        };
        let inst = self.load_validated(staged.installation_id)?;
        self.import_preview(&inst, &staged, root.as_ref())
    }

    /// Discards the staged archive and deletes its staging folder. Unknown ids are ignored.
    pub fn discard_import(&self, staging_id: &str) -> Result<(), AppError> {
        let mut guard = self.staged_lock();
        if guard.as_ref().is_some_and(|s| s.id == staging_id)
            && let Some(old) = guard.take()
        {
            remove_staging_dir(&self.dirs, &old.archive.dir);
        }
        Ok(())
    }

    /// Imports the staged archive as a new **disabled** mod; with `enable_now` the mod is
    /// then enabled through the normal path (a failure there leaves it imported but disabled).
    pub fn confirm_import(
        &self,
        req: ConfirmImportRequest,
        sink: &dyn ProgressSink,
    ) -> Result<ModDto, AppError> {
        self.run_operation(OperationKind::Import, sink, |op| {
            // Held until the end so a concurrent discard or remap waits for the import.
            let mut guard = self.staged_lock();
            let staged = match guard.as_ref() {
                Some(s) if s.id == req.staging_id => s.clone(),
                _ => return Err(staging_not_found()),
            };
            let name = check_mod_name(&req.name)?;
            let notes = clean_notes(req.notes.clone());
            let root = parse_root(req.root.as_deref())?;
            let inst = self.load_validated(staged.installation_id)?;
            if self
                .lock_db()?
                .list_mods(inst.id)?
                .iter()
                .any(|m| m.name == name)
            {
                return Err(name_taken());
            }
            let mapped = map_to_game(&staged.archive.entries, root.as_ref());
            if mapped.is_empty() {
                return Err(AppError::new(
                    ErrorCode::InvalidInput,
                    "No files of the archive map onto the game folder with this root.",
                )
                .with_hint("Choose another root folder."));
            }
            let catalog = self.catalogs.get(inst.edition);
            let protected: Vec<&str> = mapped
                .iter()
                .filter(|(t, _)| catalog.is_original(t) || parse_artifact(t.file_name()).is_some())
                .map(|(t, _)| t.as_str())
                .collect();
            if !protected.is_empty() {
                return Err(AppError::new(
                    ErrorCode::OwnershipUncertain,
                    "The archive would replace original game files, which the app never touches.",
                )
                .with_hint("Choose another root folder or use a different archive.")
                .with_details(protected.join(", ")));
            }
            let total: u64 = staged.archive.entries.iter().map(|e| e.size).sum();
            self.check_guards_id(
                inst.id,
                GuardNeeds {
                    game_folder_writes: false,
                    bytes_game: 0,
                    bytes_vault: total,
                },
            )?;
            op.report(0, 0, Some("Copying files into the vault"));
            let tx_hint = TransactionId::new();
            let imported = self.with_engine(&inst, |ctx| {
                import_to_vault(ctx.fs, ctx.vault, &mapped, tx_hint)
            })?;
            let mod_id = ModId::new();
            let committed = self.commit_plan(op, &inst, false, |ctx| {
                plan_register_imported(ctx, mod_id, &name, req.category, notes, &imported)
            })?;
            if committed.is_none() {
                return Err(AppError::new(
                    ErrorCode::Internal,
                    "The import had nothing to do.",
                ));
            }
            if let Some(old) = guard.take() {
                remove_staging_dir(&self.dirs, &old.archive.dir);
            }
            drop(guard);
            if req.enable_now
                && let Err(e) = self.set_enabled_inner(inst.id, mod_id, true, false, op)
            {
                let original_hint = e.hint.clone();
                let mut err = e.with_hint("The mod was imported but not enabled.");
                if let Some(h) = original_hint {
                    let details = err.details.take().unwrap_or_default();
                    err.details = Some(format!("{details} (original hint: {h})"));
                }
                return Err(err);
            }
            let db = self.lock_db()?;
            read_mod_dto(&db, mod_id)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_from_archive_stem() {
        assert_eq!(
            suggested_name(Path::new("C:/x/My Mod v1.2.zip")),
            "My Mod v1.2"
        );
        assert_eq!(suggested_name(Path::new("")), "New mod");
        let long = format!("{}.zip", "a".repeat(300));
        assert_eq!(suggested_name(Path::new(&long)).chars().count(), 120);
    }

    #[test]
    fn root_parsing() {
        assert_eq!(parse_root(None).unwrap(), None);
        assert_eq!(parse_root(Some("  ")).unwrap(), None);
        assert_eq!(
            parse_root(Some("Mod/files")).unwrap().unwrap().as_str(),
            "Mod/files"
        );
        assert_eq!(
            parse_root(Some("../x")).unwrap_err().code,
            ErrorCode::InvalidInput
        );
    }
}
