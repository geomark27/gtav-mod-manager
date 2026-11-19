//! Mod queries and metadata: list, detail, update and forget.

use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use gmm_domain::{Category, Mod, ModEntry, ModId, ModState, validate_mod_name};
use gmm_store::Database;
use serde::Serialize;
use ts_rs::TS;

use crate::app::App;
use crate::dependencies::ModRefDto;
use crate::error::{AppError, ErrorCode};
use crate::installations::{not_found, parse_id};
use crate::operation::{NullSink, OperationKind};
use crate::scan::rfc3339;

/// Another mod that targets at least one of the same paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModConflictDto {
    /// The other mod.
    pub mod_id: String,
    /// Its name.
    pub name: String,
}

/// A mod as shown by the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModDto {
    /// Identifier (UUID).
    pub id: String,
    /// Unique name within the installation.
    pub name: String,
    /// Category.
    pub category: Category,
    /// Current state.
    pub state: ModState,
    /// Free-form notes.
    pub notes: Option<String>,
    /// Number of files.
    #[ts(type = "number")]
    pub entry_count: u64,
    /// Total size of its files in bytes.
    #[ts(type = "number")]
    pub total_size: u64,
    /// Mods that share at least one path with this one.
    pub conflicts_with: Vec<ModConflictDto>,
    /// Number of mods this one directly depends on.
    #[ts(type = "number")]
    pub depends_on_count: u64,
    /// Creation time, RFC 3339 UTC.
    pub created_at: String,
    /// Last update time, RFC 3339 UTC.
    pub updated_at: String,
}

/// One file of a mod.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModEntryDto {
    /// Path relative to the installation root.
    pub rel_path: String,
    /// Size in bytes.
    #[ts(type = "number")]
    pub size: u64,
    /// SHA-256 of the content (the vault object).
    pub sha256: String,
    /// `a.b.c.d` from the PE version resource, for `.dll`/`.asi`/`.exe` files of an enabled
    /// mod that are present in the game folder; `null` otherwise.
    pub file_version: Option<String>,
}

/// A mod with its files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModDetailDto {
    /// The mod summary.
    #[serde(flatten)]
    #[ts(flatten)]
    pub summary: ModDto,
    /// Its files, ordered by path.
    pub entries: Vec<ModEntryDto>,
    /// Mods this one directly depends on, ordered by name.
    pub depends_on: Vec<ModRefDto>,
    /// Mods that directly depend on this one, ordered by name.
    pub required_by: Vec<ModRefDto>,
}

/// Resolves mod ids to `{ modId, name }` references, ordered by name.
pub(crate) fn mod_refs(db: &Database, ids: &[ModId]) -> Result<Vec<ModRefDto>, AppError> {
    let mut refs = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(m) = db.get_mod(*id)? {
            refs.push(ModRefDto {
                mod_id: m.id.to_string(),
                name: m.name,
            });
        }
    }
    refs.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.mod_id.cmp(&b.mod_id))
    });
    Ok(refs)
}

pub(crate) fn parse_mod_id(id: &str) -> Result<ModId, AppError> {
    Ok(ModId::from_str(id)?)
}

pub(crate) fn mod_not_found() -> AppError {
    AppError::new(ErrorCode::NotFound, "The mod was not found.")
}

