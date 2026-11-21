//! Safe ZIP staging for ModSwitch. Every archive is treated as hostile: it is fully
//! validated, then extracted into a private staging directory, never into a game folder.

pub mod error;
pub mod limits;
pub mod mapping;
pub mod stage;
mod validate;

pub use error::{Error, Result};
pub use limits::Limits;
pub use mapping::{map_to_game, suggest_root};
pub use stage::{StagedArchive, StagedEntry, stage_zip};
