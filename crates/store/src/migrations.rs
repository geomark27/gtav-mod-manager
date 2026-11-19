//! Schema migration runner driven by `PRAGMA user_version`.

use rusqlite::Connection;

use crate::error::{Error, Result};

/// Ordered list of `(version, sql)` migrations.
const MIGRATIONS: &[(u32, &str)] = &[
    (1, include_str!("../migrations/0001_initial.sql")),
    (2, include_str!("../migrations/0002_dependencies.sql")),
];

/// Highest schema version this build understands.
pub const LATEST_SCHEMA_VERSION: u32 = 2;

/// Reads `PRAGMA user_version`.
pub(crate) fn current_version(conn: &Connection) -> Result<u32> {
    let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    Ok(u32::try_from(v).unwrap_or(u32::MAX))
}

/// Applies every pending migration, each in its own transaction.
pub(crate) fn run(conn: &mut Connection) -> Result<()> {
    let found = current_version(conn)?;
    if found > LATEST_SCHEMA_VERSION {
        return Err(Error::SchemaTooNew {
            found,
            supported: LATEST_SCHEMA_VERSION,
        });
    }
    for (version, sql) in MIGRATIONS.iter().filter(|(v, _)| *v > found) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.execute_batch(&format!("PRAGMA user_version = {version}"))?;
        tx.commit()?;
    }
    Ok(())
}
