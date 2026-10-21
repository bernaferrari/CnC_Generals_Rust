//! Complete an ordinary Main-owned frame, then copy its observations.
//!
//! No shadow tick or reverse write participates in this boundary;
//! the projection cannot select another simulation authority.

use super::{GameWorldShadow, presentation_entity_count_from_shadow};
use crate::game_logic::{GameLogic, HostSupportOp};

/// Admit required owner effects before the next fixed step. Only afterward may
/// the observer copy completed values from an immutable Main borrow.
pub(crate) fn run_owned_host_boundary(
    shadow: Option<&mut GameWorldShadow>,
    logic: &mut GameLogic,
) -> usize {
    // A completed step has released its compatibility store/policy scopes.
    // Death callbacks still use those adapters, so resolve this owner for the
    // bounded admission operation and restore the previous context afterward.
    // Neither scope publishes or lends a shadow as writable simulation state.
    let stores = std::sync::Arc::clone(&logic.world_services);
    gamelogic::system::engine_stores::with_world_services(&stores, || {
        // Ordered owner admission distinguishes already-applied damage/
        // healing observations from required pending effects.
        let _ = logic.health_events.drain_for_host_boundary();
        logic.apply_host_support_op(HostSupportOp::ProcessDestroyListIfNeeded);
        freeze_host_observations(logic);

        if let Some(shadow) = shadow {
            shadow.sync_from_host(logic);
            presentation_entity_count_from_shadow(shadow)
        } else {
            0
        }
    })
}

/// These records describe state already committed by Main. Movement completes
/// into its owner's FIFO output; the other channels still use drain() to retain
/// attack/owner/production/fire-sound/economy presentation receipts. Mirror-only records
/// have no remaining default gameplay consumer: the observer copies the owner.
///
/// Health is admitted above. Projectile work stays in CombatSystem (the
/// fire-spawn experiment is never enabled on this path). Production ready work
/// is already applied by its owner. EVA stays pending for PresentationFrame's
/// consuming take_last_drain(), which intentionally drains its own queue.
/// Construction completion also stays pending for PresentationFrame's direct
/// drain; unlike production completion it has no LAST_DRAIN transport.
fn freeze_host_observations(logic: &mut GameLogic) {
    logic.attack_events.complete_boundary();

    #[cfg(test)]
    let _ = crate::game_logic::host_destroy_log::drain();

    #[cfg(test)]
    let _ = crate::game_logic::host_fire_intent_log::drain();
    logic
        .combat_system
        .fire_sound_loop_events
        .complete_boundary();

    let _ = logic.movement_events.drain_for_host_boundary();

    logic.ownership_events.complete_boundary();

    logic.production_events.complete_boundary();
    let _ = crate::game_logic::host_production_progress_log::drain();

    let _ = crate::game_logic::host_spawn_log::drain();

    logic.economy_events.complete_boundary();
}
