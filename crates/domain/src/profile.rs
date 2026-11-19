//! Profiles: named sets of desired mod states.

use crate::{InstallationId, ModId, ProfileId};
use chrono::{DateTime, Utc};

/// Name of the computed profile in which every managed mod is disabled.
pub const VANILLA_PROFILE_NAME: &str = "Vanilla";

/// Suggested names offered when creating a profile.
pub const SUGGESTED_PROFILE_NAMES: [&str; 5] = [
    "Visual Overhaul",
    "Vehicle Collection",
    "Immersive Gameplay",
    "World Expansion",
    "Complete Experience",
];

persisted_enum!(
    /// State a profile wants a mod to be in.
    DesiredState, "desired state" {
        Enabled => "enabled",
        Disabled => "disabled",
    }
);

/// A stored profile.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Profile {
    /// Identifier.
    pub id: ProfileId,
    /// Owning installation.
    pub installation_id: InstallationId,
    /// Unique name within the installation.
    pub name: String,
    /// Creation time.
    pub created_at: DateTime<Utc>,
    /// Last update time.
    pub updated_at: DateTime<Utc>,
}

/// Desired state of one mod inside a profile.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProfileEntry {
    /// Owning profile.
    pub profile_id: ProfileId,
    /// Target mod.
    pub mod_id: ModId,
    /// Desired state.
    pub desired: DesiredState,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desired_contract() {
        crate::macros::assert_enum_contract(
            &DesiredState::ALL,
            &["enabled", "disabled"],
            DesiredState::as_str,
        );
    }

    #[test]
    fn names() {
        assert_eq!(VANILLA_PROFILE_NAME, "Vanilla");
        assert_eq!(SUGGESTED_PROFILE_NAMES.len(), 5);
        assert!(!SUGGESTED_PROFILE_NAMES.contains(&VANILLA_PROFILE_NAME));
    }

    #[test]
    fn serde_round_trip() {
        let now = Utc::now();
        let p = Profile {
            id: ProfileId::new(),
            installation_id: InstallationId::new(),
            name: "Visual Overhaul".into(),
            created_at: now,
            updated_at: now,
        };
        let back: Profile = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(back, p);
        let e = ProfileEntry {
            profile_id: p.id,
            mod_id: ModId::new(),
            desired: DesiredState::Enabled,
        };
        let back: ProfileEntry = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(back, e);
    }
}
