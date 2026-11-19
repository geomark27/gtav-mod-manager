//! Game installations.

use crate::{Edition, InstallationId};
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};

persisted_enum!(
    /// Store the game was bought from.
    DistributionPlatform, "platform" {
        Steam => "steam",
        Epic => "epic",
        Rockstar => "rockstar",
        Unknown => "unknown",
    }
);

persisted_enum!(
    /// Health of an installation folder.
    InstallationStatus, "installation status" {
        Ok => "ok",
        Missing => "missing",
        Incomplete => "incomplete",
        NotWritable => "not_writable",
    }
);

persisted_enum!(
    /// How far the captured baseline can be trusted.
    BaselineTrust, "baseline trust" {
        None => "none",
        Untrusted => "untrusted",
        Trusted => "trusted",
    }
);

/// A registered game installation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Installation {
    /// Identifier.
    pub id: InstallationId,
    /// Game edition.
    pub edition: Edition,
    /// Distribution platform.
    pub platform: DistributionPlatform,
    /// Name shown in the UI.
    pub display_name: String,
    /// Absolute root folder of the game.
    pub root_path: PathBuf,
    /// Normalized comparison key of `root_path` (see [`root_key`]).
    pub root_key: String,
    /// Executable version, when known.
    pub exe_version: Option<String>,
    /// Folder health.
    pub status: InstallationStatus,
    /// Baseline trust level.
    pub baseline: BaselineTrust,
    /// Creation time.
    pub created_at: DateTime<Utc>,
    /// Last update time.
    pub updated_at: DateTime<Utc>,
}

/// Normalized comparison key for an installation root: lowercase, `\` replaced by `/`,
/// no trailing `/` except for a bare root such as `c:/` or `/`.
pub fn root_key(path: &Path) -> String {
    let lowered = path.to_string_lossy().to_lowercase().replace('\\', "/");
    if lowered.is_empty() {
        return lowered;
    }
    let trimmed = lowered.trim_end_matches('/');
    if trimmed.is_empty() {
        return "/".to_owned();
    }
    let bytes = trimmed.as_bytes();
    if bytes.len() == 2 && bytes[1] == b':' {
        return format!("{trimmed}/");
    }
    trimmed.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macros::assert_enum_contract;

    #[test]
    fn platform_contract() {
        assert_enum_contract(
            &DistributionPlatform::ALL,
            &["steam", "epic", "rockstar", "unknown"],
            DistributionPlatform::as_str,
        );
    }

    #[test]
    fn status_contract() {
        assert_enum_contract(
            &InstallationStatus::ALL,
            &["ok", "missing", "incomplete", "not_writable"],
            InstallationStatus::as_str,
        );
    }

    #[test]
    fn baseline_contract() {
        assert_enum_contract(
            &BaselineTrust::ALL,
            &["none", "untrusted", "trusted"],
            BaselineTrust::as_str,
        );
    }

    #[test]
    fn root_key_normalizes() {
        assert_eq!(root_key(Path::new("C:\\Games\\GTA V\\")), "c:/games/gta v");
        assert_eq!(root_key(Path::new("C:\\Games\\GTA V")), "c:/games/gta v");
        assert_eq!(root_key(Path::new("D:/Steam/GTAV//")), "d:/steam/gtav");
    }

    #[test]
    fn root_key_keeps_bare_roots() {
        assert_eq!(root_key(Path::new("C:\\")), "c:/");
        assert_eq!(root_key(Path::new("C:/")), "c:/");
        assert_eq!(root_key(Path::new("/")), "/");
        assert_eq!(root_key(Path::new("")), "");
    }

    #[test]
    fn installation_serde_round_trip() {
        let now = Utc::now();
        let inst = Installation {
            id: InstallationId::new(),
            edition: Edition::Legacy,
            platform: DistributionPlatform::Steam,
            display_name: "GTA V".into(),
            root_path: PathBuf::from("/tmp/x"),
            root_key: root_key(Path::new("/tmp/x")),
            exe_version: None,
            status: InstallationStatus::Ok,
            baseline: BaselineTrust::None,
            created_at: now,
            updated_at: now,
        };
        let json = serde_json::to_string(&inst).unwrap();
        let back: Installation = serde_json::from_str(&json).unwrap();
        assert_eq!(back, inst);
    }
}