/// Builds the DTOs of every mod of an installation, ordered by name.
pub(crate) fn installation_mod_dtos(
    db: &Database,
    installation_id: gmm_domain::InstallationId,
) -> Result<Vec<ModDto>, AppError> {
    let mods = db.list_mods(installation_id)?;
    let names: HashMap<ModId, &str> = mods.iter().map(|m| (m.id, m.name.as_str())).collect();
    let mut totals: HashMap<ModId, (u64, u64)> = HashMap::new();
    let mut owners: HashMap<String, HashSet<ModId>> = HashMap::new();
    for (mod_id, _state, entry) in db.list_entries_for_installation(installation_id)? {
        let t = totals.entry(mod_id).or_default();
        t.0 += 1;
        t.1 += entry.size;
        owners
            .entry(entry.rel_path.key())
            .or_default()
            .insert(mod_id);
    }
    let mut dep_counts: HashMap<ModId, u64> = HashMap::new();
    for (m, _dep) in db.dependency_edges(installation_id)? {
        *dep_counts.entry(m).or_default() += 1;
    }
    let mut conflicts: HashMap<ModId, HashSet<ModId>> = HashMap::new();
    for ids in owners.values().filter(|ids| ids.len() > 1) {
        for a in ids {
            conflicts
                .entry(*a)
                .or_default()
                .extend(ids.iter().filter(|b| *b != a));
        }
    }
    Ok(mods
        .iter()
        .map(|m| {
            let (entry_count, total_size) = totals.get(&m.id).copied().unwrap_or_default();
            let mut conflicts_with: Vec<ModConflictDto> = conflicts
                .get(&m.id)
                .into_iter()
                .flatten()
                .map(|other| ModConflictDto {
                    mod_id: other.to_string(),
                    name: names.get(other).copied().unwrap_or_default().to_owned(),
                })
                .collect();
            conflicts_with.sort_by(|a, b| {
                a.name
                    .to_lowercase()
                    .cmp(&b.name.to_lowercase())
                    .then_with(|| a.mod_id.cmp(&b.mod_id))
            });
            let depends_on_count = dep_counts.get(&m.id).copied().unwrap_or_default();
            mod_dto(m, entry_count, total_size, conflicts_with, depends_on_count)
        })
        .collect())
}

fn mod_dto(
    m: &Mod,
    entry_count: u64,
    total_size: u64,
    conflicts_with: Vec<ModConflictDto>,
    depends_on_count: u64,
) -> ModDto {
    ModDto {
        id: m.id.to_string(),
        name: m.name.clone(),
        category: m.category,
        state: m.state,
        notes: m.notes.clone(),
        entry_count,
        total_size,
        conflicts_with,
        depends_on_count,
        created_at: rfc3339(m.created_at),
        updated_at: rfc3339(m.updated_at),
    }
}

/// Reads one mod back from the database as a DTO (I-07: only committed state is shown).
pub(crate) fn read_mod_dto(db: &Database, id: ModId) -> Result<ModDto, AppError> {
    let m = db.get_mod(id)?.ok_or_else(mod_not_found)?;
    installation_mod_dtos(db, m.installation_id)?
        .into_iter()
        .find(|d| d.id == id.to_string())
        .ok_or_else(mod_not_found)
}

pub(crate) fn entry_dto(e: &ModEntry, file_version: Option<String>) -> ModEntryDto {
    ModEntryDto {
        rel_path: e.rel_path.as_str().to_owned(),
        size: e.size,
        sha256: e.sha256.as_str().to_owned(),
        file_version,
    }
}

/// True for the file types that carry a PE version resource we show.
fn has_version_resource(rel: &gmm_domain::RelPath) -> bool {
    matches!(
        rel.extension_lower().as_deref(),
        Some("dll" | "asi" | "exe")
    )
}

/// Trims notes; blank notes are stored as `None`.
pub(crate) fn clean_notes(notes: Option<String>) -> Option<String> {
    notes.map(|n| n.trim().to_owned()).filter(|n| !n.is_empty())
}

/// Validates a mod name, with a user-readable message.
pub(crate) fn check_mod_name(name: &str) -> Result<String, AppError> {
    validate_mod_name(name).map_err(|e| {
        AppError::new(
            ErrorCode::InvalidInput,
            "The mod name must be between 1 and 120 characters.",
        )
        .with_details(e.to_string())
    })
}

pub(crate) fn name_taken() -> AppError {
    AppError::new(
        ErrorCode::InvalidInput,
        "A mod with this name already exists in this installation.",
    )
    .with_hint("Choose a different name.")
}

