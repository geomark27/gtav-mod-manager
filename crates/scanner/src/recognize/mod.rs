//! Mod recognition (Phase 4): known-mod signatures (R1) and grouping of the remaining
//! candidates (R2). Recognition only proposes; nothing is managed until the user adopts it.

/// `(stem, contents)` of an embedded signature file.
macro_rules! sig {
    ($id:literal) => {
        (
            $id,
            include_str!(concat!("../../catalog/mods/", $id, ".toml")),
        )
    };
}

mod group;
mod matcher;
mod signature;

pub use group::{ProposedGroup, group_candidates, title_case};
pub use matcher::{AmbiguousFile, Confidence, Recognition, RecognizedMod, recognize};
pub use signature::{Signature, Signatures};
