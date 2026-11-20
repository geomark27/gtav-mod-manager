//! Plans: pure functions over the database and the file system that decide which steps a
//! transaction will run. Planning never writes to the file system or to the journal.

use std::collections::{HashMap, HashSet};

use gmm_domain::{
    Category, Mod, ModEntry, ModEntryId, ModId, ModSource, ModState, RelPath, Sha256, StepKind,
    TransactionKind, parse_artifact, validate_mod_name,
};
use gmm_platform::sha256_file;
use serde::{Deserialize, Serialize};

use crate::context::{EngineContext, inspect_file};
use crate::deps::check_dependency_rules;
use crate::error::{Error, Result};

/// One step of a plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedStep {
    /// What the step does.
    pub kind: StepKind,
    /// Mod the step belongs to (`None` for adoption: the mod row is created at commit).
    pub mod_id: Option<ModId>,
    /// Installation-relative path.
    pub rel_path: RelPath,
    /// Expected content digest (`None` for directories).
    pub sha256: Option<Sha256>,
    /// Size in bytes (0 for directories).
    pub size: u64,
}

/// A complete transaction plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// Transaction kind (callers may override `Group` with `Profile`).
    pub kind: TransactionKind,
    /// Steps in execution order.
    pub steps: Vec<PlannedStep>,
    /// What the commit will record; persisted as the transaction's `summary_json`.
    pub summary: TxSummary,
    /// Bytes that will be copied (vault for adopt, game folder for enable).
    pub bytes_to_copy: u64,
}

/// What a transaction changes in the database once committed.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TxSummary {
    /// New state of every mod whose state changes.
    pub mod_states: Vec<(ModId, ModState)>,
    /// The mod created by an adoption.
    pub adopt: Option<AdoptPayload>,
    /// Vault objects to register at commit (content captured from the game folder by a
    /// restore or re-adopt): `(sha256, size)`.
    #[serde(default)]
    pub vault_objects: Vec<(Sha256, u64)>,
    /// New `(sha256, size)` of existing entries (re-adopt of modified files).
    #[serde(default)]
    pub entry_updates: Vec<(ModEntryId, Sha256, u64)>,
    /// Entries to delete (re-adopt of files that are missing).
    #[serde(default)]
    pub entry_removals: Vec<ModEntryId>,
}

/// Data needed to create the mod of an adoption at commit time (also the ownership manifest).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdoptPayload {
    /// Id of the mod to create.
    pub mod_id: ModId,
    /// Validated (trimmed) name.
    pub name: String,
    /// Category.
    pub category: Category,
    /// Free notes.
    pub notes: Option<String>,
    /// `(path, sha256, size)` of every file.
    pub entries: Vec<(RelPath, Sha256, u64)>,
    /// True when `Unknown` items were included with the extra confirmation.
    pub included_unknown: bool,
    /// State the mod is created in: `Enabled` for an adoption (files are in the game folder),
    /// `Disabled` for an imported mod. Journals written before this field existed are `Enabled`.
    #[serde(default = "enabled")]
    pub initial_state: ModState,
    /// Where the mod came from. Journals written before provenance existed are `manual`.
    #[serde(default)]
    pub source: ModSource,
}

fn enabled() -> ModState {
    ModState::Enabled
}

impl Plan {
    /// Records the provenance of the mod an adoption or import creates (`manual` by default).
    /// Plans that create no mod are returned unchanged.
    #[must_use]
    pub fn with_source(mut self, source: ModSource) -> Self {
        if let Some(adopt) = self.summary.adopt.as_mut() {
            adopt.source = source;
        }
        self
    }
}

pub(crate) fn finish(kind: TransactionKind, steps: Vec<PlannedStep>, summary: TxSummary) -> Plan {
    let bytes_to_copy = steps
        .iter()
        .filter(|s| matches!(s.kind, StepKind::CaptureToVault | StepKind::PlaceFromVault))
        .map(|s| s.size)
        .sum();
    Plan {
        kind,
        steps,
        summary,
        bytes_to_copy,
    }
}

