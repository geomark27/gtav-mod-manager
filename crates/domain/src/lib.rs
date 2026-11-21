//! Domain model of the mod manager (GTA V and Red Dead Redemption 2): entities, value objects and state machines. No I/O.

#[macro_use]
mod macros;

pub mod artifact;
pub mod category;
pub mod edition;
pub mod error;
pub mod game;
pub mod ids;
pub mod installation;
pub mod inventory;
pub mod mods;
pub mod profile;
pub mod rel_path;
pub mod sha256;
pub mod transaction;

pub use artifact::{Artifact, ArtifactKind, parse_artifact, tmp_name, tomb_name, tx_suffix};
pub use category::Category;
pub use edition::Edition;
pub use error::{Error, Result};
pub use game::Game;
pub use ids::{InstallationId, ModEntryId, ModId, ProfileId, ScanId, TransactionId};
pub use installation::{
    BaselineTrust, DistributionPlatform, Installation, InstallationStatus, root_key,
};
pub use inventory::{Classification, Evidence, InventoryItem};
pub use mods::{
    InstallMechanism, Mod, ModEntry, ModSource, ModSourceKind, ModState, validate_mod_name,
};
pub use profile::{
    DesiredState, Profile, ProfileEntry, ProfileKind, SUGGESTED_PROFILE_NAMES, VANILLA_PROFILE_NAME,
};
pub use rel_path::RelPath;
pub use sha256::Sha256;
pub use transaction::{StepKind, StepStatus, TransactionKind, TransactionStatus};
