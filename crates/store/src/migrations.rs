//! Schema migration runner driven by `PRAGMA user_version`.

use rusqlite::Connection;

use crate::error::{Error, Result};

/// One schema migration.
struct Migration {
    /// Schema version reached after it.
    version: u32,
    /// SQL applied in one transaction.
    sql: &'static str,
    /// Rebuilds a table that others reference: it runs with foreign keys OFF, so that dropping
    /// the old table does not cascade, and must leave `PRAGMA foreign_key_check` empty.
    rebuilds_table: bool,
}

/// Ordered migrations.
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: include_str!("../migrations/0001_initial.sql"),
        rebuilds_table: false,
    },
    Migration {
        version: 2,
        sql: include_str!("../migrations/0002_dependencies.sql"),
        rebuilds_table: false,
    },
    Migration {
        version: 3,
        sql: include_str!("../migrations/0003_provenance_restore_point.sql"),
        rebuilds_table: false,
    },
    Migration {
        version: 4,
        sql: include_str!("../migrations/0004_rdr2_edition.sql"),
        rebuilds_table: true,
    },
];

/// Highest schema version this build understands.
pub const LATEST_SCHEMA_VERSION: u32 = 4;

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
    for m in MIGRATIONS.iter().filter(|m| m.version > found) {
        if m.rebuilds_table {
            // The pragma is a no-op inside a transaction: switch it before and after.
            conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
            let applied = apply(conn, m);
            conn.execute_batch("PRAGMA foreign_keys = ON;")?;
            applied?;
        } else {
            apply(conn, m)?;
        }
    }
    Ok(())
}

fn apply(conn: &mut Connection, m: &Migration) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute_batch(m.sql)?;
    if m.rebuilds_table {
        let violations: i64 =
            tx.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
                r.get(0)
            })?;
        if violations != 0 {
            // Dropping `tx` rolls the migration back.
            return Err(Error::InvalidState {
                what: format!(
                    "migration {} left {violations} foreign key violation(s)",
                    m.version
                ),
            });
        }
    }
    tx.execute_batch(&format!("PRAGMA user_version = {}", m.version))?;
    tx.commit()?;
    Ok(())
}
