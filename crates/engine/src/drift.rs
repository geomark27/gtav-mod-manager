//! Resolution of drift: restore the vault version of a drifted mod, or re-adopt what is on
//! disk. Nothing the user had is lost: any file the engine removes is first captured (I-02).

use std::collections::HashSet;

use gmm_domain::{ModEntry, ModId, ModState, RelPath, Sha256, StepKind, TransactionKind};
use gmm_platform::sha256_file;

use crate::context::{EngineContext, inspect_file};
use crate::error::{Error, Result};
use crate::plan::{
    Plan, PlannedStep, TxSummary, ensure_vault_object, finish, load_mods, require_state,
};

/// What an entry looks like on disk right now.
enum OnDisk {
    Missing,
    Unchanged,
    Modified { sha: Sha256, size: u64 },
}

fn observe(ctx: &EngineContext<'_>, e: &ModEntry) -> Result<OnDisk> {
    ctx.ensure_not_protected(&e.rel_path)?;
    let path = ctx.resolve(&e.rel_path)?;
    let Some(meta) = inspect_file(ctx.fs, &path, &e.rel_path)? else {
        return Ok(OnDisk::Missing);
    };
    let sha = Sha256::parse(&sha256_file(ctx.fs, &path)?)?;
    if sha == e.sha256 {
        Ok(OnDisk::Unchanged)
    } else {
        Ok(OnDisk::Modified {
            sha,
            size: meta.len,
        })
    }
}

/// Capture step for content found in the game folder. A verified object that already exists
/// copies nothing; an existing but corrupt one is refused.
fn capture_step(
    ctx: &EngineContext<'_>,
    mod_id: ModId,
    rel: &RelPath,
    sha: &Sha256,
    size: u64,
) -> Result<PlannedStep> {
    let in_vault = ctx.fs.exists(&ctx.vault.object_path(sha))?;
    if in_vault {
        ensure_vault_object(ctx, sha)?;
    }
    Ok(PlannedStep {
        kind: StepKind::CaptureToVault,
        mod_id: Some(mod_id),
        rel_path: rel.clone(),
        sha256: Some(sha.clone()),
        size: if in_vault { 0 } else { size },
    })
}

fn load_drifted(ctx: &EngineContext<'_>, mod_id: ModId) -> Result<(ModId, Vec<ModEntry>)> {
    let mut loaded = load_mods(ctx, &[mod_id])?;
    let Some((m, entries)) = loaded.pop() else {
        return Err(Error::NotFound {
            what: format!("mod {mod_id}"),
        });
    };
    require_state(&m, ModState::Drifted)?;
    Ok((m.id, entries))
}

/// Plans restoring the vault version of every drifted entry of a `Drifted` mod.
///
/// Missing files are placed from the vault; modified files are first captured to the vault
/// (the user's version is preserved), then tombstoned, then replaced by the vault version.
pub fn plan_restore(ctx: &EngineContext<'_>, mod_id: ModId) -> Result<Plan> {
    let (id, entries) = load_drifted(ctx, mod_id)?;
    for e in &entries {
        ensure_vault_object(ctx, &e.sha256)?;
    }
    let mut steps = Vec::new();
    let mut captured: Vec<(Sha256, u64)> = Vec::new();
    let mut dirs: HashSet<String> = HashSet::new();
    for e in &entries {
        let rel = &e.rel_path;
        match observe(ctx, e)? {
            OnDisk::Unchanged => continue,
            OnDisk::Missing => {
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
                        mod_id: Some(id),
                        rel_path: dir,
                        sha256: None,
                        size: 0,
                    });
                }
            }
            OnDisk::Modified { sha, size } => {
                steps.push(capture_step(ctx, id, rel, &sha, size)?);
                if !captured.iter().any(|(s, _)| *s == sha) {
                    captured.push((sha.clone(), size));
                }
                // The tombstone frees the path, so the placement below does not conflict.
                steps.push(PlannedStep {
                    kind: StepKind::TombstoneGameFile,
                    mod_id: Some(id),
                    rel_path: rel.clone(),
                    sha256: Some(sha),
                    size,
                });
            }
        }
        steps.push(PlannedStep {
            kind: StepKind::PlaceFromVault,
            mod_id: Some(id),
            rel_path: rel.clone(),
            sha256: Some(e.sha256.clone()),
            size: e.size,
        });
    }
    if steps.is_empty() {
        return Err(Error::InvalidState {
            what: "no drift found; reconcile the installation again".into(),
        });
    }
    let summary = TxSummary {
        mod_states: vec![(id, ModState::Enabled)],
        vault_objects: captured,
        ..TxSummary::default()
    };
    Ok(finish(TransactionKind::Restore, steps, summary))
}

/// Plans accepting the current content of a `Drifted` mod: modified files are captured to the
/// vault and their entries updated; missing files are dropped from the mod. Game files are
/// never touched.
pub fn plan_readopt(ctx: &EngineContext<'_>, mod_id: ModId) -> Result<Plan> {
    let (id, entries) = load_drifted(ctx, mod_id)?;
    let mut steps = Vec::new();
    let mut summary = TxSummary {
        mod_states: vec![(id, ModState::Enabled)],
        ..TxSummary::default()
    };
    for e in &entries {
        match observe(ctx, e)? {
            OnDisk::Unchanged => {}
            OnDisk::Missing => summary.entry_removals.push(e.id),
            OnDisk::Modified { sha, size } => {
                steps.push(capture_step(ctx, id, &e.rel_path, &sha, size)?);
                if !summary.vault_objects.iter().any(|(s, _)| *s == sha) {
                    summary.vault_objects.push((sha.clone(), size));
                }
                summary.entry_updates.push((e.id, sha, size));
            }
        }
    }
    if summary.entry_removals.len() == entries.len() {
        return Err(Error::InvalidInput {
            what: "All files of this mod are missing; restore it instead.".into(),
        });
    }
    if steps.is_empty() && summary.entry_removals.is_empty() {
        return Err(Error::InvalidState {
            what: "no drift found; reconcile the installation again".into(),
        });
    }
    Ok(finish(TransactionKind::Adopt, steps, summary))
}