/// Plans the adoption of existing game files as a new mod. The game folder is not modified.
pub fn plan_adopt(
    ctx: &EngineContext<'_>,
    mod_id: ModId,
    name: &str,
    category: Category,
    notes: Option<String>,
    files: &[RelPath],
    included_unknown: bool,
) -> Result<Plan> {
    let name = validate_mod_name(name)?;
    if files.is_empty() {
        return Err(Error::InvalidInput {
            what: "no files to adopt".into(),
        });
    }
    if ctx
        .db
        .list_mods(ctx.installation_id)?
        .iter()
        .any(|m| m.name == name)
    {
        return Err(Error::InvalidInput {
            what: format!("a mod named {name:?} already exists"),
        });
    }
    let keys: Vec<String> = files.iter().map(RelPath::key).collect();
    let unique: HashSet<&String> = keys.iter().collect();
    if unique.len() != keys.len() {
        return Err(Error::InvalidInput {
            what: "duplicate files in the adoption".into(),
        });
    }
    if let Some((key, _)) = ctx
        .db
        .enabled_owners(ctx.installation_id, &keys)?
        .into_iter()
        .next()
    {
        let path = files
            .iter()
            .find(|f| f.key() == key)
            .cloned()
            .unwrap_or_else(|| files[0].clone());
        return Err(Error::PathConflict { path });
    }

    let mut steps = Vec::new();
    let mut entries = Vec::new();
    for rel in files {
        ctx.ensure_not_protected(rel)?;
        if parse_artifact(rel.file_name()).is_some() {
            return Err(Error::InvalidInput {
                what: format!("{rel} looks like an engine artifact"),
            });
        }
        let path = ctx.resolve(rel)?;
        let meta = inspect_file(ctx.fs, &path, rel)?.ok_or_else(|| Error::NotFound {
            what: format!("file {rel}"),
        })?;
        let sha = Sha256::parse(&sha256_file(ctx.fs, &path)?)?;
        let size = meta.len;
        let in_vault = ctx.fs.exists(&ctx.vault.object_path(&sha))?;
        steps.push(PlannedStep {
            kind: StepKind::CaptureToVault,
            mod_id: None,
            rel_path: rel.clone(),
            sha256: Some(sha.clone()),
            // Objects already in the vault copy nothing.
            size: if in_vault { 0 } else { size },
        });
        entries.push((rel.clone(), sha, size));
    }
    let summary = TxSummary {
        adopt: Some(AdoptPayload {
            mod_id,
            name,
            category,
            notes,
            entries,
            included_unknown,
            initial_state: ModState::Enabled,
            source: ModSource::manual(),
        }),
        ..TxSummary::default()
    };
    Ok(finish(TransactionKind::Adopt, steps, summary))
}

/// Plans registering a mod whose files are already in the vault (staged from an archive) as a
/// **disabled** mod. The plan has no steps and the game folder is not touched; the user enables
/// the mod later through [`plan_enable`] (path conflicts are checked then).
pub fn plan_register_imported(
    ctx: &EngineContext<'_>,
    mod_id: ModId,
    name: &str,
    category: Category,
    notes: Option<String>,
    entries: &[(RelPath, Sha256, u64)],
) -> Result<Plan> {
    let name = validate_mod_name(name)?;
    if entries.is_empty() {
        return Err(Error::InvalidInput {
            what: "no files to register".into(),
        });
    }
    if ctx
        .db
        .list_mods(ctx.installation_id)?
        .iter()
        .any(|m| m.name == name)
    {
        return Err(Error::InvalidInput {
            what: format!("a mod named {name:?} already exists"),
        });
    }
    let mut keys = HashSet::new();
    let mut vault_objects: Vec<(Sha256, u64)> = Vec::new();
    for (rel, sha, size) in entries {
        if !keys.insert(rel.key()) {
            return Err(Error::InvalidInput {
                what: format!("duplicate file {rel} in the import"),
            });
        }
        ctx.ensure_not_protected(rel)?;
        if parse_artifact(rel.file_name()).is_some() {
            return Err(Error::InvalidInput {
                what: format!("{rel} looks like an engine artifact"),
            });
        }
        ensure_vault_object(ctx, sha)?;
        if !vault_objects.iter().any(|(s, _)| s == sha) {
            vault_objects.push((sha.clone(), *size));
        }
    }
    let summary = TxSummary {
        adopt: Some(AdoptPayload {
            mod_id,
            name,
            category,
            notes,
            entries: entries.to_vec(),
            included_unknown: false,
            initial_state: ModState::Disabled,
            source: ModSource::manual(),
        }),
        vault_objects,
        ..TxSummary::default()
    };
    Ok(finish(TransactionKind::Adopt, Vec::new(), summary))
}

