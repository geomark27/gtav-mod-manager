//! Repositories: typed access to the tables, implemented as methods on `Database`.

pub(crate) mod baseline;
pub(crate) mod dependencies;
pub(crate) mod installations;
pub(crate) mod inventory;
pub(crate) mod managed;
pub(crate) mod mods;
pub(crate) mod profiles;
pub(crate) mod scans;
pub(crate) mod settings;
pub(crate) mod transactions;
pub(crate) mod vault;

use crate::error::{Error, Result};

/// True for UNIQUE and PRIMARY KEY constraint violations.
pub(crate) fn is_conflict(e: &rusqlite::Error) -> bool {
    matches!(
        e,
        rusqlite::Error::SqliteFailure(f, _)
            if f.code == rusqlite::ErrorCode::ConstraintViolation
                && (f.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
                    || f.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_PRIMARYKEY)
    )
}

/// Parses a stored text value into its domain type; `Corrupt` when it does not parse.
pub(crate) fn parse_col<T: std::str::FromStr>(
    table: &'static str,
    column: &'static str,
    value: &str,
) -> Result<T> {
    value.parse().map_err(|_| Error::Corrupt {
        table,
        column,
        value: value.to_owned(),
    })
}

/// Converts a stored integer to `u64`; `Corrupt` when negative.
pub(crate) fn to_u64(table: &'static str, column: &'static str, v: i64) -> Result<u64> {
    u64::try_from(v).map_err(|_| Error::Corrupt {
        table,
        column,
        value: v.to_string(),
    })
}
