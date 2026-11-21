//! Test support for ModSwitch: fake game installations built inside a temp dir and
//! precise disk snapshots. Intended to be used only as a dev-dependency.

pub mod error;
pub mod fixtures;
pub mod install;
pub mod snapshot;

pub use error::{Error, Result};
pub use fixtures::{ENHANCED_BASE_FILES, LEGACY_BASE_FILES, base_files, fake_bytes};
pub use install::{FakeInstall, FakeInstallBuilder};
pub use snapshot::{Change, Entry, Snapshot};