pub(crate) type LoadedMod = (Mod, Vec<ModEntry>);

pub(crate) fn load_mods(ctx: &EngineContext<'_>, ids: &[ModId]) -> Result<Vec<LoadedMod>> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for id in ids {
        if !seen.insert(*id) {
            return Err(Error::InvalidInput {
                what: "the same mod appears twice".into(),
            });
        }
        let m = ctx
            .db
            .get_mod(*id)?
            .filter(|m| m.installation_id == ctx.installation_id)
            .ok_or_else(|| Error::NotFound {
                what: format!("mod {id}"),
            })?;
        let entries = ctx.db.list_mod_entries(*id)?;
        out.push((m, entries));
    }
    Ok(out)
}

pub(crate) fn require_state(m: &Mod, state: ModState) -> Result<()> {
    if m.state == state {
        Ok(())
    } else {
        Err(Error::InvalidState {
            what: format!("mod {:?} is {}, expected {}", m.name, m.state, state),
        })
    }
}

pub(crate) fn ensure_vault_object(ctx: &EngineContext<'_>, sha: &Sha256) -> Result<()> {
    if ctx.vault.verify(ctx.fs, sha)? {
        Ok(())
    } else {
        Err(Error::VaultCorrupt { sha: sha.clone() })
    }
}

fn build_disable(ctx: &EngineContext<'_>, mods: &[LoadedMod]) -> Result<Vec<PlannedStep>> {
    let mut steps = Vec::new();
    for (m, entries) in mods {
        require_state(m, ModState::Enabled)?;
        for e in entries {
            ctx.ensure_not_protected(&e.rel_path)?;
            let path = ctx.resolve(&e.rel_path)?;
            if inspect_file(ctx.fs, &path, &e.rel_path)?.is_none()
                || sha256_file(ctx.fs, &path)? != e.sha256.as_str()
            {
                return Err(Error::Drift {
                    path: e.rel_path.clone(),
                });
            }
            ensure_vault_object(ctx, &e.sha256)?;
            steps.push(PlannedStep {
                kind: StepKind::TombstoneGameFile,
                mod_id: Some(m.id),
                rel_path: e.rel_path.clone(),
                sha256: Some(e.sha256.clone()),
                size: e.size,
            });
        }
    }
    Ok(steps)
}

/// `released`: path keys freed by mods disabled earlier in the same plan.
fn build_enable(
    ctx: &EngineContext<'_>,
    mods: &[LoadedMod],
    released: &HashSet<String>,
) -> Result<Vec<PlannedStep>> {
    let mut steps = Vec::new();
    let mut claimed: HashMap<String, ModId> = HashMap::new();
    let mut dirs: HashSet<String> = HashSet::new();
    let all_keys: Vec<String> = mods
        .iter()
        .flat_map(|(_, es)| es.iter().map(|e| e.rel_path.key()))
        .collect();
    let owners = ctx.db.enabled_owners(ctx.installation_id, &all_keys)?;
    for (m, entries) in mods {
        require_state(m, ModState::Disabled)?;
        for e in entries {
            let rel = &e.rel_path;
            let key = rel.key();
            ctx.ensure_not_protected(rel)?;
            let claimed_twice = claimed.insert(key.clone(), m.id).is_some();
            let owned_elsewhere = owners.iter().any(|(k, _)| *k == key) && !released.contains(&key);
            if claimed_twice || owned_elsewhere {
                return Err(Error::PathConflict { path: rel.clone() });
            }
            let path = ctx.resolve(rel)?;
            if !released.contains(&key) && ctx.fs.exists(&path)? {
                return Err(Error::PathConflict { path: rel.clone() });
            }
            ensure_vault_object(ctx, &e.sha256)?;
            let mut ancestors = Vec::new();
            let mut cur = rel.parent();
            while let Some(dir) = cur {
                cur = dir.parent();
                ancestors.push(dir);
            }
            for dir in ancestors.into_iter().rev() {
                if dirs.contains(&dir.key()) || ctx.fs.exists(&ctx.resolve(&dir)?)? {
                    continue;
                }
                dirs.insert(dir.key());
                steps.push(PlannedStep {
                    kind: StepKind::EnsureDir,
                    mod_id: Some(m.id),
                    rel_path: dir,
                    sha256: None,
                    size: 0,
                });
            }
            steps.push(PlannedStep {
                kind: StepKind::PlaceFromVault,
                mod_id: Some(m.id),
                rel_path: rel.clone(),
                sha256: Some(e.sha256.clone()),
                size: e.size,
            });
        }
    }
    Ok(steps)
}

