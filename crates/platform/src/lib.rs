//! OS abstractions for ModSwitch: the `Fs` trait, its real and fault-injecting
//! implementations, streaming hashing and application directories.

pub mod dirs;
pub mod error;
pub mod faulty_fs;
pub mod fs;
pub mod hash;
pub mod std_fs;

pub use dirs::AppDirs;
pub use error::{Error, Result};
pub use faulty_fs::{FaultMode, FaultPlan, FaultyFs, OpMatch};
pub use fs::{DirEntryInfo, EntryKind, FileMeta, Fs, FsOp};
pub use hash::{sha256_bytes, sha256_file, sha256_reader};
pub use std_fs::StdFs;
