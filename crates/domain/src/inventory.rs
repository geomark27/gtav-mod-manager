//! Inventory items, classifications and evidence.

use crate::{Category, InstallationId, ModId, RelPath, ScanId, Sha256};
use chrono::{DateTime, Utc};

persisted_enum!(
    /// Classification of a scanned file.
    Classification, "classification" {
        Original => "original",
        OriginalModified => "original_modified",
        Candidate => "candidate",
        Unknown => "unknown",
        Managed => "managed",
        Drifted => "drifted",
        EngineArtifact => "engine_artifact",
    }
);

impl Classification {
    /// True only for files the engine may act on (`Managed`).
    pub const fn engine_may_act(self) -> bool {
        matches!(self, Self::Managed)
    }

    /// True for files the user may adopt into a mod (`Candidate`, `Unknown`).
    pub const fn can_be_adopted(self) -> bool {
        matches!(self, Self::Candidate | Self::Unknown)
    }
}

/// One reason behind a classification.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum Evidence {
    /// Path is in the catalog of original files.
    CatalogOriginal,
    /// Content matches the baseline.
    BaselineMatch,
    /// Content differs from the baseline.
    BaselineMismatch,
    /// PE file signed by a publisher.
    SignedBy {
        /// Signer subject.
        publisher: String,
    },
    /// PE file without a valid signature.
    Unsigned,
    /// Matches a known mod pattern.
    ModPattern {
        /// Pattern name.
        pattern: String,
    },
    /// File is a symlink or junction.
    ReparsePoint,
    /// File has more than one hard link.
    HardLinked,
    /// File belongs to a managed mod.
    ManagedBy {
        /// Owning mod.
        mod_id: ModId,
    },
}

impl Evidence {
    /// True when this evidence forbids adopting the file.
    pub const fn blocks_adoption(&self) -> bool {
        matches!(
            self,
            Self::CatalogOriginal
                | Self::BaselineMatch
                | Self::BaselineMismatch
                | Self::ReparsePoint
                | Self::HardLinked
        )
    }
}

/// A scanned file of an installation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InventoryItem {
    /// Owning installation.
    pub installation_id: InstallationId,
    /// Path relative to the installation root.
    pub rel_path: RelPath,
    /// Size in bytes.
    pub size: u64,
    /// Modification time, nanoseconds since the Unix epoch.
    pub mtime_ns: i64,
    /// Content hash, when computed.
    pub sha256: Option<Sha256>,
    /// Classification.
    pub classification: Classification,
    /// Category suggested by catalog patterns.
    pub suggested_category: Option<Category>,
    /// Reasons behind the classification.
    pub evidence: Vec<Evidence>,
    /// First time the file was seen.
    pub first_seen_at: DateTime<Utc>,
    /// Last time the file was seen.
    pub last_seen_at: DateTime<Utc>,
    /// Scan that last saw the file.
    pub last_scan_id: Option<ScanId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification_contract() {
        crate::macros::assert_enum_contract(
            &Classification::ALL,
            &[
                "original",
                "original_modified",
                "candidate",
                "unknown",
                "managed",
                "drifted",
                "engine_artifact",
            ],
            Classification::as_str,
        );
    }

    #[test]
    fn engine_and_adoption_rules() {
        for c in Classification::ALL {
            assert_eq!(c.engine_may_act(), c == Classification::Managed, "{c}");
            assert_eq!(
                c.can_be_adopted(),
                matches!(c, Classification::Candidate | Classification::Unknown),
                "{c}"
            );
        }
    }

    #[test]
    fn evidence_blocking() {
        let blocking = [
            Evidence::CatalogOriginal,
            Evidence::BaselineMatch,
            Evidence::BaselineMismatch,
            Evidence::ReparsePoint,
            Evidence::HardLinked,
        ];
        let allowing = [
            Evidence::SignedBy {
                publisher: "x".into(),
            },
            Evidence::Unsigned,
            Evidence::ModPattern {
                pattern: "p".into(),
            },
            Evidence::ManagedBy {
                mod_id: ModId::new(),
            },
        ];
        assert!(blocking.iter().all(Evidence::blocks_adoption));
        assert!(allowing.iter().all(|e| !e.blocks_adoption()));
    }

    #[test]
    fn evidence_serde_is_internally_tagged() {
        let json = serde_json::to_string(&Evidence::CatalogOriginal).unwrap();
        assert_eq!(json, r#"{"kind":"catalog_original"}"#);
        let json = serde_json::to_string(&Evidence::SignedBy {
            publisher: "Acme".into(),
        })
        .unwrap();
        assert_eq!(json, r#"{"kind":"signed_by","publisher":"Acme"}"#);
        let id = ModId::new();
        let e = Evidence::ManagedBy { mod_id: id };
        let back: Evidence = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(back, e);
        let list: Vec<Evidence> =
            serde_json::from_str(r#"[{"kind":"unsigned"},{"kind":"mod_pattern","pattern":"asi"}]"#)
                .unwrap();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn item_serde_round_trip() {
        let now = Utc::now();
        let item = InventoryItem {
            installation_id: InstallationId::new(),
            rel_path: RelPath::parse("scripts/a.asi").unwrap(),
            size: 10,
            mtime_ns: 1,
            sha256: None,
            classification: Classification::Candidate,
            suggested_category: Some(Category::ScriptsGameplay),
            evidence: vec![Evidence::Unsigned],
            first_seen_at: now,
            last_seen_at: now,
            last_scan_id: None,
        };
        let back: InventoryItem =
            serde_json::from_str(&serde_json::to_string(&item).unwrap()).unwrap();
        assert_eq!(back, item);
    }
}