fn states(mods: &[LoadedMod], state: ModState) -> Vec<(ModId, ModState)> {
    mods.iter().map(|(m, _)| (m.id, state)).collect()
}

/// Plans enabling disabled mods: places their files from the vault into the game folder.
pub fn plan_enable(ctx: &EngineContext<'_>, mod_ids: &[ModId]) -> Result<Plan> {
    let mods = load_mods(ctx, mod_ids)?;
    let steps = build_enable(ctx, &mods, &HashSet::new())?;
    let mod_states = states(&mods, ModState::Enabled);
    check_dependency_rules(ctx, &mod_states)?;
    let summary = TxSummary {
        mod_states,
        ..TxSummary::default()
    };
    Ok(finish(TransactionKind::Enable, steps, summary))
}

/// Plans disabling enabled mods: moves their files to tombstones, deleted at commit.
pub fn plan_disable(ctx: &EngineContext<'_>, mod_ids: &[ModId]) -> Result<Plan> {
    let mods = load_mods(ctx, mod_ids)?;
    let steps = build_disable(ctx, &mods)?;
    let mod_states = states(&mods, ModState::Disabled);
    check_dependency_rules(ctx, &mod_states)?;
    let summary = TxSummary {
        mod_states,
        ..TxSummary::default()
    };
    Ok(finish(TransactionKind::Disable, steps, summary))
}

/// Plans a group switch / profile: disables first, then enables, in a single plan.
///
/// Mods already in the desired state are skipped. The caller may change the plan kind to
/// `Profile`.
pub fn plan_set_states(ctx: &EngineContext<'_>, desired: &[(ModId, bool)]) -> Result<Plan> {
    let ids: Vec<ModId> = desired.iter().map(|(id, _)| *id).collect();
    let loaded = load_mods(ctx, &ids)?;
    let mut to_disable = Vec::new();
    let mut to_enable = Vec::new();
    for (loaded_mod, (_, want_enabled)) in loaded.into_iter().zip(desired) {
        match (loaded_mod.0.state, want_enabled) {
            (ModState::Enabled, true) | (ModState::Disabled, false) => {}
            (ModState::Enabled, false) => to_disable.push(loaded_mod),
            (ModState::Disabled, true) => to_enable.push(loaded_mod),
            (other, _) => {
                return Err(Error::InvalidState {
                    what: format!("mod {:?} is {other}", loaded_mod.0.name),
                });
            }
        }
    }
    let released: HashSet<String> = to_disable
        .iter()
        .flat_map(|(_, es)| es.iter().map(|e| e.rel_path.key()))
        .collect();
    let mut steps = build_disable(ctx, &to_disable)?;
    steps.extend(build_enable(ctx, &to_enable, &released)?);
    let mut mod_states = states(&to_disable, ModState::Disabled);
    mod_states.extend(states(&to_enable, ModState::Enabled));
    check_dependency_rules(ctx, &mod_states)?;
    let summary = TxSummary {
        mod_states,
        ..TxSummary::default()
    };
    Ok(finish(TransactionKind::Group, steps, summary))
}
