//! Guards evaluated before planning and again right before executing a transaction
//! (`docs/architecture.md` section 9).
//!
//! Order of evaluation (first failure wins): game running, installation missing, recovery
//! required, installation not writable, insufficient space.

use std::path::{Path, PathBuf};

use gmm_domain::{Installation, InstallationId, InstallationStatus};
use gmm_platform::{Fs, StdFs};
use gmm_scanner::running_guarded;
use serde::Serialize;
use ts_rs::TS;

use crate::app::App;
use crate::error::{AppError, ErrorCode};
use crate::installations::{not_found, parse_id};

/// Free space that must remain after an operation (512 MiB).
pub const SPACE_MARGIN_BYTES: u64 = 512 * 1024 * 1024;

/// Reports free disk space for the volume that holds a path.
pub trait SpaceProbe: Send + Sync {
    /// Free bytes on the volume of `path`; `None` when unknown.
    fn free_bytes(&self, path: &Path) -> Option<u64>;
}

/// The real disks, through `sysinfo`.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemSpace;

fn norm(path: &Path) -> String {
    path.to_string_lossy().to_lowercase().replace('\\', "/")
}

/// Picks the disk whose mount point is the longest prefix of `path`.
pub(crate) fn pick_free_space(path: &Path, disks: &[(PathBuf, u64)]) -> Option<u64> {
    let p = norm(path);
    disks
        .iter()
        .filter_map(|(mount, free)| {
            let m = norm(mount);
            let m = m.trim_end_matches('/');
            let inside = p == m || p.starts_with(&format!("{m}/"));
            inside.then_some((m.len(), *free))
        })
        .max_by_key(|(len, _)| *len)
        .map(|(_, free)| free)
}

impl SpaceProbe for SystemSpace {
    fn free_bytes(&self, path: &Path) -> Option<u64> {
        let disks = match sysinfo::Disks::new_with_refreshed_list() {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!(error = %e, "could not list disks");
                return None;
            }
        };
        let list: Vec<(PathBuf, u64)> = disks
            .iter()
            .map(|d| (d.mount_point().to_path_buf(), d.available_space()))
            .collect();
        let resolved = dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        pick_free_space(&resolved, &list)
    }
}

/// Guard state of an installation, as shown by the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GuardStatusDto {
    /// Guarded processes that are running (empty when none).
    pub game_running: Vec<String>,
    /// Whether the installation folder accepts writes.
    pub writable: bool,
    /// Whether an interrupted or unresolved transaction blocks the installation.
    pub recovery_required: bool,
    /// Whether the installation folder is missing.
    pub installation_missing: bool,
}

/// What a planned operation needs from the guards.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GuardNeeds {
    /// True when the operation creates, renames or deletes files in the game folder.
    pub game_folder_writes: bool,
    /// Bytes that will be written to the game volume.
    pub bytes_game: u64,
    /// Bytes that will be written to the vault volume.
    pub bytes_vault: u64,
}

/// Creates and removes a probe file in `root`; true when both worked.
fn write_probe(root: &Path) -> bool {
    let fs = StdFs::new();
    let probe = root.join(format!(".gmm-probe-{}", uuid::Uuid::new_v4().simple()));
    if fs.write_new(&probe, b"").is_err() {
        if matches!(fs.exists(&probe), Ok(true)) && fs.remove_file(&probe).is_err() {
            tracing::error!(path = %probe.display(), "write probe file could not be removed");
        }
        return false;
    }
    if let Err(e) = fs.remove_file(&probe) {
        tracing::error!(path = %probe.display(), error = %e, "write probe file could not be removed");
        return false;
    }
    true
}

fn mib(bytes: u64) -> u64 {
    bytes.div_ceil(1024 * 1024)
}

impl App {
    /// Re-validates the installation folder (persisting the status) and probes every guard.
    fn guard_snapshot(&self, id: InstallationId) -> Result<GuardStatusDto, AppError> {
        let inst: Installation = self
            .lock_db()?
            .get_installation(id)?
            .ok_or_else(not_found)?;
        let report = self.validate_root(&inst.root_path);
        let recovery_required = {
            let db = self.lock_db()?;
            db.update_installation_validation(
                id,
                report.status,
                report.exe_version.as_deref(),
                chrono::Utc::now(),
            )?;
            self.has_unresolved_transactions(&db, id)?
        };
        let missing = report.status == InstallationStatus::Missing;
        Ok(GuardStatusDto {
            game_running: running_guarded(&*self.os),
            writable: !missing && write_probe(&inst.root_path),
            recovery_required,
            installation_missing: missing,
        })
    }

