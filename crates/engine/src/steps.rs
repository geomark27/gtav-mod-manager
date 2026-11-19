//! Effect and undo of each step kind. The executor journals around these functions; the undo
//! functions inspect the real disk first so they are idempotent (shared by rollback and recovery).

use std::path::{Path, PathBuf};

use gmm_domain::{
    RelPath, Sha256, StepKind, StepStatus, TransactionId, parse_artifact, tmp_name, tomb_name,
};
use gmm_platform::{EntryKind, Fs, sha256_file};
use gmm_store::{Database, StepRow};

use crate::context::{EngineContext, inspect_file, resolve};
use crate::error::{Error, Result};
use crate::vault::{Vault, vault_tmp_name};

/// What undo needs: no protection callback (undo only touches journaled artifacts).
pub(crate) struct Env<'a> {
    pub fs: &'a dyn Fs,
    pub db: &'a Database,
    pub vault: &'a Vault,
    pub root: &'a Path,
}

impl EngineContext<'_> {
    pub(crate) fn env(&self) -> Env<'_> {
        Env {
            fs: self.fs,
            db: self.db,
            vault: self.vault,
            root: self.root,
        }
    }
}

fn need_sha<'a>(sha: Option<&'a Sha256>, rel: &RelPath) -> Result<&'a Sha256> {
    sha.ok_or_else(|| Error::InvalidState {
        what: format!("step for {rel} has no digest"),
    })
}

/// Name of the artifact a step may leave on disk (recorded in the journal before the effect).
pub(crate) fn artifact_name(
    kind: StepKind,
    rel: &RelPath,
    sha: Option<&Sha256>,
    tx: TransactionId,
) -> Result<Option<String>> {
    Ok(match kind {
        StepKind::CaptureToVault => Some(vault_tmp_name(need_sha(sha, rel)?, tx)),
        StepKind::EnsureDir => None,
        StepKind::PlaceFromVault => Some(tmp_name(rel.file_name(), tx)),
        StepKind::TombstoneGameFile => Some(tomb_name(rel.file_name(), tx)),
    })
}

/// Checks that need no journal entry because they happen before any effect: a step that fails
/// here stays `pending` and is never undone (so a foreign file at the target is never judged).
pub(crate) fn precheck(ctx: &EngineContext<'_>, kind: StepKind, rel: &RelPath) -> Result<()> {
    ctx.ensure_not_protected(rel)?;
    let path = ctx.resolve(rel)?;
    if kind == StepKind::PlaceFromVault && ctx.fs.exists(&path)? {
        return Err(Error::PathConflict { path: rel.clone() });
    }
    Ok(())
}

/// Performs the file-system effect of one step. Returns `created_by_step`.
pub(crate) fn perform(
    ctx: &EngineContext<'_>,
    tx: TransactionId,
    kind: StepKind,
    rel: &RelPath,
    sha: Option<&Sha256>,
) -> Result<bool> {
    let fs = ctx.fs;
    let path = ctx.resolve(rel)?;
    match kind {
        StepKind::CaptureToVault => {
            let sha = need_sha(sha, rel)?;
            if inspect_file(fs, &path, rel)?.is_none() {
                return Err(Error::NotFound {
                    what: format!("file {rel}"),
                });
            }
            ctx.vault.store_from(fs, &path, sha, tx)
        }
        StepKind::EnsureDir => {
            if fs.exists(&path)? {
                let meta = fs.metadata(&path)?;
                if meta.kind == EntryKind::Dir && !meta.is_reparse_point {
                    return Ok(false);
                }
                return Err(Error::PathConflict { path: rel.clone() });
            }
            fs.create_dir(&path)?;
            Ok(true)
        }
        StepKind::PlaceFromVault => {
            let sha = need_sha(sha, rel)?;
            if fs.exists(&path)? {
                return Err(Error::PathConflict { path: rel.clone() });
            }
            if !ctx.vault.verify(fs, sha)? {
                return Err(Error::VaultCorrupt { sha: sha.clone() });
            }
            let tmp = path.with_file_name(tmp_name(rel.file_name(), tx));
            fs.copy_new(&ctx.vault.object_path(sha), &tmp)?;
            if sha256_file(fs, &tmp)? != sha.as_str() {
                fs.remove_file(&tmp)?;
                return Err(Error::HashMismatch {
                    path: rel.to_string(),
                });
            }
            match fs.rename_no_replace(&tmp, &path) {
                Ok(()) => Ok(true),
                Err(gmm_platform::Error::AlreadyExists { .. }) => {
                    Err(Error::PathConflict { path: rel.clone() })
                }
                Err(e) => Err(e.into()),
            }
        }
        StepKind::TombstoneGameFile => {
            let sha = need_sha(sha, rel)?;
            let drift = || Error::Drift { path: rel.clone() };
            if inspect_file(fs, &path, rel)?.is_none() || sha256_file(fs, &path)? != sha.as_str() {
                return Err(drift());
            }
            if !ctx.vault.verify(fs, sha)? {
                return Err(Error::VaultCorrupt { sha: sha.clone() });
            }
            let tomb = path.with_file_name(tomb_name(rel.file_name(), tx));
            match fs.rename_no_replace(&path, &tomb) {
                Ok(()) => Ok(false),
                Err(gmm_platform::Error::AlreadyExists { .. }) => {
                    Err(Error::PathConflict { path: rel.clone() })
                }
                Err(e) => Err(e.into()),
            }
        }
    }
}