impl App {
    /// Lists the mods of an installation, ordered by name.
    pub fn list_mods(&self, installation_id: &str) -> Result<Vec<ModDto>, AppError> {
        let id = parse_id(installation_id)?;
        let db = self.lock_db()?;
        if db.get_installation(id)?.is_none() {
            return Err(not_found());
        }
        installation_mod_dtos(&db, id)
    }

    /// Returns one mod with its files.
    pub fn get_mod(&self, mod_id: &str) -> Result<ModDetailDto, AppError> {
        let id = parse_mod_id(mod_id)?;
        let (summary, rows, root, depends_on, required_by) = {
            let db = self.lock_db()?;
            let summary = read_mod_dto(&db, id)?;
            let rows = db.list_mod_entries(id)?;
            let root = db
                .get_mod(id)?
                .and_then(|m| db.get_installation(m.installation_id).transpose())
                .transpose()?
                .map(|i| i.root_path);
            let depends_on = mod_refs(&db, &db.dependencies_of(id)?)?;
            let required_by = mod_refs(&db, &db.dependents_of(id)?)?;
            (summary, rows, root, depends_on, required_by)
        };
        // Read-only metadata of files that are really in the game folder (enabled mods).
        let probe_versions = summary.state == ModState::Enabled;
        let fs = gmm_platform::StdFs::new();
        let entries = rows
            .iter()
            .map(|e| {
                let version = match (&root, probe_versions && has_version_resource(&e.rel_path)) {
                    (Some(root), true) => gmm_engine::resolve(&fs, root, &e.rel_path)
                        .ok()
                        .and_then(|path| self.os.file_version(&path)),
                    _ => None,
                };
                entry_dto(e, version)
            })
            .collect();
        Ok(ModDetailDto {
            summary,
            entries,
            depends_on,
            required_by,
        })
    }

    /// Changes name, category and notes of a mod.
    pub fn update_mod(
        &self,
        mod_id: &str,
        name: &str,
        category: Category,
        notes: Option<String>,
    ) -> Result<ModDto, AppError> {
        let id = parse_mod_id(mod_id)?;
        let name = check_mod_name(name)?;
        let notes = clean_notes(notes);
        self.run_operation(OperationKind::Transaction, &NullSink, |_op| {
            let db = self.lock_db()?;
            db.update_mod_meta(id, &name, category, notes.as_deref(), chrono::Utc::now())
                .map_err(|e| match e {
                    gmm_store::Error::Conflict { .. } => name_taken(),
                    other => other.into(),
                })?;
            read_mod_dto(&db, id)
        })
    }

    /// Ends the management of a mod whose files are in the game folder. The files stay
    /// where they are; the vault objects are kept.
    ///
    /// Only `enabled`, `drifted` and `error` mods can be forgotten: a disabled mod has no
    /// files in the game folder, so forgetting it would lose the mod.
    pub fn forget_mod(&self, mod_id: &str) -> Result<(), AppError> {
        let id = parse_mod_id(mod_id)?;
        self.run_operation(OperationKind::Transaction, &NullSink, |op| {
            let (installation_id, name) = {
                let db = self.lock_db()?;
                let m = db.get_mod(id)?.ok_or_else(mod_not_found)?;
                match m.state {
                    ModState::Enabled | ModState::Drifted | ModState::Error => {}
                    ModState::Disabled => {
                        return Err(AppError::new(
                            ErrorCode::InvalidInput,
                            "Enable the mod first so its files are back in the game folder.",
                        ));
                    } // The stored copy is damaged or missing: stopping the management is the
                      // user's way out. Any files in the game folder stay where they are.
                }
                db.delete_mod(id)?;
                (m.installation_id, m.name)
            };
            tracing::info!(%installation_id, mod_id = %id, %name, "mod forgotten");
            self.rescan_best_effort(installation_id, op);
            Ok(())
        })
    }
}
