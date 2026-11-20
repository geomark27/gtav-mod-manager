//! Mods and their file entries.

use crate::{Category, Error, InstallationId, ModEntryId, ModId, RelPath, Result, Sha256};
use chrono::{DateTime, Utc};

/// Maximum mod name length, in characters.
const MAX_NAME_CHARS: usize = 120;

persisted_enum!(
    /// Lifecycle state of a mod.
    ModState, "mod state" {
        Enabled => "enabled",
        Disabled => "disabled",
        Drifted => "drifted",
        Error => "error",
    }
);

persisted_enum!(
    /// How a mod is installed into the game.
    InstallMechanism, "install mechanism" {
        LooseFiles => "loose_files",
    }
);

persisted_enum!(
    /// Where a mod came from (provenance).
    ModSourceKind, "mod source kind" {
        Recognized => "recognized",
        Grouped => "grouped",
        Manual => "manual",
        ZipImport => "zip_import",
    }
);

/// Provenance of a mod: how it entered the app and what it was made from.
///
/// `detail` is the signature id (`recognized`) or the archive file name (`zip_import`);
/// `sha256` is the archive hash of a ZIP import; `version` is the signature or PE version when
/// known. Mods created before provenance existed are `manual`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ModSource {
    /// How the mod entered the app.
    pub kind: ModSourceKind,
    /// Signature id or archive file name.
    #[serde(default)]
    pub detail: Option<String>,
    /// SHA-256 of the imported archive.
    #[serde(default)]
    pub sha256: Option<Sha256>,
    /// Version of the mod when known.
    #[serde(default)]
    pub version: Option<String>,
}

impl ModSource {
    /// Files picked and adopted by hand.
    pub const fn manual() -> Self {
        Self {
            kind: ModSourceKind::Manual,
            detail: None,
            sha256: None,
            version: None,
        }
    }

    /// Imported from the archive `archive_name` whose content hash is `sha256`.
    pub fn zip_import(archive_name: impl Into<String>, sha256: Option<Sha256>) -> Self {
        let name = archive_name.into();
        Self {
            kind: ModSourceKind::ZipImport,
            detail: Some(name).filter(|n| !n.trim().is_empty()),
            sha256,
            version: None,
        }
    }
}

impl Default for ModSource {
    fn default() -> Self {
        Self::manual()
    }
}

/// Trims a mod name and rejects empty or over-long names (120 characters max).
pub fn validate_mod_name(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(Error::InvalidName {
            reason: "name is empty",
        });
    }
    if trimmed.chars().count() > MAX_NAME_CHARS {
        return Err(Error::InvalidName {
            reason: "name is longer than 120 characters",
        });
    }
    Ok(trimmed.to_owned())
}

/// A mod: a named set of files adopted into the vault.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Mod {
    /// Identifier.
    pub id: ModId,
    /// Owning installation.
    pub installation_id: InstallationId,
    /// Unique name within the installation.
    pub name: String,
    /// Category.
    pub category: Category,
    /// Current state.
    pub state: ModState,
    /// Install mechanism.
    pub mechanism: InstallMechanism,
    /// Free-form notes.
    pub notes: Option<String>,
    /// Where the mod came from.
    #[serde(default)]
    pub source: ModSource,
    /// Creation time.
    pub created_at: DateTime<Utc>,
    /// Last update time.
    pub updated_at: DateTime<Utc>,
}

/// One file of a mod.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ModEntry {
    /// Identifier.
    pub id: ModEntryId,
    /// Owning mod.
    pub mod_id: ModId,
    /// Target path inside the installation.
    pub rel_path: RelPath,
    /// Content hash (vault object).
    pub sha256: Sha256,
    /// Size in bytes.
    pub size: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_contract() {
        crate::macros::assert_enum_contract(
            &ModState::ALL,
            &["enabled", "disabled", "drifted", "error"],
            ModState::as_str,
        );
    }

    #[test]
    fn mechanism_contract() {
        crate::macros::assert_enum_contract(
            &InstallMechanism::ALL,
            &["loose_files"],
            InstallMechanism::as_str,
        );
    }

    #[test]
    fn source_kind_contract() {
        crate::macros::assert_enum_contract(
            &ModSourceKind::ALL,
            &["recognized", "grouped", "manual", "zip_import"],
            ModSourceKind::as_str,
        );
    }

    #[test]
    fn source_constructors_and_defaults() {
        assert_eq!(ModSource::default(), ModSource::manual());
        assert_eq!(ModSource::manual().kind, ModSourceKind::Manual);
        let z = ModSource::zip_import("Mod.zip", None);
        assert_eq!(z.kind, ModSourceKind::ZipImport);
        assert_eq!(z.detail.as_deref(), Some("Mod.zip"));
        assert_eq!(ModSource::zip_import("  ", None).detail, None);
        // A mod serialized before provenance existed reads back as manual.
        let old: ModSource = serde_json::from_str(r#"{"kind":"manual"}"#).unwrap();
        assert_eq!(old, ModSource::manual());
    }

    #[test]
    fn name_validation() {
        assert_eq!(validate_mod_name("  Cool Mod \n").unwrap(), "Cool Mod");
        assert!(matches!(
            validate_mod_name(""),
            Err(Error::InvalidName { .. })
        ));
        assert!(matches!(
            validate_mod_name("   \t"),
            Err(Error::InvalidName { .. })
        ));
        assert!(validate_mod_name(&"a".repeat(120)).is_ok());
        assert!(validate_mod_name(&"a".repeat(121)).is_err());
        assert!(validate_mod_name(&format!("  {}  ", "a".repeat(120))).is_ok());
    }

    #[test]
    fn mod_serde_round_trip() {
        let now = Utc::now();
        let m = Mod {
            id: ModId::new(),
            installation_id: InstallationId::new(),
            name: "x".into(),
            category: Category::Vehicles,
            state: ModState::Enabled,
            mechanism: InstallMechanism::LooseFiles,
            notes: None,
            source: ModSource::zip_import("m.zip", Some(Sha256::from_bytes([2; 32]))),
            created_at: now,
            updated_at: now,
        };
        let back: Mod = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
        assert_eq!(back, m);
        let e = ModEntry {
            id: ModEntryId::new(),
            mod_id: m.id,
            rel_path: RelPath::parse("a/b.dll").unwrap(),
            sha256: Sha256::from_bytes([1; 32]),
            size: 3,
        };
        let back: ModEntry = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(back, e);
    }
}
