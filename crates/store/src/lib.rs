//! SQLite persistence for ModSwitch: connection, pragmas and schema migrations.

mod database;
mod error;
mod migrations;
mod repo;
mod time;

pub use database::{Database, PragmaSnapshot};
pub use error::{Error, Result};
pub use migrations::LATEST_SCHEMA_VERSION;
pub use repo::inventory::{InventoryFilter, InventoryPage, MAX_PAGE_SIZE};
pub use repo::managed::ManagedEntryRow;
pub use repo::scans::{ScanRecord, ScanStatus};
pub use repo::settings::SETTING_SELECTED_INSTALLATION;
pub use repo::transactions::{AdoptRecord, CommitEffects, NewStep, StepRow, TransactionRow};
pub use repo::vault::VaultObjectInfo;