/// Result of undoing one step.
pub(crate) enum Undo {
    /// The step's effect is gone (or never happened).
    Undone,
    /// The undo cannot be proven safe; the reason is shown to the user.
    Unsafe(String),
}

/// Removes `path` only when its file name is a well-formed engine artifact name (I-08).
fn remove_artifact(fs: &dyn Fs, path: &Path) -> Result<()> {
    let named = path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| parse_artifact(n).is_some());
    if named && fs.exists(path)? {
        fs.remove_file(path)?;
    }
    Ok(())
}

/// Undoes one `started` or `done` step. Errors other than a crash or a database failure turn
/// into `Unsafe`.
pub(crate) fn undo_step(env: &Env<'_>, step: &StepRow) -> Result<Undo> {
    match undo_inner(env, step) {
        Err(
            e @ (Error::Fs(_) | Error::OwnershipUncertain { .. } | Error::PathOutsideRoot { .. }),
        ) => Ok(Undo::Unsafe(format!("{}: {e}", step.rel_path))),
        other => other,
    }
}

fn undo_inner(env: &Env<'_>, step: &StepRow) -> Result<Undo> {
    let fs = env.fs;
    let rel = &step.rel_path;
    let unsafe_step = |why: &str| Ok(Undo::Unsafe(format!("{rel}: {why}")));
    match step.kind {
        StepKind::CaptureToVault => {
            let Some(sha) = &step.sha256 else {
                return unsafe_step("capture step without digest");
            };
            let object = env.vault.object_path(sha);
            if let Some(name) = &step.artifact_name {
                remove_artifact(fs, &object.with_file_name(name))?;
            }
            if step.status == StepStatus::Done
                && step.created_by_step
                && !env.db.vault_object_referenced(sha)?
                && fs.exists(&object)?
            {
                fs.set_readonly(&object, false)?;
                fs.remove_file(&object)?;
            }
        }
        StepKind::EnsureDir => {
            if step.status == StepStatus::Done && !step.created_by_step {
                return Ok(Undo::Undone);
            }
            let path = resolve(fs, env.root, rel)?;
            if !fs.exists(&path)? {
                return Ok(Undo::Undone);
            }
            let meta = fs.metadata(&path)?;
            if meta.kind != EntryKind::Dir || meta.is_reparse_point {
                tracing::warn!(%rel, "not a real directory; left in place");
            } else if fs.read_dir(&path)?.is_empty() {
                fs.remove_dir(&path)?;
            } else {
                tracing::warn!(%rel, "directory is not empty; left in place");
            }
        }
        StepKind::PlaceFromVault => {
            let Some(sha) = &step.sha256 else {
                return unsafe_step("place step without digest");
            };
            let path = resolve(fs, env.root, rel)?;
            if let Some(name) = &step.artifact_name {
                remove_artifact(fs, &path.with_file_name(name))?;
            }
            if inspect_file(fs, &path, rel)?.is_some() {
                if sha256_file(fs, &path)? != sha.as_str() {
                    return unsafe_step("the target exists with different content");
                }
                fs.remove_file(&path)?;
            }
        }
        StepKind::TombstoneGameFile => {
            let path = resolve(fs, env.root, rel)?;
            let Some(name) = &step.artifact_name else {
                return unsafe_step("tombstone step without artifact name");
            };
            let tomb = path.with_file_name(name);
            match (fs.exists(&tomb)?, fs.exists(&path)?) {
                (true, false) => fs.rename_no_replace(&tomb, &path)?,
                (false, true) => {}
                (true, true) => return unsafe_step("both the file and its tombstone exist"),
                (false, false) => return unsafe_step("neither the file nor its tombstone exist"),
            }
        }
    }
    Ok(Undo::Undone)
}

/// Deletes the tombstones of the `done` tombstone steps (missing = already deleted).
///
/// A tombstone whose content is not the recorded digest is never deleted.
pub(crate) fn delete_tombstones(env: &Env<'_>, steps: &[StepRow]) -> Result<()> {
    for step in steps
        .iter()
        .filter(|s| s.kind == StepKind::TombstoneGameFile && s.status == StepStatus::Done)
    {
        let (Some(name), Some(sha)) = (&step.artifact_name, &step.sha256) else {
            continue;
        };
        let tomb: PathBuf = resolve(env.fs, env.root, &step.rel_path)?.with_file_name(name);
        if !env.fs.exists(&tomb)? {
            continue;
        }
        if sha256_file(env.fs, &tomb)? == sha.as_str() {
            env.fs.remove_file(&tomb)?;
        } else {
            tracing::warn!(rel = %step.rel_path, "tombstone content differs from the journal; kept");
        }
    }
    Ok(())
}
