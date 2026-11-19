//! Dependencies between mods of the same installation (schema v2).

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use gmm_domain::{InstallationId, ModId};
use rusqlite::params;

use super::{is_conflict, parse_col};
use crate::database::Database;
use crate::error::{Error, Result};
use crate::time::to_db;

impl Database {
    /// Records that `mod_id` requires `depends_on`.
    ///
    /// Errors: `NotFound` (either mod), `InvalidState` (different installations, self
    /// dependency, or `depends_on` already depends on `mod_id` transitively), `Conflict`
    /// (the edge already exists).
    pub fn add_dependency(
        &self,
        mod_id: ModId,
        depends_on: ModId,
        now: DateTime<Utc>,
    ) -> Result<()> {
        if mod_id == depends_on {
            return Err(Error::InvalidState {
                what: "a mod cannot depend on itself".to_owned(),
            });
        }
        let a = self
            .get_mod(mod_id)?
            .ok_or(Error::NotFound { what: "mod" })?;
        let b = self
            .get_mod(depends_on)?
            .ok_or(Error::NotFound { what: "mod" })?;
        if a.installation_id != b.installation_id {
            return Err(Error::InvalidState {
                what: "dependencies must stay within one installation".to_owned(),
            });
        }
        if self.dependencies_of(mod_id)?.contains(&depends_on) {
            return Err(Error::Conflict {
                what: "mod dependency",
            });
        }
        // Cycle check: does `depends_on` already reach `mod_id`?
        let mut seen = HashSet::new();
        let mut stack = vec![depends_on];
        while let Some(cur) = stack.pop() {
            if cur == mod_id {
                return Err(Error::InvalidState {
                    what: "the dependency would create a cycle".to_owned(),
                });
            }
            if seen.insert(cur) {
                stack.extend(self.dependencies_of(cur)?);
            }
        }
        self.conn()
            .execute(
                "INSERT INTO mod_dependencies (mod_id, depends_on_mod_id, created_at) \
                 VALUES (?1, ?2, ?3)",
                params![mod_id.to_string(), depends_on.to_string(), to_db(now)],
            )
            .map_err(|e| {
                if is_conflict(&e) {
                    Error::Conflict {
                        what: "mod dependency",
                    }
                } else {
                    e.into()
                }
            })?;
        Ok(())
    }

    /// Removes a dependency; `NotFound` when it does not exist.
    pub fn remove_dependency(&self, mod_id: ModId, depends_on: ModId) -> Result<()> {
        let n = self.conn().execute(
            "DELETE FROM mod_dependencies WHERE mod_id = ?1 AND depends_on_mod_id = ?2",
            params![mod_id.to_string(), depends_on.to_string()],
        )?;
        if n == 0 {
            return Err(Error::NotFound {
                what: "mod dependency",
            });
        }
        Ok(())
    }

    /// Mods that `mod_id` directly requires, ordered by id.
    pub fn dependencies_of(&self, mod_id: ModId) -> Result<Vec<ModId>> {
        self.mod_id_column(
            "SELECT depends_on_mod_id FROM mod_dependencies WHERE mod_id = ?1 \
             ORDER BY depends_on_mod_id",
            mod_id,
            "depends_on_mod_id",
        )
    }

    /// Mods that directly require `mod_id`, ordered by id.
    pub fn dependents_of(&self, mod_id: ModId) -> Result<Vec<ModId>> {
        self.mod_id_column(
            "SELECT mod_id FROM mod_dependencies WHERE depends_on_mod_id = ?1 ORDER BY mod_id",
            mod_id,
            "mod_id",
        )
    }

    /// Every `(mod, depends_on)` edge among the mods of an installation.
    pub fn dependency_edges(&self, installation_id: InstallationId) -> Result<Vec<(ModId, ModId)>> {
        let mut stmt = self.conn().prepare(
            "SELECT d.mod_id, d.depends_on_mod_id FROM mod_dependencies d \
             JOIN mods m ON m.id = d.mod_id WHERE m.installation_id = ?1 \
             ORDER BY d.mod_id, d.depends_on_mod_id",
        )?;
        let rows = stmt.query_map([installation_id.to_string()], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (a, b) = row?;
            out.push((
                parse_col("mod_dependencies", "mod_id", &a)?,
                parse_col("mod_dependencies", "depends_on_mod_id", &b)?,
            ));
        }
        Ok(out)
    }

    fn mod_id_column(&self, sql: &str, mod_id: ModId, column: &'static str) -> Result<Vec<ModId>> {
        let mut stmt = self.conn().prepare(sql)?;
        let rows = stmt.query_map([mod_id.to_string()], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(parse_col("mod_dependencies", column, &row?)?);
        }
        Ok(out)
    }
}
