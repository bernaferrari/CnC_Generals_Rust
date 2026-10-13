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
    let _ = crate::game_logic::host_ai_request_log::drain();
    logic.attack_events.complete_boundary();
    let _ = crate::game_logic::host_bounce_land_log::drain();
    let _ = crate::game_logic::host_building_type_log::drain();

    let _ = crate::game_logic::host_command_set_log::drain();

    let _ = crate::game_logic::host_continuous_fire_log::drain();
    let _ = crate::game_logic::host_crush_vision_log::drain();
    let _ = crate::game_logic::host_death_type_log::drain();
    let _ = crate::game_logic::host_demo_mine_cheer_log::drain();
    let _ = crate::game_logic::host_destroy_log::drain();

    let _ = crate::game_logic::host_faerie_fire_log::drain();
    let _ = crate::game_logic::host_fire_intent_log::drain();
    logic
        .combat_system
        .fire_sound_loop_events
        .complete_boundary();
    let _ = crate::game_logic::host_formation_log::drain();
    let _ = crate::game_logic::host_fow_log::drain();
    let _ = crate::game_logic::host_ground_height_log::drain();
    let _ = crate::game_logic::host_hijacker_log::drain();

    let _ = crate::game_logic::host_locomotor_log::drain();
    let _ = crate::game_logic::host_max_health_log::drain();
    let _ = crate::game_logic::host_model_condition_log::drain();
    let _ = crate::game_logic::host_model_mesh_log::drain();
    let _ = logic.movement_events.drain_for_host_boundary();
    let _ = crate::game_logic::host_movement_log::drain();

    logic.ownership_events.complete_boundary();
    let _ = crate::game_logic::host_physics_motive_log::drain();

    let _ = crate::game_logic::host_production_door_log::drain();
    logic.production_events.complete_boundary();
    let _ = crate::game_logic::host_production_progress_log::drain();
    let _ = crate::game_logic::host_projectile_log::drain();

    let _ = crate::game_logic::host_rebuild_producer_log::drain();

    let _ = crate::game_logic::host_shock_stun_log::drain();
    let _ = crate::game_logic::host_sole_healing_log::drain();
    let _ = crate::game_logic::host_spawn_log::drain();

    let _ = crate::game_logic::host_stealth_delay_log::drain();

    let _ = crate::game_logic::host_stored_supplies_log::drain();

    let _ = crate::game_logic::host_turret_log::drain();

    let _ = crate::game_logic::host_weapon_bonus_log::drain();
    let _ = crate::game_logic::host_weapon_set_log::drain();
    let _ = crate::game_logic::host_weapon_slot_log::drain();
    let _ = crate::game_logic::host_weapon_stats_log::drain();
    logic.economy_events.complete_boundary();
}
