//! Database connection: opening, pragmas and migrations.

use std::path::Path;

use rusqlite::Connection;

use crate::error::{Error, Result};
use crate::migrations;

/// Effective connection pragmas, for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PragmaSnapshot {
    /// `journal_mode` (e.g. `wal`).
    pub journal_mode: String,
    /// `synchronous` level (2 = FULL).
    pub synchronous: i64,
    /// Whether foreign key enforcement is on.
    pub foreign_keys: bool,
}

/// The application database.
#[derive(Debug)]
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Opens (creating if missing) the database file, applies pragmas and migrations.
    ///
    /// The parent directory must already exist.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
            && !parent.is_dir()
        {
            return Err(Error::ParentDirMissing {
                path: parent.to_path_buf(),
            });
        }
        let conn = Connection::open(path)?;
        Self::init(conn, true)
    }

    /// Opens a private in-memory database at the latest schema.
    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?, false)
    }

    fn init(mut conn: Connection, wal: bool) -> Result<Self> {
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;
        if wal {
            let mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
            if !mode.eq_ignore_ascii_case("wal") {
                return Err(Error::PragmaRejected {
                    pragma: "journal_mode",
                    value: mode,
                });
            }
        }
        conn.execute_batch("PRAGMA synchronous = FULL; PRAGMA foreign_keys = ON;")?;
        let snap = Self::snapshot(&conn)?;
        if snap.synchronous != 2 {
            return Err(Error::PragmaRejected {
                pragma: "synchronous",
                value: snap.synchronous.to_string(),
            });
        }
        if !snap.foreign_keys {
            return Err(Error::PragmaRejected {
                pragma: "foreign_keys",
                value: "off".to_owned(),
            });
        }
        migrations::run(&mut conn)?;
        Ok(Self { conn })
    }

    fn snapshot(conn: &Connection) -> Result<PragmaSnapshot> {
        let journal_mode: String = conn.query_row("PRAGMA journal_mode", [], |r| r.get(0))?;
        let synchronous: i64 = conn.query_row("PRAGMA synchronous", [], |r| r.get(0))?;
        let fk: i64 = conn.query_row("PRAGMA foreign_keys", [], |r| r.get(0))?;
        Ok(PragmaSnapshot {
            journal_mode,
            synchronous,
            foreign_keys: fk != 0,
        })
    }

    /// Returns the current schema version (`PRAGMA user_version`).
    pub fn schema_version(&self) -> Result<u32> {
        migrations::current_version(&self.conn)
    }

    /// Returns the pragmas in effect on this connection.
    pub fn pragma_snapshot(&self) -> Result<PragmaSnapshot> {
        Self::snapshot(&self.conn)
    }

    /// Raw connection for repositories.
    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }
}
