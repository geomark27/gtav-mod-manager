//! Dependency rules between mods: a mod is never left enabled while a dependency is not.

use std::collections::{HashMap, HashSet};

use gmm_domain::{InstallationId, ModId, ModState};
use gmm_store::Database;

use crate::context::EngineContext;
use crate::error::{Error, Result};

/// Validates the **final** states of a plan against the dependency graph.
///
/// `changes` are the states the plan sets; every other mod keeps its stored state. A mod that
/// ends `Enabled` needs all its dependencies `Enabled`; a mod that ends `Disabled` must not
/// have a dependent that ends `Enabled`. Only mods whose state the plan changes are checked, so
/// a pre-existing violation never blocks an unrelated operation.
pub(crate) fn check_dependency_rules(
    ctx: &EngineContext<'_>,
    changes: &[(ModId, ModState)],
) -> Result<()> {
    let overrides: HashMap<ModId, ModState> = changes.iter().copied().collect();
    let name_of = |id: ModId| -> Result<String> {
        Ok(ctx
            .db
            .get_mod(id)?
            .map_or_else(|| id.to_string(), |m| m.name))
    };
    let final_state = |id: ModId| -> Result<Option<ModState>> {
        match overrides.get(&id) {
            Some(s) => Ok(Some(*s)),
            None => Ok(ctx.db.get_mod(id)?.map(|m| m.state)),
        }
    };
    for (id, state) in changes {
        match state {
            ModState::Enabled => {
                for dep in ctx.db.dependencies_of(*id)? {
                    if final_state(dep)? != Some(ModState::Enabled) {
                        return Err(Error::DependencyDisabled {
                            r#mod: name_of(*id)?,
                            dependency: name_of(dep)?,
                        });
                    }
                }
            }
            ModState::Disabled => {
                for dependent in ctx.db.dependents_of(*id)? {
                    if final_state(dependent)? == Some(ModState::Enabled) {
                        return Err(Error::DependentEnabled {
                            r#mod: name_of(*id)?,
                            dependent: name_of(dependent)?,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// The mods plus everything that has to change with them, so the app can offer "also
/// enable/disable N mods".
///
/// With `enable`, adds all transitive dependencies; otherwise all transitive dependents. The
/// requested mods come first (in the given order, without duplicates), followed by the added
/// ones in discovery order. Only the edges of `installation_id` are considered.
pub fn dependency_closure(
    db: &Database,
    installation_id: InstallationId,
    mod_ids: &[ModId],
    enable: bool,
) -> Result<Vec<ModId>> {
    let mut next: HashMap<ModId, Vec<ModId>> = HashMap::new();
    for (m, dep) in db.dependency_edges(installation_id)? {
        let (from, to) = if enable { (m, dep) } else { (dep, m) };
        next.entry(from).or_default().push(to);
    }
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let mut queue: std::collections::VecDeque<ModId> = std::collections::VecDeque::new();
    for id in mod_ids {
        if seen.insert(*id) {
            out.push(*id);
            queue.push_back(*id);
        }
    }
    while let Some(cur) = queue.pop_front() {
        for n in next.get(&cur).into_iter().flatten() {
            if seen.insert(*n) {
                out.push(*n);
                queue.push_back(*n);
            }
        }
    }
    Ok(out)
}
