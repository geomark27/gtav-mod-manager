//! Operations engine: the only component that changes game folders.
//!
//! Every operation is a journaled two-phase transaction (see `docs/architecture.md` section 7)
//! that rolls back on error and recovers after a crash at any point.

mod context;
mod deps;
mod drift;
mod error;
mod exec;
mod gc;
mod import;
mod journal;
mod plan;
mod reconcile;
mod recovery;
mod steps;
mod vault;

pub use context::{EngineContext, resolve};
pub use deps::dependency_closure;
pub use drift::{plan_readopt, plan_restore};
pub use error::{Error, Result};
pub use exec::execute;
pub use gc::{GcReport, vault_gc, vault_gc_candidates};
pub use import::import_to_vault;
pub use plan::{
    AdoptPayload, Plan, PlannedStep, TxSummary, plan_adopt, plan_disable, plan_enable,
    plan_register_imported, plan_set_states,
};
pub use reconcile::{DriftItem, DriftKind, reconcile};
pub use recovery::{RecoveryReport, recover};
pub use vault::Vault;
