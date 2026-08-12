//! Single post-logic host→GameWorld residual batch (coupled tick).

use super::*;
use crate::game_logic::GameLogic;
use crate::gameworld_shadow::GameWorldShadow;

/// Wave 682: immediately after the host logic frame on a coupled tick, drain
/// `host_fire_spawn_log` into host CombatSystem + GameWorld projectile map.
///
/// Runs before `shadow_session_after_host_tick` so deferred weapon discharges
/// materialize the same frame without waiting for the full session tail.
/// Session fire-spawn drain stays idempotent (log already empty).
///
/// Safe exclusive borrows: caller passes `&mut GameWorldShadow` + `&mut GameLogic`.
/// Wave 925: single post-logic host→GameWorld residual batch (coupled tick).
/// Replaces N separate eager_apply_* dual-borrows in the engine frame path.
#[inline]
pub fn eager_apply_all_host_residuals_after_logic(
    shadow: &mut GameWorldShadow,
    logic: &mut crate::game_logic::GameLogic,
) {
    let _ = eager_apply_host_fire_spawns_after_logic(shadow, logic);
    let _ = eager_apply_host_move_attack_after_logic(shadow, logic);
    let _ = eager_apply_host_damage_after_logic(shadow, logic);
    let _ = eager_apply_host_heal_after_logic(shadow, logic);
    let _ = eager_apply_host_max_health_after_logic(shadow, logic);
    let _ = eager_apply_host_experience_after_logic(shadow, logic);
    let _ = eager_apply_host_ai_state_after_logic(shadow, logic);
    let _ = eager_apply_host_fire_intent_after_logic(shadow, logic);
    let _ = eager_apply_host_owner_after_logic(shadow, logic);
    let _ = eager_apply_host_movement_after_logic(shadow, logic);
    let _ = eager_apply_host_status_after_logic(shadow, logic);
    let _ = eager_apply_host_veterancy_after_logic(shadow, logic);
    let _ = eager_apply_host_weapon_bonus_after_logic(shadow, logic);
    let _ = eager_apply_host_weapon_slot_after_logic(shadow, logic);
    let _ = eager_apply_host_weapon_set_after_logic(shadow, logic);
    let _ = eager_apply_host_entity_power_after_logic(shadow, logic);
    let _ = eager_apply_host_turret_after_logic(shadow, logic);
    let _ = eager_apply_host_guard_after_logic(shadow, logic);
    let _ = eager_apply_host_rally_after_logic(shadow, logic);
    let _ = eager_apply_host_target_location_after_logic(shadow, logic);
    let _ = eager_apply_host_detector_after_logic(shadow, logic);
    let _ = eager_apply_host_continuous_fire_after_logic(shadow, logic);
    let _ = eager_apply_host_ai_attitude_after_logic(shadow, logic);
    let _ = eager_apply_host_overcharge_after_logic(shadow, logic);
    let _ = eager_apply_host_stealth_flags_after_logic(shadow, logic);
    let _ = eager_apply_host_contain_capacity_after_logic(shadow, logic);
    let _ = eager_apply_host_hive_after_logic(shadow, logic);
    let _ = eager_apply_host_overlord_after_logic(shadow, logic);
    let _ = eager_apply_host_command_set_after_logic(shadow, logic);
    let _ = eager_apply_host_disguise_after_logic(shadow, logic);
    let _ = eager_apply_host_vision_camo_after_logic(shadow, logic);
    let _ = eager_apply_host_weapon_stats_after_logic(shadow, logic);
    let _ = eager_apply_host_selection_radius_after_logic(shadow, logic);
    let _ = eager_apply_host_model_condition_after_logic(shadow, logic);
    let _ = eager_apply_host_demo_mine_cheer_after_logic(shadow, logic);
    let _ = eager_apply_host_formation_after_logic(shadow, logic);
    let _ = eager_apply_host_crush_vision_after_logic(shadow, logic);
    let _ = eager_apply_host_building_type_after_logic(shadow, logic);
    let _ = eager_apply_host_identity_after_logic(shadow, logic);
    let _ = eager_apply_host_ground_height_after_logic(shadow, logic);
    let _ = eager_apply_host_model_mesh_after_logic(shadow, logic);
    let _ = eager_apply_host_fow_after_logic(shadow, logic);
    let _ = eager_apply_host_kind_of_after_logic(shadow, logic);
    let _ = eager_apply_host_faerie_fire_after_logic(shadow, logic);
    let _ = eager_apply_host_repulsor_after_logic(shadow, logic);
    let _ = eager_apply_host_disable_timers_after_logic(shadow, logic);
    let _ = eager_apply_host_body_damage_after_logic(shadow, logic);
    let _ = eager_apply_host_death_type_after_logic(shadow, logic);
    let _ = eager_apply_host_physics_motive_after_logic(shadow, logic);
    let _ = eager_apply_host_locomotor_after_logic(shadow, logic);
    let _ = eager_apply_host_bounce_land_after_logic(shadow, logic);
    let _ = eager_apply_host_ai_mood_after_logic(shadow, logic);
    let _ = eager_apply_host_ai_request_after_logic(shadow, logic);
    let _ = eager_apply_host_shock_stun_after_logic(shadow, logic);
    let _ = eager_apply_host_stealth_delay_after_logic(shadow, logic);
    let _ = eager_apply_host_sole_healing_after_logic(shadow, logic);
    let _ = eager_apply_host_radar_extend_after_logic(shadow, logic);
    let _ = eager_apply_host_hijacker_after_logic(shadow, logic);
    let _ = eager_apply_host_rebuild_producer_after_logic(shadow, logic);
    let _ = eager_apply_host_stored_supplies_after_logic(shadow, logic);
    let _ = eager_apply_host_special_power_after_logic(shadow, logic);
    let _ = eager_apply_host_radar_after_logic(shadow, logic);
    let _ = eager_apply_host_player_progress_after_logic(shadow, logic);
    let _ = eager_apply_host_player_meta_after_logic(shadow, logic);
    let _ = eager_apply_host_player_cooldown_after_logic(shadow, logic);
    let _ = eager_apply_host_production_door_after_logic(shadow, logic);
    let _ = eager_apply_host_production_after_logic(shadow, logic);
    let _ = eager_apply_host_production_progress_after_logic(shadow, logic);
    let _ = eager_apply_host_construction_after_logic(shadow, logic);
    let _ = eager_apply_host_construction_progress_after_logic(shadow, logic);
    let _ = eager_apply_host_combat_attack_after_logic(shadow, logic);
    let _ = eager_apply_host_projectile_after_logic(shadow, logic);
    let _ = eager_apply_host_destroy_after_logic(shadow, logic);
    let _ = eager_apply_host_contain_after_logic(shadow, logic);
    let _ = eager_apply_host_ai_decision_after_logic(shadow, logic);
    let _ = eager_apply_host_spawn_after_logic(shadow, logic);
}
