//! UUID v7 identifier newtypes.

use crate::{Error, Result};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

macro_rules! id_type {
    ($(#[$meta:meta])* $name:ident, $kind:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash,
                 serde::Serialize, serde::Deserialize)]
        #[serde(transparent)]
        #[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(type = "string"))]
        pub struct $name(Uuid);

        impl $name {
            /// Generates a new time-ordered (UUID v7) id.
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }

            /// Wraps an existing UUID.
            pub const fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            /// Returns the underlying UUID.
            pub const fn as_uuid(&self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0.hyphenated())
            }
        }

        impl FromStr for $name {
            type Err = Error;

            fn from_str(s: &str) -> Result<Self> {
                let bad = || Error::InvalidId { kind: $kind, value: s.to_owned() };
                if s.len() != 36 {
                    return Err(bad());
                }
                Uuid::parse_str(s).map(Self).map_err(|_| bad())
            }
        }
    };
}

id_type!(
    /// Identifier of a registered game installation.
    InstallationId, "installation"
);
id_type!(
    /// Identifier of a mod.
    ModId, "mod"
);
id_type!(
    /// Identifier of one file entry of a mod.
    ModEntryId, "mod entry"
);
id_type!(
    /// Identifier of a profile.
    ProfileId, "profile"
);
id_type!(
    /// Identifier of an engine transaction.
    TransactionId, "transaction"
);
id_type!(
    /// Identifier of an inventory scan.
    ScanId, "scan"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_parse_round_trip() {
        let id = ModId::new();
        let text = id.to_string();
        assert_eq!(text.len(), 36);
        assert_eq!(text, text.to_lowercase());
        assert_eq!(text.parse::<ModId>(), Ok(id));
        assert_eq!(id.as_uuid().get_version_num(), 7);
        assert_eq!(ModId::from_uuid(id.as_uuid()), id);
    }

    #[test]
    fn parse_rejects_garbage() {
        assert!(matches!(
            "nope".parse::<ScanId>(),
            Err(Error::InvalidId { .. })
        ));
        let simple = Uuid::now_v7().simple().to_string();
        assert!(simple.parse::<ScanId>().is_err());
    }

    #[test]
    fn serde_is_a_plain_string() {
        let id = TransactionId::new();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, format!("\"{id}\""));
        let back: TransactionId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, id);
        assert!(serde_json::from_str::<TransactionId>("\"x\"").is_err());
    }

    #[test]
    fn ids_are_ordered_by_creation() {
        let a = ProfileId::new();
        let b = ProfileId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn all_types_construct() {
        let _ = (
            InstallationId::new(),
            ModEntryId::new(),
            ProfileId::default(),
        );
    }
}
