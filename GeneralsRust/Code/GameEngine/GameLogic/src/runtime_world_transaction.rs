//! Native save admission keeps a freshly constructed match detached until
//! its snapshot, post-process state and presentation have been validated.
//! This boundary does not serialize subsystem state or change the save format.

use crate::system::engine_stores::{self, WorldServices};
use crate::team::TeamFactoryDeferredEffects;

/// Whether the specified world's legacy team callbacks are held for native
/// save admission.
pub fn world_runtime_staging_active_in(services: &WorldServices) -> bool {
    services.team_factory().save_stage_active()
}

/// A compatibility reader for the standalone Core update path.
pub fn world_runtime_staging_active() -> bool {
    engine_stores::try_active_services()
        .is_some_and(|services| world_runtime_staging_active_in(&services))
}

/// Native map admission executes its authored actions against the receiving
/// interpreter. Any deferred Core callback indicates a second execution path.
fn validate_native_team_effects(effects: &[TeamFactoryDeferredEffects]) -> Result<(), String> {
    let (create, generic) = effects
        .iter()
        .fold((0usize, 0usize), |(create, generic), effect| {
            let (next_create, next_generic) = effect.pending_callback_counts();
            (
                create.saturating_add(next_create),
                generic.saturating_add(next_generic),
            )
        });
    if create != 0 || generic != 0 {
        return Err(format!(
            "native staged world has unexpected Core team callbacks: {create} create, {generic} generic"
        ));
    }
    Ok(())
}

mod host;
pub use host::{CheckedNativeRuntimeWorld, HostRuntimeWorldStage, StagedHostRuntimeWorld};

#[cfg(test)]
#[path = "runtime_world_transaction/native_team_preflight_tests.rs"]
mod native_team_preflight_tests;
