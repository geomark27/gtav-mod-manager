//! Installation discovery and structural validation for ModSwitch.
//!
//! Everything OS-specific goes through [`OsProbe`]; everything file-system related goes
//! through `gmm_platform::Fs`. Validation never modifies an installation except for the
//! short-lived write probe.

pub mod baseline;
pub mod catalog;
pub mod discovery;
pub mod error;
pub mod glob;
pub mod os;
pub mod processes;
pub mod recognize;
pub mod scan;
pub mod validation;
mod walk;

pub use baseline::{BaselineEntryData, capture_baseline};
pub use catalog::{Catalog, Catalogs, ModPattern};
pub use discovery::{DiscoveredInstallation, DiscoveryHints, discover};
pub use error::{Error, Result};
pub use glob::Glob;
pub use os::{Hive, OsProbe, SystemOs, testing};
pub use processes::{
    GTAV_PROCESSES, RDR2_PROCESSES, ROCKSTAR_PROCESSES, guarded_processes, running_guarded,
};
pub use recognize::{
    AmbiguousFile, Confidence, ProposedGroup, Recognition, RecognizedMod, Signature, Signatures,
    group_candidates, recognize,
};
pub use scan::{
    BaselineRef, CachedHash, ManagedRef, ScanContext, ScanOutcome, ScanProgress, ScanStats, scan,
};
pub use validation::{ValidationProblem, ValidationReport, validate};
