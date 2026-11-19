//! Dependencies between mods: add, remove and the dependency-aware preview.

use gmm_domain::Mod;
use serde::Serialize;
use ts_rs::TS;

use crate::app::App;
use crate::error::{AppError, ErrorCode};
use crate::mods::{ModDetailDto, mod_not_found, parse_mod_id};
use crate::toggle::{PlanPreviewDto, ensure_usable, plan_preview, set_states_plan};

/// A reference to another mod.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModRefDto {
    /// The mod.
    pub mod_id: String,
    /// Its name.
    pub name: String,
}

impl App {
    /// Records that `mod_id` requires `depends_on_mod_id` and returns the updated mod.
    ///
    /// Self dependencies, cycles, mods of different installations and duplicate edges are
    /// refused with `INVALID_INPUT`.
    pub fn add_dependency(
        &self,
        mod_id: &str,
        depends_on_mod_id: &str,
    ) -> Result<ModDetailDto, AppError> {
        let id = parse_mod_id(mod_id)?;
        let dep = parse_mod_id(depends_on_mod_id)?;
        {
            let db = self.lock_db()?;
            db.add_dependency(id, dep, chrono::Utc::now())
                .map_err(|e| match e {
                    gmm_store::Error::Conflict { .. } => {
                        AppError::new(ErrorCode::InvalidInput, "This dependency already exists.")
                    }
                    other => other.into(),
                })?;
        }
        self.get_mod(mod_id)
    }

    /// Removes a dependency and returns the updated mod.
    pub fn remove_dependency(
        &self,
        mod_id: &str,
        depends_on_mod_id: &str,
    ) -> Result<ModDetailDto, AppError> {
        let id = parse_mod_id(mod_id)?;
        let dep = parse_mod_id(depends_on_mod_id)?;
        self.lock_db()?.remove_dependency(id, dep)?;
        self.get_mod(mod_id)
    }

    /// Dry run of enabling or disabling a mod *with* its dependency closure: the required
    /// mods when enabling, the dependents when disabling. Reads only the database state.
    pub fn preview_set_mod_enabled(
        &self,
        mod_id: &str,
        enabled: bool,
    ) -> Result<PlanPreviewDto, AppError> {
        let id = parse_mod_id(mod_id)?;
        let installation_id = self
            .lock_db()?
            .get_mod(id)?
            .ok_or_else(mod_not_found)?
            .installation_id;
        let inst = self.load_validated(installation_id)?;
        let (ids, mods) = {
            let db = self.lock_db()?;
            let ids = gmm_engine::dependency_closure(&db, installation_id, &[id], enabled)?;
            let mods: Vec<Mod> = ids
                .iter()
                .map(|m| db.get_mod(*m)?.ok_or_else(mod_not_found))
                .collect::<Result<_, AppError>>()?;
            (ids, mods)
        };
        ensure_usable(&mods)?;
        let plan = self.with_engine(&inst, |ctx| set_states_plan(ctx, &ids, enabled))?;
        Ok(plan_preview(&plan, &mods))
    }
}