    /// True when the installation has a `needs_attention` or an unrecovered transaction.
    pub(crate) fn has_unresolved_transactions(
        &self,
        db: &gmm_store::Database,
        id: InstallationId,
    ) -> Result<bool, AppError> {
        Ok(db.installation_has_attention(id)?
            || db
                .transactions_needing_recovery()?
                .iter()
                .any(|t| t.installation_id == id))
    }

    /// Reports the state of every guard for an installation.
    pub fn guard_status(&self, installation_id: &str) -> Result<GuardStatusDto, AppError> {
        self.guard_snapshot(parse_id(installation_id)?)
    }

    /// Fails with the first guard that blocks an operation with the given needs.
    pub fn check_guards(&self, installation_id: &str, need: GuardNeeds) -> Result<(), AppError> {
        self.check_guards_id(parse_id(installation_id)?, need)
    }

    pub(crate) fn check_guards_id(
        &self,
        id: InstallationId,
        need: GuardNeeds,
    ) -> Result<(), AppError> {
        let status = self.guard_snapshot(id)?;
        if !status.game_running.is_empty() {
            return Err(AppError::new(
                ErrorCode::GameRunning,
                "GTA V or one of its launchers is running.",
            )
            .with_hint("Close the game and its launchers, then try again.")
            .with_details(status.game_running.join(", ")));
        }
        if status.installation_missing {
            return Err(AppError::new(
                ErrorCode::NotFound,
                "The installation folder was not found.",
            )
            .with_hint("Check that the drive is connected, then refresh the installation."));
        }
        if status.recovery_required {
            return Err(AppError::new(
                ErrorCode::RecoveryRequired,
                "An earlier operation on this installation must be resolved first.",
            )
            .with_hint("Restart the app so it can finish or undo the interrupted operation."));
        }
        if need.game_folder_writes && !status.writable {
            return Err(AppError::new(
                ErrorCode::InstallNotWritable,
                "The game folder is not writable for this app.",
            )
            .with_hint("The app never runs elevated. Move the game outside protected folders or fix the folder permissions."));
        }
        let root = self
            .lock_db()?
            .get_installation(id)?
            .ok_or_else(not_found)?
            .root_path;
        self.check_space(&root, need.bytes_game, "the game folder")?;
        self.check_space(&self.dirs.vault_dir, need.bytes_vault, "the backup vault")
    }

    fn check_space(&self, path: &Path, bytes: u64, what: &str) -> Result<(), AppError> {
        if bytes == 0 {
            return Ok(());
        }
        let required = bytes.saturating_add(SPACE_MARGIN_BYTES);
        match self.space.free_bytes(path) {
            None => {
                tracing::warn!(path = %path.display(), "free disk space is unknown; not blocking");
                Ok(())
            }
            Some(free) if free >= required => Ok(()),
            Some(free) => Err(AppError::new(
                ErrorCode::InsufficientSpace,
                format!("Not enough free disk space for {what}."),
            )
            .with_hint(format!(
                "Free at least {} MiB on the drive of {}, then try again.",
                mib(required - free),
                path.display()
            ))
            .with_details(format!(
                "free {free} bytes, required {required} bytes ({bytes} + 512 MiB margin)"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disks() -> Vec<(PathBuf, u64)> {
        vec![
            (PathBuf::from("/"), 1),
            (PathBuf::from("/mnt/c"), 2),
            (PathBuf::from("/mnt/c/games"), 3),
            (PathBuf::from("D:\\"), 4),
        ]
    }

    #[test]
    fn longest_mount_prefix_wins() {
        let d = disks();
        assert_eq!(pick_free_space(Path::new("/home/u"), &d), Some(1));
        assert_eq!(pick_free_space(Path::new("/mnt/c/Users"), &d), Some(2));
        assert_eq!(pick_free_space(Path::new("/mnt/c/games/gta"), &d), Some(3));
        assert_eq!(pick_free_space(Path::new("/mnt/c/gamesx"), &d), Some(2));
        assert_eq!(pick_free_space(Path::new("d:\\Steam\\GTA V"), &d), Some(4));
        assert_eq!(pick_free_space(Path::new("E:\\x"), &d), None);
    }

    #[test]
    fn mib_rounds_up() {
        assert_eq!(mib(1), 1);
        assert_eq!(mib(1024 * 1024), 1);
        assert_eq!(mib(1024 * 1024 + 1), 2);
    }

    #[test]
    fn guard_status_json_shape() {
        let v = serde_json::to_value(GuardStatusDto {
            game_running: vec!["GTA5.exe".into()],
            writable: true,
            recovery_required: false,
            installation_missing: false,
        })
        .unwrap();
        assert_eq!(
            v,
            serde_json::json!({"gameRunning":["GTA5.exe"],"writable":true,
                "recoveryRequired":false,"installationMissing":false})
        );
    }
}
