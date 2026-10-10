//! Complete an ordinary Main-owned frame, then copy its observations.
//!
//! No shadow tick or reverse write participates in this boundary. Optional
//! authority experiments use the separate coupled session entrypoint.

use super::{
    GameWorldShadow, materialize_host_authority_logs, presentation_entity_count_from_shadow,
};
use crate::game_logic::game_logic::gameworld_authority::GameWorldAuthority;
use crate::game_logic::{GameLogic, HostSupportOp};

/// Admit required owner effects before the next fixed step. Only afterward may
/// the observer copy completed values from an immutable Main borrow.
pub(crate) fn run_owned_host_boundary(
    shadow: Option<&mut GameWorldShadow>,
    logic: &mut GameLogic,
) -> usize {
    assert_eq!(
        *logic.gameworld_authority(),
        GameWorldAuthority::DEFAULT_OFF,
        "an authority experiment requires its coupled boundary"
    );
    assert!(
        !super::shadow_coupled_tick_active(),
        "ordinary Main timers must run without coupled suppression"
    );

    // A completed step has released its compatibility store/policy scopes.
    // Death callbacks still use those adapters, so resolve this owner for the
    // bounded admission operation and restore the previous context afterward.
    // Neither scope publishes or lends a shadow as writable simulation state.
    let stores = std::sync::Arc::clone(&logic.engine_stores);
    super::with_gameworld_authority(GameWorldAuthority::DEFAULT_OFF, || {
        gamelogic::system::engine_stores::with_active_stores(&stores, || {
            // Ordered owner admission distinguishes already-applied damage/
            // healing observations from required pending effects.
            materialize_host_authority_logs(logic);
            logic.apply_host_support_op(HostSupportOp::ProcessDestroyListIfNeeded);
            freeze_host_observations();

            if let Some(shadow) = shadow {
                shadow.sync_from_host(logic);
                presentation_entity_count_from_shadow(shadow)
            } else {
                0
            }
        })
    })
}

/// These records describe state already committed by Main. drain(), rather
/// than clear(), preserves the existing presentation LAST_DRAIN receipts for
/// move/attack/owner/production/fire-sound/economy. Other mirror-only records
/// have no remaining default gameplay consumer: the observer copies the owner.
///
/// Health is admitted above. Projectile work stays in CombatSystem (the
/// fire-spawn experiment is never enabled on this path). Production ready work
/// is already applied by its owner. EVA stays pending for PresentationFrame's
/// consuming take_last_drain(), which intentionally drains its own queue.
/// Construction completion also stays pending for PresentationFrame's direct
/// drain; unlike production completion it has no LAST_DRAIN transport.
fn freeze_host_observations() {
    let _ = crate::game_logic::host_ai_attitude_log::drain();
    let _ = crate::game_logic::host_ai_decision_log::drain();
    let _ = crate::game_logic::host_ai_mood_log::drain();
    let _ = crate::game_logic::host_ai_request_log::drain();
    let _ = crate::game_logic::host_ai_state_log::drain();
    let _ = crate::game_logic::host_attack_log::drain();
    let _ = crate::game_logic::host_body_damage_log::drain();
    let _ = crate::game_logic::host_bounce_land_log::drain();
    let _ = crate::game_logic::host_building_type_log::drain();
    let _ = crate::game_logic::host_combat_attack_log::drain();
    let _ = crate::game_logic::host_command_set_log::drain();
    let _ = crate::game_logic::host_construction_progress_log::drain();
    let _ = crate::game_logic::host_contain_capacity_log::drain();
    let _ = crate::game_logic::host_contain_log::drain();
    let _ = crate::game_logic::host_continuous_fire_log::drain();
    let _ = crate::game_logic::host_crush_vision_log::drain();
    let _ = crate::game_logic::host_death_type_log::drain();
    let _ = crate::game_logic::host_demo_mine_cheer_log::drain();
    let _ = crate::game_logic::host_destroy_log::drain();
    let _ = crate::game_logic::host_detector_log::drain();
    let _ = crate::game_logic::host_disable_timers_log::drain();
    let _ = crate::game_logic::host_disguise_log::drain();
    let _ = crate::game_logic::host_entity_power_log::drain();
    let _ = crate::game_logic::host_experience_log::drain();
    let _ = crate::game_logic::host_faerie_fire_log::drain();
    let _ = crate::game_logic::host_fire_intent_log::drain();
    let _ = crate::game_logic::host_fire_sound_loop_log::drain();
    let _ = crate::game_logic::host_formation_log::drain();
    let _ = crate::game_logic::host_fow_log::drain();
    let _ = crate::game_logic::host_ground_height_log::drain();
    let _ = crate::game_logic::host_guard_log::drain();
    let _ = crate::game_logic::host_hijacker_log::drain();
    let _ = crate::game_logic::host_hive_log::drain();
    let _ = crate::game_logic::host_identity_log::drain();
    let _ = crate::game_logic::host_kind_of_log::drain();
    let _ = crate::game_logic::host_locomotor_log::drain();
    let _ = crate::game_logic::host_max_health_log::drain();
    let _ = crate::game_logic::host_model_condition_log::drain();
    let _ = crate::game_logic::host_model_mesh_log::drain();
    let _ = crate::game_logic::host_move_log::drain();
    let _ = crate::game_logic::host_movement_log::drain();
    let _ = crate::game_logic::host_overcharge_log::drain();
    let _ = crate::game_logic::host_overlord_log::drain();
    let _ = crate::game_logic::host_owner_log::drain();
    let _ = crate::game_logic::host_physics_motive_log::drain();
    let _ = crate::game_logic::host_player_cooldown_log::drain();
    let _ = crate::game_logic::host_player_meta_log::drain();
    let _ = crate::game_logic::host_player_progress_log::drain();
    let _ = crate::game_logic::host_production_door_log::drain();
    let _ = crate::game_logic::host_production_log::drain();
    let _ = crate::game_logic::host_production_progress_log::drain();
    let _ = crate::game_logic::host_projectile_log::drain();
    let _ = crate::game_logic::host_radar_extend_log::drain();
    let _ = crate::game_logic::host_radar_log::drain();
    let _ = crate::game_logic::host_rally_log::drain();
    let _ = crate::game_logic::host_rebuild_producer_log::drain();
    let _ = crate::game_logic::host_repulsor_log::drain();
    let _ = crate::game_logic::host_selection_radius_log::drain();
    let _ = crate::game_logic::host_shock_stun_log::drain();
    let _ = crate::game_logic::host_sole_healing_log::drain();
    let _ = crate::game_logic::host_spawn_log::drain();
    let _ = crate::game_logic::host_special_power_log::drain();
    let _ = crate::game_logic::host_status_log::drain();
    let _ = crate::game_logic::host_stealth_delay_log::drain();
    let _ = crate::game_logic::host_stealth_flags_log::drain();
    let _ = crate::game_logic::host_stored_supplies_log::drain();
    let _ = crate::game_logic::host_target_location_log::drain();
    let _ = crate::game_logic::host_turret_log::drain();
    let _ = crate::game_logic::host_veterancy_log::drain();
    let _ = crate::game_logic::host_vision_camo_log::drain();
    let _ = crate::game_logic::host_weapon_bonus_log::drain();
    let _ = crate::game_logic::host_weapon_set_log::drain();
    let _ = crate::game_logic::host_weapon_slot_log::drain();
    let _ = crate::game_logic::host_weapon_stats_log::drain();
    let _ = crate::game_logic::host_economy_log::drain();
}
