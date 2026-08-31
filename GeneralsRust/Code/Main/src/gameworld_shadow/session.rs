//! Host mirror helpers, authority-log materialize, and coupled shadow session tick.

use super::*;
use crate::game_logic::{GameLogic, ObjectId, Team};
use gamelogic::world::entities::{EntityId, EntityProductionItem, TemplateRef, Transform};
use gamelogic::world::{GameWorld, PlayerId, WorldMutation, WorldSnapshot};
use std::collections::{HashMap, HashSet};

/// Rebuild convenience: one-shot mirror (stable map discarded with the session).
pub fn mirror_host_into_gameworld(logic: &GameLogic, max_entities: usize) -> GameWorld {
    let mut shadow = GameWorldShadow::new(max_entities);
    shadow.sync_from_host(logic);
    std::mem::replace(&mut shadow.world, GameWorld::new(8))
}

/// Incremental API with stable IDs: sync into an existing shadow session.
pub fn remirror_host_into_gameworld(world: &mut GameWorld, logic: &GameLogic, max_entities: usize) {
    // Legacy signature: no session — full replace (unstable IDs).
    *world = mirror_host_into_gameworld(logic, max_entities);
}

/// Session-based remirror (preferred).
pub fn sync_shadow_from_host(shadow: &mut GameWorldShadow, logic: &GameLogic) {
    shadow.sync_from_host(logic);
}

/// Build shadow session + probe.
pub fn probe_host_vs_gameworld(logic: &mut GameLogic) -> (GameWorldShadow, GameWorldShadowProbe) {
    const MAX_ENTITIES: usize = 4096;
    let mut shadow = GameWorldShadow::new(MAX_ENTITIES);
    shadow.sync_from_host(logic);
    let probe = shadow.probe(logic);
    (shadow, probe)
}

/// Apply undrained host authority logs onto Main `GameLogic`.
///
/// Used when no GameWorld shadow session will last-write this tick (bare
/// `GameLogic::update`, tests, golden host-only). Engine path with an active
/// shadow session drains logs via `shadow_session_after_host_tick` instead.
///
/// Damage authority freezes mid-frame HP; without this, host-only combat never
/// shows HP/destroy. Economy authority parks refunds in `pending_supply_delta`.

/// Apply pending supply deltas without replaying damage/heal logs.
///
/// Host-only command/sim paths use this when DAMAGE_AUTHORITY is fail-open
/// (HP already applied) but ECONOMY_AUTHORITY still defers cash to pending.
pub fn materialize_host_economy_pending(logic: &mut GameLogic) {
    for p in logic.get_players_mut().values_mut() {
        if p.pending_supply_delta != 0 {
            let v = p.resources.supplies as i64 + p.pending_supply_delta;
            p.resources.supplies = if v <= 0 {
                0
            } else if v >= u32::MAX as i64 {
                u32::MAX
            } else {
                v as u32
            };
            p.pending_supply_delta = 0;
        }
    }
}

pub fn materialize_host_authority_logs(logic: &mut GameLogic) {
    // --- Damage (DAMAGE_AUTHORITY freezes mid-frame HP) ---
    let damage_events = crate::game_logic::host_damage_log::drain();
    let mut destroy_ids = Vec::new();
    for e in damage_events {
        let Some(obj) = logic.host_object_mut(e.target) else {
            continue;
        };
        if e.destroyed || e.amount + 1e-3 >= obj.health.current {
            obj.health.current = 0.0;
            obj.status.destroyed = true;
            destroy_ids.push(e.target);
        } else if e.amount > 0.0 {
            obj.health.current = (obj.health.current - e.amount).max(0.0);
        }
    }
    for id in destroy_ids {
        logic.apply_host_object_id_op(crate::game_logic::HostObjectIdOp::MarkForDestruction {
            id: id,
            team: None,
        });
    }

    // --- Heal / absolute HP ---
    for e in crate::game_logic::host_heal_log::drain() {
        if let Some(obj) = logic.host_object_mut(e.target) {
            let max_hp = obj.health.maximum.max(0.0);
            obj.health.current = e.health.clamp(0.0, max_hp);
        }
    }

    // Construction percent already accumulates on host — do not re-apply clamped log.

    // --- Economy pending deltas → real supplies ---
    for p in logic.get_players_mut().values_mut() {
        if p.pending_supply_delta != 0 {
            let v = p.resources.supplies as i64 + p.pending_supply_delta;
            p.resources.supplies = if v <= 0 {
                0
            } else if v >= u32::MAX as i64 {
                u32::MAX
            } else {
                v as u32
            };
            p.pending_supply_delta = 0;
        }
    }
}

/// Optional post-host-tick hook when no long-lived shadow session is held.
/// Materializes DAMAGE/ECONOMY authority logs onto host (does not discard them).
/// Wave 927: single post-logic shadow session boundary (session or no-session).
/// Returns GameWorld presentation entity count for host residual stamping.
#[inline]
pub fn run_post_logic_shadow_boundary(
    shadow: Option<&mut GameWorldShadow>,
    logic: &mut GameLogic,
) -> usize {
    if let Some(shadow) = shadow {
        let probe = shadow_session_after_host_tick(shadow, logic);
        if !probe.full_match() {
            log::warn!("{}", probe.format_report());
        }
        let gw_view = presentation_view_from_shadow(shadow, 0);
        gw_view.entities.len()
    } else {
        let _ = maybe_shadow_after_host_tick(logic);
        0
    }
}

pub fn maybe_shadow_after_host_tick(logic: &mut GameLogic) -> Option<GameWorldShadowProbe> {
    // Engine holds `GameWorldShadow` and calls `shadow_session_after_host_tick`.
    // This helper is the no-session path: materialize authority logs onto host.
    materialize_host_authority_logs(logic);
    if !gameworld_shadow_enabled() {
        return None;
    }
    let (shadow, _probe) = probe_host_vs_gameworld(logic);
    let probe = shadow.probe(logic);
    if !probe.full_match() {
        log::trace!("maybe_shadow probe: {}", probe.format_report());
    }
    Some(probe)
}

pub fn shadow_session_after_host_tick(
    shadow: &mut GameWorldShadow,
    logic: &mut GameLogic,
) -> GameWorldShadowProbe {
    // Wave 939: ready-log drains use logic.apply_ready_log_drain_op(ReadyLogDrainOp::*).
    let _couple_guard = ShadowCoupleGuard::enter();
    shadow.sync_horde_player_rel(logic);

    // Wave 761: GW sole-expires status timers under coupled dual-tick; host peels.
    let _status_timer_exp = shadow.tick_status_timer_expirations(logic.get_frame());
    // Wave 684: prefer post-logic damage batch (already applied to GW when present).
    let (events, early_damage_applied) = match take_early_damage_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_damage_log::drain(), false),
    };
    // Wave 685: prefer post-logic heal batch (already applied to GW when present).
    let (heal_events, early_heal_applied) = match take_early_heal_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_heal_log::drain(), false),
    };
    // Wave 686: prefer post-logic max-health / experience batches.
    let (max_health_events, early_max_health_applied) = match take_early_max_health_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_max_health_log::drain(), false),
    };
    let (experience_events, early_experience_applied) = match take_early_experience_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_experience_log::drain(), false),
    };
    // Wave 690: prefer post-logic weapon-bonus / weapon-slot batches.
    let (weapon_bonus_events, early_weapon_bonus_applied) = match take_early_weapon_bonus_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_weapon_bonus_log::drain(), false),
    };
    let (weapon_slot_events, early_weapon_slot_applied) = match take_early_weapon_slot_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_weapon_slot_log::drain(), false),
    };
    // Wave 691: prefer post-logic entity-power batch.
    let (entity_power_events, early_entity_power_applied) = match take_early_entity_power_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_entity_power_log::drain(), false),
    };
    // Wave 692: prefer post-logic turret batch.
    let (turret_events, early_turret_applied) = match take_early_turret_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_turret_log::drain(), false),
    };
    // Wave 693: prefer post-logic target-location batch.
    let (target_location_events, early_target_location_applied) =
        match take_early_target_location_batch() {
            Some((ev, applied)) => (ev, applied),
            None => (crate::game_logic::host_target_location_log::drain(), false),
        };
    // Wave 693: prefer post-logic detector batch.
    let (detector_events, early_detector_applied) = match take_early_detector_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_detector_log::drain(), false),
    };
    // Wave 693: prefer post-logic continuous-fire batch.
    let (continuous_fire_events, early_continuous_fire_applied) =
        match take_early_continuous_fire_batch() {
            Some((ev, applied)) => (ev, applied),
            None => (crate::game_logic::host_continuous_fire_log::drain(), false),
        };
    // Wave 692: prefer post-logic guard batch.
    let (guard_events, early_guard_applied) = match take_early_guard_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_guard_log::drain(), false),
    };
    // Wave 692: prefer post-logic rally batch.
    let (rally_events, early_rally_applied) = match take_early_rally_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_rally_log::drain(), false),
    };
    // Wave 694: prefer post-logic AI-attitude batch.
    let (ai_attitude_events, early_ai_attitude_applied) = match take_early_ai_attitude_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_ai_attitude_log::drain(), false),
    };
    // Wave 691: prefer post-logic weapon-set batch.
    let (weapon_set_events, early_weapon_set_applied) = match take_early_weapon_set_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_weapon_set_log::drain(), false),
    };
    // Wave 694: prefer post-logic overcharge batch.
    let (overcharge_events, early_overcharge_applied) = match take_early_overcharge_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_overcharge_log::drain(), false),
    };
    // Wave 695: prefer post-logic contain-capacity batch.
    let (contain_capacity_events, early_contain_capacity_applied) =
        match take_early_contain_capacity_batch() {
            Some((ev, applied)) => (ev, applied),
            None => (crate::game_logic::host_contain_capacity_log::drain(), false),
        };
    // Wave 695: prefer post-logic hive batch.
    let (hive_events, early_hive_applied) = match take_early_hive_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_hive_log::drain(), false),
    };
    // Wave 694: prefer post-logic stealth-flags batch.
    let (stealth_flags_events, early_stealth_flags_applied) = match take_early_stealth_flags_batch()
    {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_stealth_flags_log::drain(), false),
    };
    // Wave 695: prefer post-logic overlord batch.
    let (overlord_events, early_overlord_applied) = match take_early_overlord_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_overlord_log::drain(), false),
    };
    // Wave 696: prefer post-logic command-set batch.
    let (command_set_events, early_command_set_applied) = match take_early_command_set_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_command_set_log::drain(), false),
    };
    // Wave 696: prefer post-logic disguise batch.
    let (disguise_events, early_disguise_applied) = match take_early_disguise_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_disguise_log::drain(), false),
    };
    // Wave 696: prefer post-logic vision-camo batch.
    let (vision_camo_events, early_vision_camo_applied) = match take_early_vision_camo_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_vision_camo_log::drain(), false),
    };
    // Wave 697: prefer post-logic weapon-stats batch.
    let (weapon_stats_events, early_weapon_stats_applied) = match take_early_weapon_stats_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_weapon_stats_log::drain(), false),
    };
    // Wave 688: prefer post-logic movement batch.
    let (movement_events, early_movement_applied) = match take_early_movement_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_movement_log::drain(), false),
    };
    // Wave 697: prefer post-logic selection-radius batch.
    let (selection_radius_events, early_selection_radius_applied) =
        match take_early_selection_radius_batch() {
            Some((ev, applied)) => (ev, applied),
            None => (crate::game_logic::host_selection_radius_log::drain(), false),
        };
    // Wave 697: prefer post-logic model-condition batch.
    let (model_condition_events, early_model_condition_applied) =
        match take_early_model_condition_batch() {
            Some((ev, applied)) => (ev, applied),
            None => (crate::game_logic::host_model_condition_log::drain(), false),
        };
    // Wave 698: prefer post-logic demo-mine-cheer batch.
    let (demo_mine_cheer_events, early_demo_mine_cheer_applied) =
        match take_early_demo_mine_cheer_batch() {
            Some((ev, applied)) => (ev, applied),
            None => (crate::game_logic::host_demo_mine_cheer_log::drain(), false),
        };
    // Wave 698: prefer post-logic formation batch.
    let (formation_events, early_formation_applied) = match take_early_formation_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_formation_log::drain(), false),
    };
    // Wave 698: prefer post-logic crush-vision batch.
    let (crush_vision_events, early_crush_vision_applied) = match take_early_crush_vision_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_crush_vision_log::drain(), false),
    };
    // Wave 699: prefer post-logic building-type batch.
    let (building_type_events, early_building_type_applied) = match take_early_building_type_batch()
    {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_building_type_log::drain(), false),
    };
    // Wave 699: prefer post-logic identity batch.
    let (identity_events, early_identity_applied) = match take_early_identity_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_identity_log::drain(), false),
    };
    // Wave 699: prefer post-logic ground-height batch.
    let (ground_height_events, early_ground_height_applied) = match take_early_ground_height_batch()
    {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_ground_height_log::drain(), false),
    };
    // Wave 700: prefer post-logic model-mesh batch.
    let (model_mesh_events, early_model_mesh_applied) = match take_early_model_mesh_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_model_mesh_log::drain(), false),
    };
    // Wave 700: prefer post-logic FOW batch.
    let (fow_events, early_fow_applied) = match take_early_fow_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_fow_log::drain(), false),
    };
    // Wave 700: prefer post-logic kind-of batch.
    let (kind_of_events, early_kind_of_applied) = match take_early_kind_of_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_kind_of_log::drain(), false),
    };
    // Wave 701: prefer post-logic faerie-fire batch.
    let (faerie_events, early_faerie_fire_applied) = match take_early_faerie_fire_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_faerie_fire_log::drain(), false),
    };
    // Wave 701: prefer post-logic repulsor batch.
    let (repulsor_events, early_repulsor_applied) = match take_early_repulsor_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_repulsor_log::drain(), false),
    };
    // Wave 701: prefer post-logic disable-timers batch.
    let (disable_timer_events, early_disable_timers_applied) =
        match take_early_disable_timers_batch() {
            Some((ev, applied)) => (ev, applied),
            None => (crate::game_logic::host_disable_timers_log::drain(), false),
        };
    // Wave 688: prefer post-logic owner batch.
    let (owner_events, early_owner_applied) = match take_early_owner_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_owner_log::drain(), false),
    };
    // Wave 712: prefer post-logic spawn batch (mid-frame map already idempotent).
    let (spawn_events, early_spawn_applied) = match take_early_spawn_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_spawn_log::drain(), false),
    };
    // Wave 711: prefer post-logic destroy batch.
    let (destroy_events, early_destroy_applied) = match take_early_destroy_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_destroy_log::drain(), false),
    };
    // Wave 712: prefer post-logic attack batch (move_attack eager path).
    let (attack_events, early_attack_applied) = match take_early_attack_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_attack_log::drain(), false),
    };
    let _fire_loop_events = crate::game_logic::host_fire_sound_loop_log::drain();
    // Wave 689: prefer post-logic status / veterancy batches.
    let (status_events, early_status_applied) = match take_early_status_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_status_log::drain(), false),
    };
    let (veterancy_events, early_veterancy_applied) = match take_early_veterancy_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_veterancy_log::drain(), false),
    };
    // Wave 712: prefer post-logic move batch (move_attack eager path).
    let (move_events, early_move_applied) = match take_early_move_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_move_log::drain(), false),
    };
    // Wave 709: prefer post-logic production / construction batches.
    let (production_events, early_production_applied) = match take_early_production_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_production_log::drain(), false),
    };
    let (production_progress_events, early_production_progress_applied) =
        match take_early_production_progress_batch() {
            Some((ev, applied)) => (ev, applied),
            None => (
                crate::game_logic::host_production_progress_log::drain(),
                false,
            ),
        };
    let (construction_events, early_construction_applied) = match take_early_construction_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_construction_log::drain(), false),
    };
    let (construction_progress_events, early_construction_progress_applied) =
        match take_early_construction_progress_batch() {
            Some((ev, applied)) => (ev, applied),
            None => (
                crate::game_logic::host_construction_progress_log::drain(),
                false,
            ),
        };
    // Wave 707: prefer post-logic special-power batch.
    let (special_power_events, early_special_power_applied) = match take_early_special_power_batch()
    {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_special_power_log::drain(), false),
    };
    // Wave 706: prefer post-logic stored-supplies batch.
    let (stored_supplies_events, early_stored_supplies_applied) =
        match take_early_stored_supplies_batch() {
            Some((ev, applied)) => (ev, applied),
            None => (crate::game_logic::host_stored_supplies_log::drain(), false),
        };
    // Wave 687: prefer post-logic AI-state batch.
    let (ai_state_events, early_ai_state_applied) = match take_early_ai_state_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_ai_state_log::drain(), false),
    };
    // Wave 711: prefer post-logic contain batch.
    let (contain_events, early_contain_applied) = match take_early_contain_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_contain_log::drain(), false),
    };
    // Wave 707: prefer post-logic radar batch.
    let (radar_events, early_radar_applied) = match take_early_radar_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_radar_log::drain(), false),
    };
    // Wave 707: prefer post-logic player-progress batch.
    let (player_progress_events, early_player_progress_applied) =
        match take_early_player_progress_batch() {
            Some((ev, applied)) => (ev, applied),
            None => (crate::game_logic::host_player_progress_log::drain(), false),
        };
    // Wave 708: prefer post-logic player-meta batch.
    let (player_meta_events, early_player_meta_applied) = match take_early_player_meta_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_player_meta_log::drain(), false),
    };
    // Wave 708: prefer post-logic player-cooldown batch.
    let (player_cooldown_events, early_player_cooldown_applied) =
        match take_early_player_cooldown_batch() {
            Some((ev, applied)) => (ev, applied),
            None => (crate::game_logic::host_player_cooldown_log::drain(), false),
        };
    let upgrade_events = logic.host_upgrades().completed_this_frame_snapshot();
    let auth = gameworld_damage_authority_enabled();
    // Keep pre-tick shadow HP when we will re-apply damage/heal events as mutations.
    let write_health = !(auth && (!events.is_empty() || !heal_events.is_empty()));
    shadow.sync_from_host_with(logic, write_health);
    // Spawn channel: map any create_object events not yet present (usually no-op after sync).
    // Wave 712: skip GW re-apply when post-logic eager path already ran.
    let spawns_applied = if early_spawn_applied {
        0
    } else {
        shadow.apply_host_spawn_events(&spawn_events, logic)
    };
    // Wave 709: skip GW re-apply when post-logic eager path already ran.
    let _prod_applied = if early_production_applied {
        0
    } else {
        shadow.apply_host_production_events(&production_events, logic)
    };
    let _pp_applied = if early_production_progress_applied {
        0
    } else {
        shadow.apply_host_production_progress_events(&production_progress_events)
    };
    // Sole progress tick under PRODUCTION_AUTHORITY (host skips advance; Wave 477 no progress stomp).
    let _prod_tick = shadow
        .tick_production_queues(game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL);
    let host_frame = logic.get_frame();
    shadow.world_mut().set_frame(host_frame as u64);
    let _door_tick = shadow.tick_production_doors(host_frame);
    let _cmd_view = shadow.ingest_command_queue_view(logic);
    // Wave 708: prefer post-logic production-door batch.
    let (production_door_events, early_production_door_applied) =
        match take_early_production_door_batch() {
            Some((ev, applied)) => (ev, applied),
            None => (crate::game_logic::host_production_door_log::drain(), false),
        };
    let _pd_applied = if early_production_door_applied {
        0
    } else {
        shadow.apply_host_production_door_events(&production_door_events)
    };
    // Wave 709: skip GW re-apply when post-logic eager path already ran.
    let _construction_applied = if early_construction_applied {
        0
    } else {
        shadow.apply_host_construction_events(&construction_events, logic)
    };
    let _construction_progress_applied = if early_construction_progress_applied {
        0
    } else {
        shadow.apply_host_construction_progress_events(&construction_progress_events)
    };
    let _construction_tick = shadow
        .tick_construction_progress(game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL);
    // Wave 707: skip GW re-apply when post-logic eager path already ran.
    let _sp_applied = if early_special_power_applied {
        0
    } else {
        shadow.apply_host_special_power_events(&special_power_events)
    };
    // Under SPECIAL_POWER_AUTHORITY, GameWorld sole-ticks SP countdown; host skips advance.
    let _sp_tick = shadow.tick_special_power_cooldowns(
        game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL,
    );
    if gameworld_special_power_sole_tick_enabled() {
        let _sp_wb = shadow.writeback_special_power_to_host(logic);
        // Wave 717/938: same-frame host SP-ready via post-writeback authority.
        logic.apply_post_writeback_complete_op(
            crate::game_logic::PostWritebackCompleteOp::SpecialPowerReadyAfterWriteback,
        );
    }

    // Wave 706: skip GW re-apply when post-logic eager path already ran.
    let _ss_applied = if early_stored_supplies_applied {
        0
    } else {
        shadow.apply_host_stored_supplies_events(&stored_supplies_events)
    };
    // Wave 687: skip GW re-apply when post-logic eager path already ran.
    let _ai_applied = if !early_ai_state_applied {
        shadow.apply_host_ai_state_events(&ai_state_events)
    } else {
        ai_state_events.len()
    };
    // Wave 711: skip GW re-apply when post-logic eager path already ran.
    let _contain_applied = if early_contain_applied {
        0
    } else {
        shadow.apply_host_contain_events(&contain_events)
    };
    // Wave 628: contain membership last-write + ready-log residual.
    let _contain_wb = shadow.writeback_contain_to_host(logic);
    let _contain_ready =
        logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Contain);
    // Wave 707: skip GW re-apply when post-logic eager path already ran.
    let _radar_applied = if early_radar_applied {
        0
    } else {
        shadow.apply_host_radar_events(&radar_events)
    };
    // Wave 707: skip GW re-apply when post-logic eager path already ran.
    let _progress_applied = if early_player_progress_applied {
        0
    } else {
        shadow.apply_host_player_progress_events(&player_progress_events)
    };
    // Wave 708: skip GW re-apply when post-logic eager path already ran.
    let _meta_applied = if early_player_meta_applied {
        0
    } else {
        shadow.apply_host_player_meta_events(&player_meta_events)
    };
    // Wave 708: skip GW re-apply when post-logic eager path already ran.
    let _cd_applied = if early_player_cooldown_applied {
        0
    } else {
        shadow.apply_host_player_cooldown_events(&player_cooldown_events)
    };

    // Shared SP sole-tick after host cooldown snapshot applied.
    let _sp_player_tick = shadow.tick_player_shared_special_power_cooldowns(
        game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL,
    );
    if gameworld_special_power_sole_tick_enabled() {
        let _ = shadow.writeback_shared_special_power_cooldowns_to_host(logic);
    }
    let _upgrades_applied = shadow.apply_host_upgrade_events(&upgrade_events);
    // Wave 711: skip GW re-apply when post-logic eager path already ran.
    let (dest_q, _dest_a) = if early_destroy_applied {
        (0usize, 0usize)
    } else {
        shadow.apply_host_destroy_events(&destroy_events)
    };
    // Wave 685: skip GW re-apply when post-logic eager path already ran.
    let _heals = if !early_heal_applied {
        shadow.apply_host_heal_events(&heal_events)
    } else {
        heal_events.len()
    };
    // Wave 686: skip GW re-apply when post-logic eager path already ran.
    let _maxh_applied = if !early_max_health_applied {
        shadow.apply_host_max_health_events(&max_health_events)
    } else {
        max_health_events.len()
    };
    let _xp_applied = if !early_experience_applied {
        shadow.apply_host_experience_events(&experience_events)
    } else {
        experience_events.len()
    };
    // Wave 690: skip GW re-apply when post-logic eager path already ran.
    let _wb_applied = if early_weapon_bonus_applied {
        0
    } else {
        shadow.apply_host_weapon_bonus_events(&weapon_bonus_events)
    };
    let _wslot_applied = if early_weapon_slot_applied {
        0
    } else {
        shadow.apply_host_weapon_slot_events(&weapon_slot_events)
    };
    // Wave 691: skip GW re-apply when post-logic eager path already ran.
    let _epow_applied = if early_entity_power_applied {
        0
    } else {
        shadow.apply_host_entity_power_events(&entity_power_events)
    };
    // Wave 692: skip GW re-apply when post-logic eager path already ran.
    let _tur_applied = if early_turret_applied {
        0
    } else {
        shadow.apply_host_turret_events(&turret_events)
    };
    // Wave 693: skip GW re-apply when post-logic eager path already ran.
    let _tloc_applied = if early_target_location_applied {
        0
    } else {
        shadow.apply_host_target_location_events(&target_location_events)
    };
    // Wave 693: skip GW re-apply when post-logic eager path already ran.
    let _det_applied = if early_detector_applied {
        0
    } else {
        shadow.apply_host_detector_events(&detector_events)
    };
    // Wave 693: skip GW re-apply when post-logic eager path already ran.
    let _cf_applied = if early_continuous_fire_applied {
        0
    } else {
        shadow.apply_host_continuous_fire_events(&continuous_fire_events)
    };
    // Wave 710: prefer post-logic combat-attack batch.
    let (combat_attack_events, early_combat_attack_applied) = match take_early_combat_attack_batch()
    {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_combat_attack_log::drain(), false),
    };
    let _ca_applied = if early_combat_attack_applied {
        0
    } else {
        shadow.apply_host_combat_attack_events(&combat_attack_events)
    };
    // Wave 687: prefer post-logic fire-intent batch.
    let (fire_intent_events, early_fire_intent_applied) = match take_early_fire_intent_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_fire_intent_log::drain(), false),
    };
    let _fi_applied = if !early_fire_intent_applied {
        shadow.apply_host_fire_intent_events(&fire_intent_events)
    } else {
        fire_intent_events.len()
    };
    // Wave 710: prefer post-logic projectile batch.
    let (projectile_events, early_projectile_applied) = match take_early_projectile_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_projectile_log::drain(), false),
    };
    let _proj_applied = if early_projectile_applied {
        0
    } else {
        shadow.apply_host_projectile_events(&projectile_events)
    };
    // Fire-spawn authority: materialize deferred weapon discharges into CombatSystem
    // before projectile integrate authority steps flight.
    if gameworld_fire_spawn_authority_enabled() {
        // Wave 712: prefer post-logic fire-spawn batch.
        let (spawns, early_fire_spawn_applied) = match take_early_fire_spawn_batch() {
            Some((ev, applied)) => (ev, applied),
            None => (crate::game_logic::host_fire_spawn_log::drain(), false),
        };
        let _fs = if early_fire_spawn_applied {
            0
        } else {
            shadow.apply_host_fire_spawn_events(logic, spawns)
        };
    }
    if gameworld_projectile_authority_enabled() {
        let dt = 1.0_f32 / 30.0;
        // Host object poses for homing refresh.
        let stepped = {
            let logic_ref = &*logic;
            shadow.world.step_projectiles(dt, |hid| {
                logic_ref.host_objects().get(&ObjectId(hid)).map(|o| {
                    let p = o.get_position();
                    [p.x, p.y, p.z]
                })
            })
        };
        let _ = stepped;
        let _pw = shadow.writeback_projectiles_to_host(logic);
        // Wave 678: drain projectiles ready log after GW writeback.
        let _w678_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Projectiles);
        // Hit resolution at GameWorld-integrated poses (dt=0 keeps pose stable).
        let hits = logic.resolve_projectiles_hits_only();
        let _ = hits;
        crate::game_logic::host_projectile_log::record_snapshot(
            logic.combat_system.projectiles_snapshot(),
        );
        // Re-apply post-hit residual so GW drops destroyed projectiles.
        let _ =
            shadow.apply_host_projectile_events(&crate::game_logic::host_projectile_log::drain());
    }

    // Wave 692: skip GW re-apply when post-logic eager path already ran.
    let _guard_applied = if early_guard_applied {
        0
    } else {
        shadow.apply_host_guard_events(&guard_events)
    };
    // Wave 692: skip GW re-apply when post-logic eager path already ran.
    let _rally_applied = if early_rally_applied {
        0
    } else {
        shadow.apply_host_rally_events(&rally_events)
    };
    // Wave 694: skip GW re-apply when post-logic eager path already ran.
    let _att_applied = if early_ai_attitude_applied {
        0
    } else {
        shadow.apply_host_ai_attitude_events(&ai_attitude_events)
    };
    // Wave 704: prefer post-logic AI-mood batch.
    let (ai_mood_events, early_ai_mood_applied) = match take_early_ai_mood_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_ai_mood_log::drain(), false),
    };
    let _mood_applied = if early_ai_mood_applied {
        0
    } else {
        shadow.apply_host_ai_mood_events(&ai_mood_events)
    };
    // Wave 704: prefer post-logic AI-request batch.
    let (ai_req_events, early_ai_request_applied) = match take_early_ai_request_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_ai_request_log::drain(), false),
    };
    let _ar_applied = if early_ai_request_applied {
        0
    } else {
        shadow.apply_host_ai_request_events(&ai_req_events)
    };
    if gameworld_ai_decision_authority_enabled() {
        // Wave 711: prefer post-logic AI-decision batch.
        let (ai_decision_events, early_ai_decision_applied) = match take_early_ai_decision_batch() {
            Some((ev, applied)) => (ev, applied),
            None => (crate::game_logic::host_ai_decision_log::drain(), false),
        };
        let _ad = if early_ai_decision_applied {
            0
        } else {
            shadow.apply_ai_decisions_as_world_mutations(&ai_decision_events)
        };
        // Last-write host attack target / AI state / move from GameWorld.
        let _ = shadow.writeback_attack_targets_to_host(logic);
        // Wave 638: drain attack-target ready log after GW writeback.
        let _atk_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::AttackTarget);
        let _ = shadow.writeback_ai_state_to_host(logic);
        // Wave 630: drain AI-state ready log after GW writeback.
        let _ai_st_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::AiState);
        let _ = shadow.writeback_movement_to_host(logic);
        // Wave 637: drain movement ready log after GW writeback.
        let _mv_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Movement);
    } else {
        let _ =
            shadow.apply_host_ai_decision_events(&crate::game_logic::host_ai_decision_log::drain());
    }
    // Wave 691: skip GW re-apply when post-logic eager path already ran.
    let _wset_applied = if early_weapon_set_applied {
        0
    } else {
        shadow.apply_host_weapon_set_events(&weapon_set_events)
    };
    // Wave 694: skip GW re-apply when post-logic eager path already ran.
    let _oc_applied = if early_overcharge_applied {
        0
    } else {
        shadow.apply_host_overcharge_events(&overcharge_events)
    };
    // Wave 695: skip GW re-apply when post-logic eager path already ran.
    let _cap_applied = if early_contain_capacity_applied {
        0
    } else {
        shadow.apply_host_contain_capacity_events(&contain_capacity_events)
    };
    // Wave 695: skip GW re-apply when post-logic eager path already ran.
    let _hive_applied = if early_hive_applied {
        0
    } else {
        shadow.apply_host_hive_events(&hive_events)
    };
    // Wave 706: prefer post-logic hijacker batch.
    let (hijack_events, early_hijacker_applied) = match take_early_hijacker_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_hijacker_log::drain(), false),
    };
    let _hj_applied = if early_hijacker_applied {
        0
    } else {
        shadow.apply_host_hijacker_events(&hijack_events)
    };
    // Wave 694: skip GW re-apply when post-logic eager path already ran.
    let _stf_applied = if early_stealth_flags_applied {
        0
    } else {
        shadow.apply_host_stealth_flags_events(&stealth_flags_events)
    };
    // Wave 705: prefer post-logic stealth-delay batch.
    let (stealth_delay_events, early_stealth_delay_applied) = match take_early_stealth_delay_batch()
    {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_stealth_delay_log::drain(), false),
    };
    let _sd_applied = if early_stealth_delay_applied {
        0
    } else {
        shadow.apply_host_stealth_delay_events(&stealth_delay_events)
    };
    // Wave 695: skip GW re-apply when post-logic eager path already ran.
    let _ol_applied = if early_overlord_applied {
        0
    } else {
        shadow.apply_host_overlord_events(&overlord_events)
    };
    // Wave 696: skip GW re-apply when post-logic eager path already ran.
    let _cs_applied = if early_command_set_applied {
        0
    } else {
        shadow.apply_host_command_set_events(&command_set_events)
    };
    // Wave 696: skip GW re-apply when post-logic eager path already ran.
    let _dg_applied = if early_disguise_applied {
        0
    } else {
        shadow.apply_host_disguise_events(&disguise_events)
    };
    // Wave 696: skip GW re-apply when post-logic eager path already ran.
    let _vc_applied = if early_vision_camo_applied {
        0
    } else {
        shadow.apply_host_vision_camo_events(&vision_camo_events)
    };
    // Wave 697: skip GW re-apply when post-logic eager path already ran.
    let _ws_applied = if early_weapon_stats_applied {
        0
    } else {
        shadow.apply_host_weapon_stats_events(&weapon_stats_events)
    };
    // Wave 702: prefer post-logic body-damage batch.
    let (body_damage_events, early_body_damage_applied) = match take_early_body_damage_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_body_damage_log::drain(), false),
    };
    let _bd_applied = if early_body_damage_applied {
        0
    } else {
        shadow.apply_host_body_damage_events(&body_damage_events)
    };
    // Wave 702: prefer post-logic death-type batch.
    let (death_type_events, early_death_type_applied) = match take_early_death_type_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_death_type_log::drain(), false),
    };
    let _dt_applied = if early_death_type_applied {
        0
    } else {
        shadow.apply_host_death_type_events(&death_type_events)
    };
    // Wave 705: prefer post-logic radar-extend batch.
    let (radar_extend_events, early_radar_extend_applied) = match take_early_radar_extend_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_radar_extend_log::drain(), false),
    };
    let _re_applied = if early_radar_extend_applied {
        0
    } else {
        shadow.apply_host_radar_extend_events(&radar_extend_events)
    };
    // Wave 704: prefer post-logic shock-stun batch.
    let (shock_stun_events, early_shock_stun_applied) = match take_early_shock_stun_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_shock_stun_log::drain(), false),
    };
    let _ss_applied = if early_shock_stun_applied {
        0
    } else {
        shadow.apply_host_shock_stun_events(&shock_stun_events)
    };
    // Wave 706: prefer post-logic rebuild-producer batch.
    let (rebuild_producer_events, early_rebuild_producer_applied) =
        match take_early_rebuild_producer_batch() {
            Some((ev, applied)) => (ev, applied),
            None => (crate::game_logic::host_rebuild_producer_log::drain(), false),
        };
    let _rp_applied = if early_rebuild_producer_applied {
        0
    } else {
        shadow.apply_host_rebuild_producer_events(&rebuild_producer_events)
    };
    // Wave 705: prefer post-logic sole-healing batch.
    let (sole_healing_events, early_sole_healing_applied) = match take_early_sole_healing_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_sole_healing_log::drain(), false),
    };
    let _sh_applied = if early_sole_healing_applied {
        0
    } else {
        shadow.apply_host_sole_healing_events(&sole_healing_events)
    };
    // Wave 688: skip GW re-apply when post-logic eager path already ran.
    let _mv_applied = if !early_movement_applied {
        shadow.apply_host_movement_events(&movement_events)
    } else {
        movement_events.len()
    };
    // Wave 702: prefer post-logic physics-motive batch.
    let (physics_motive_events, early_physics_motive_applied) =
        match take_early_physics_motive_batch() {
            Some((ev, applied)) => (ev, applied),
            None => (crate::game_logic::host_physics_motive_log::drain(), false),
        };
    let _pm_applied = if early_physics_motive_applied {
        0
    } else {
        shadow.apply_host_physics_motive_events(&physics_motive_events)
    };
    // Wave 703: prefer post-logic locomotor batch.
    let (loco_events, early_locomotor_applied) = match take_early_locomotor_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_locomotor_log::drain(), false),
    };
    let _loco_applied = if early_locomotor_applied {
        0
    } else {
        shadow.apply_host_locomotor_events(&loco_events)
    };
    // Wave 703: prefer post-logic bounce-land batch.
    let (bounce_land_events, early_bounce_land_applied) = match take_early_bounce_land_batch() {
        Some((ev, applied)) => (ev, applied),
        None => (crate::game_logic::host_bounce_land_log::drain(), false),
    };
    let _bl_applied = if early_bounce_land_applied {
        0
    } else {
        shadow.apply_host_bounce_land_events(&bounce_land_events)
    };

    // Wave 697: skip GW re-apply when post-logic eager path already ran.
    let _sr_applied = if early_selection_radius_applied {
        0
    } else {
        shadow.apply_host_selection_radius_events(&selection_radius_events)
    };
    // Wave 697: skip GW re-apply when post-logic eager path already ran.
    let _mc_applied = if early_model_condition_applied {
        0
    } else {
        shadow.apply_host_model_condition_events(&model_condition_events)
    };
    // Wave 698: skip GW re-apply when post-logic eager path already ran.
    let _dmc_applied = if early_demo_mine_cheer_applied {
        0
    } else {
        shadow.apply_host_demo_mine_cheer_events(&demo_mine_cheer_events)
    };
    // Wave 698: skip GW re-apply when post-logic eager path already ran.
    let _form_applied = if early_formation_applied {
        0
    } else {
        shadow.apply_host_formation_events(&formation_events)
    };
    // Wave 698: skip GW re-apply when post-logic eager path already ran.
    let _cv_applied = if early_crush_vision_applied {
        0
    } else {
        shadow.apply_host_crush_vision_events(&crush_vision_events)
    };
    // Wave 699: skip GW re-apply when post-logic eager path already ran.
    let _bt_applied = if early_building_type_applied {
        0
    } else {
        shadow.apply_host_building_type_events(&building_type_events)
    };
    // Wave 699: skip GW re-apply when post-logic eager path already ran.
    let _id_applied = if early_identity_applied {
        0
    } else {
        shadow.apply_host_identity_events(&identity_events)
    };
    // Wave 699: skip GW re-apply when post-logic eager path already ran.
    let _gh_applied = if early_ground_height_applied {
        0
    } else {
        shadow.apply_host_ground_height_events(&ground_height_events)
    };
    // Wave 700: skip GW re-apply when post-logic eager path already ran.
    let _mm_applied = if early_model_mesh_applied {
        0
    } else {
        shadow.apply_host_model_mesh_events(&model_mesh_events)
    };
    // Wave 700: skip GW re-apply when post-logic eager path already ran.
    let _fow_applied = if early_fow_applied {
        0
    } else {
        shadow.apply_host_fow_events(&fow_events)
    };
    // Wave 700: skip GW re-apply when post-logic eager path already ran.
    let _ko_applied = if early_kind_of_applied {
        0
    } else {
        shadow.apply_host_kind_of_events(&kind_of_events)
    };
    // Wave 701: skip GW re-apply when post-logic eager path already ran.
    let _ff_applied = if early_faerie_fire_applied {
        0
    } else {
        shadow.apply_host_faerie_fire_events(&faerie_events)
    };
    // Wave 701: skip GW re-apply when post-logic eager path already ran.
    let _rp_applied = if early_repulsor_applied {
        0
    } else {
        shadow.apply_host_repulsor_events(&repulsor_events)
    };
    // Wave 701: skip GW re-apply when post-logic eager path already ran.
    let _dt_applied = if early_disable_timers_applied {
        0
    } else {
        shadow.apply_host_disable_timers_events(&disable_timer_events)
    };
    // Wave 688: skip GW re-apply when post-logic eager path already ran.
    let _owners = if !early_owner_applied {
        shadow.apply_host_owner_events(logic, &owner_events)
    } else {
        owner_events.len()
    };
    // When GameWorld owns path integrate, do not clobber entity poses with host
    // pre-integrate positions; still pull move targets / movement residuals above.
    if !gameworld_movement_authority_enabled() {
        let _poses = shadow.apply_host_positions_as_transforms(logic);
    } else {
        // Ensure move destinations from host are present before step.
        let _move_tgts = shadow.apply_host_move_targets(logic);
    }
    // Wave 712: skip re-queue when post-logic move/attack eager path already ran.
    if !early_attack_applied {
        for ev in &attack_events {
            let _ = shadow.queue_set_attack_target_for_host(ev.attacker, ev.target);
        }
    }
    if !early_move_applied {
        for ev in &move_events {
            let _ = shadow.queue_set_move_target_for_host(ev.unit, ev.destination);
        }
    }
    // Wave 689: skip GW re-apply when post-logic eager path already ran.
    if !early_status_applied {
        for ev in &status_events {
            let _ = shadow.queue_set_combat_status_for_host(*ev);
        }
    }
    if !early_veterancy_applied {
        for ev in &veterancy_events {
            let _ = shadow.queue_set_veterancy_for_host(ev.object, ev.ordinal);
        }
    }
    if !attack_events.is_empty()
        || !move_events.is_empty()
        || (!early_status_applied && !status_events.is_empty())
        || (!early_veterancy_applied && !veterancy_events.is_empty())
    {
        let _ = shadow.apply_pending();
    }
    let _atks = shadow.apply_host_attack_targets(logic);
    let _moves = shadow.apply_host_move_targets(logic);
    // Attack-target channel is always bidirectional once session is live: shadow mutations
    // (and host bulk resync above) settle, then writeback keeps host Object::target aligned.
    let _atk_wb = shadow.writeback_attack_targets_to_host(logic);
    // Wave 638: drain attack-target ready log after GW writeback.
    let _atk_ready =
        logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::AttackTarget);
    let _ = shadow.writeback_fire_intent_to_host(logic);
    // Wave 640: drain fire-intent ready log after GW writeback.
    let _fi_ready = logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::FireIntent);
    let _move_wb = shadow.writeback_move_targets_to_host(logic);
    // Wave 639: drain move-target ready log after GW writeback.
    let _mt_ready = logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::MoveTarget);
    // Pose last-writer after all SetTransform mutations this session.
    // Mid-frame movement authority: integrate AFTER command channels, BEFORE pose writeback.
    if gameworld_movement_authority_enabled() {
        let dt = 1.0_f32 / 30.0;
        let stepped = shadow.world.step_movement(dt);
        if stepped > 0 {
            log::trace!("GameWorld step_movement stepped={stepped}");
        }
    }
    let _pose_wb = shadow.writeback_transforms_to_host(logic);
    // Wave 636: drain transform ready log after GW writeback.
    let _xf_ready = logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Transform);
    // Movement authority: always last-write velocity/path/move_target/moving after step
    // (do not gate on damage-channel auth — path frames often have empty damage logs).
    if gameworld_movement_authority_enabled() {
        let _mv_wb = shadow.writeback_movement_to_host(logic);
        // Wave 637: drain movement ready log after GW writeback.
        let _mv_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Movement);
        let _ = shadow.writeback_locomotor_to_host(logic);
        // Wave 646: drain locomotor ready log after GW writeback.
        let _loco_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Locomotor);
        let _ = shadow.writeback_ai_request_to_host(logic);
        // Wave 648: drain AI-request ready log after GW writeback.
        let _air_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::AiRequest);
        let _ = shadow.writeback_hijacker_to_host(logic);
        // Wave 647: drain hijacker ready log after GW writeback.
        let _hj_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Hijacker);
        let _ = shadow.writeback_physics_motive_to_host(logic);
        // Wave 649: drain physics-motive ready log after GW writeback.
        let _pm_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::PhysicsMotive);
        let _ = shadow.writeback_locomotor_to_host(logic);
        // Wave 646: drain locomotor ready log after GW writeback.
        let _loco_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Locomotor);
        let _ = shadow.writeback_ai_request_to_host(logic);
        // Wave 648: drain AI-request ready log after GW writeback.
        let _air_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::AiRequest);
        let _ = shadow.writeback_hijacker_to_host(logic);
        // Wave 647: drain hijacker ready log after GW writeback.
        let _hj_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Hijacker);
        let _ = shadow.writeback_bounce_land_to_host(logic);
        // Wave 650: drain bounce-land ready log after GW writeback.
        let _bl_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::BounceLand);
        let _move_tgt_wb = shadow.writeback_move_targets_to_host(logic);
        // Wave 639: drain move-target ready log after GW writeback.
        let _mt_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::MoveTarget);
        let _moving_st_wb = shadow.writeback_combat_status_to_host(logic);
        // Wave 768: LifetimeUpdate expire → host mark-for-destruction (no dual timer).
        for id in crate::game_logic::host_lifetime_expire_log::drain() {
            logic.apply_host_object_id_op(crate::game_logic::HostObjectIdOp::MarkForDestruction {
                id: id,
                team: None,
            });
        }
        // Wave 769: PoisonedBehavior DoT → host UNRESISTABLE apply (no dual timer).
        for ev in crate::game_logic::host_poison_dot_log::drain() {
            // Wave 941: poison DoT via host residual mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::PoisonDot {
                    object: ev.object,
                    amount: ev.amount,
                    death_type: ev.death_type,
                },
            );
        }
        // Wave 770: ToppleUpdate kill-when-down → host destroy (no dual timer).
        for id in crate::game_logic::host_topple_kill_log::drain() {
            // Wave 941: force-kill residual via host residual mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::ForceKill {
                    id,
                    death_type: Some(crate::game_logic::host_usa_pilot::HostDeathType::Toppled),
                    refresh_model_condition: false,
                    mark_destroy: true,
                },
            );
        }
        // Wave 771: HeightDieUpdate kill → host destroy (no dual timer).
        for id in crate::game_logic::host_height_die_kill_log::drain() {
            // Wave 941: force-kill residual via host residual mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::ForceKill {
                    id,
                    death_type: None,
                    refresh_model_condition: true,
                    mark_destroy: true,
                },
            );
        }
        // Wave 772: JetSlowDeathBehavior done → host destroy (no dual timer).
        for id in crate::game_logic::host_jet_slow_death_kill_log::drain() {
            // Wave 941: force-kill residual via host residual mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::ForceKill {
                    id,
                    death_type: None,
                    refresh_model_condition: true,
                    mark_destroy: true,
                },
            );
        }
        // Wave 773: HelicopterSlowDeathBehavior done → host destroy (no dual timer).
        for id in crate::game_logic::host_heli_slow_death_kill_log::drain() {
            // C++ :457-472 FinalBlowUp FX/OCL + rubble before destroyObject.
            if let Some(obj) = logic.objects.get_mut(&id) {
                if let Some(h) = obj.helicopter_slow_death.as_mut() {
                    if h.pending_fx.is_none() {
                        h.pending_fx = h.fx.fx_final_blow_up.clone();
                        h.pending_ocl = h.fx.ocl_final_blow_up.clone();
                        h.pending_rubble = h.fx.final_rubble_object.clone();
                    }
                    let ev = h.take_pending_effects();
                    obj.apply_heli_death_phase(ev);
                }
            }
            if let Some((fx, killer)) = logic.objects.get(&id).and_then(|o| {
                o.pending_death_fx
                    .clone()
                    .map(|fx| (fx, o.last_damage_source))
            }) {
                let _ = logic.dispatch_fx_list_at_host_object(&fx, id, killer);
            }
            if let Some(a) = logic
                .objects
                .get(&id)
                .and_then(|o| o.pending_death_audio.clone())
            {
                let pos = logic
                    .objects
                    .get(&id)
                    .map(|o| o.get_position())
                    .unwrap_or(glam::Vec3::ZERO);
                logic.queue_audio_event(
                    crate::game_logic::AudioEventRequest::new(&a)
                        .with_object(id)
                        .with_position(pos)
                        .with_priority(200),
                );
            }
            logic.apply_pending_create_object_die(id);
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::ForceKill {
                    id,
                    death_type: None,
                    refresh_model_condition: true,
                    mark_destroy: true,
                },
            );
        }
        // Wave 774: SlowDeathBehavior done → host destroy (no dual timer).
        for id in crate::game_logic::host_slow_death_kill_log::drain() {
            // Wave 941: force-kill residual via host residual mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::ForceKill {
                    id,
                    death_type: None,
                    refresh_model_condition: false,
                    mark_destroy: true,
                },
            );
        }
        // Wave 775: StructureCollapseUpdate done → host destroy (no dual timer).
        for id in crate::game_logic::host_structure_collapse_kill_log::drain() {
            // Wave 941: force-kill residual via host residual mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::ForceKill {
                    id,
                    death_type: Some(crate::game_logic::host_usa_pilot::HostDeathType::Toppled),
                    refresh_model_condition: false,
                    mark_destroy: true,
                },
            );
        }
        // Wave 776: StructureToppleUpdate done → host destroy (no dual timer).
        for id in crate::game_logic::host_structure_topple_kill_log::drain() {
            // Wave 941: force-kill residual via host residual mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::ForceKill {
                    id,
                    death_type: Some(crate::game_logic::host_usa_pilot::HostDeathType::Toppled),
                    refresh_model_condition: false,
                    mark_destroy: true,
                },
            );
        }
        // Wave 777: StructureTopple crush sweep → host apply (no dual last_crushed).
        for (id, samples) in crate::game_logic::host_structure_topple_crush_log::drain() {
            logic.apply_structure_topple_crush_samples(id, samples);
        }
        // Wave 778: FWWDB continuous → host pending fire (no dual continuous timer).
        for (id, weapon) in crate::game_logic::host_fwwd_continuous_log::drain() {
            // Wave 941: continuous FWWDB pending fire via residual mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::SetPendingFireWhenDamaged {
                    id,
                    weapon,
                    overwrite: false,
                },
            );
        }
        // Wave 779: FWWDB reaction → host pending fire (no dual reaction debounce).
        for (id, weapon) in crate::game_logic::host_fwwd_reaction_log::drain() {
            // Wave 941: reaction FWWDB pending fire via residual mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::SetPendingFireWhenDamaged {
                    id,
                    weapon,
                    overwrite: true,
                },
            );
        }

        // Wave 788: DaisyCutter/MOAB drop + detonate (no dual flight).
        for ev in crate::game_logic::host_daisy_cutter_drop_log::drain_drops() {
            let bomb = ev.tier.bomb();
            let drop_pos = glam::Vec3::new(ev.target.x, 90.0, ev.target.z);
            if let Some(bid) =
                match logic.apply_host_object_id_op(crate::game_logic::HostObjectIdOp::Create {
                    template: bomb.to_string(),
                    team: ev.team,
                    spawn_at: drop_pos,
                }) {
                    crate::game_logic::HostObjectIdResult::Created(id) => id,
                    _ => None,
                }
            {
                // Wave 942: post-create payload config via mutation authority.
                let moab_template = if matches!(
                    ev.tier,
                    crate::game_logic::host_daisy_cutter_flight::DaisyFlightPayloadTier::Moab
                ) {
                    Some(bomb.to_string())
                } else {
                    None
                };
                logic.apply_host_residual_mutation_op(
                    crate::game_logic::HostResidualMutationOp::ConfigureSpawnedPayload {
                        id: bid,
                        producer: ev.producer,
                        target: ev.target,
                        kind: crate::game_logic::SpawnedPayloadKind::DaisyCutter { moab_template },
                    },
                );
                logic.daisy_cutter_flight_reg.record_drop();
            }
        }
        for ev in crate::game_logic::host_daisy_cutter_drop_log::drain_dets() {
            use crate::game_logic::combat::DamageType;
            let _ = logic.apply_fuel_air_radius_damage(
                ev.bomb,
                ev.producer,
                ev.team,
                ev.pos,
                ev.tier.primary_damage(),
                ev.tier.primary_radius(),
                DamageType::Explosive,
            );
            // Wave 942: bomb destroy via mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::DestroyBomb {
                    id: ev.bomb,
                    mark_destroy: true,
                },
            );
        }
        // Wave 789: AnthraxBomb drop + detonate (no dual flight).
        for ev in crate::game_logic::host_anthrax_bomb_drop_log::drain_drops() {
            let bomb = ev.tier.bomb();
            let drop_pos =
                crate::game_logic::host_anthrax_bomb_flight::anthrax_payload_drop_pos(ev.plane_pos);
            if let Some(bid) =
                match logic.apply_host_object_id_op(crate::game_logic::HostObjectIdOp::Create {
                    template: bomb.to_string(),
                    team: ev.team,
                    spawn_at: drop_pos,
                }) {
                    crate::game_logic::HostObjectIdResult::Created(id) => id,
                    _ => None,
                }
            {
                // Wave 942: post-create payload config via mutation authority.
                logic.apply_host_residual_mutation_op(
                    crate::game_logic::HostResidualMutationOp::ConfigureSpawnedPayload {
                        id: bid,
                        producer: ev.producer,
                        target: ev.target,
                        kind: crate::game_logic::SpawnedPayloadKind::AnthraxBomb,
                    },
                );
                logic.anthrax_bomb_flight_reg.record_drop();
            }
        }
        for ev in crate::game_logic::host_anthrax_bomb_drop_log::drain_dets() {
            use crate::game_logic::combat::DamageType;
            use crate::game_logic::special_power_strikes::{
                ANTHRAX_BOMB_IMPACT_DAMAGE, ANTHRAX_BOMB_IMPACT_RADIUS,
            };
            let _ = logic.apply_fuel_air_radius_damage(
                ev.bomb,
                ev.producer,
                ev.team,
                ev.pos,
                ANTHRAX_BOMB_IMPACT_DAMAGE,
                ANTHRAX_BOMB_IMPACT_RADIUS,
                DamageType::Explosive,
            );
            let src = ev.producer.unwrap_or(ev.bomb);
            let toxin_object = logic
                .objects
                .get(&ev.bomb)
                .map(|o| {
                    crate::game_logic::host_anthrax_bomb_flight::AnthraxBombPayloadTier::from_ocl(
                        &o.template_name,
                    )
                    .toxin_object()
                })
                .unwrap_or(crate::game_logic::special_power_strikes::ANTHRAX_TOXIN_OBJECT_NAME);
            let _ = logic.special_power_strikes.spawn_toxin_field_with_params(
                src,
                ev.team,
                ev.pos,
                logic.frame,
                0,
                crate::game_logic::special_power_strikes::ANTHRAX_TOXIN_DAMAGE_PER_TICK,
                crate::game_logic::special_power_strikes::ANTHRAX_TOXIN_RADIUS,
                crate::game_logic::special_power_strikes::ANTHRAX_TOXIN_TICK_INTERVAL_FRAMES,
                crate::game_logic::special_power_strikes::ANTHRAX_TOXIN_DURATION_FRAMES,
                toxin_object,
            );
            logic.anthrax_bomb_flight_reg.record_toxin_field();
            // Wave 942: bomb destroy via mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::DestroyBomb {
                    id: ev.bomb,
                    mark_destroy: true,
                },
            );
            logic.anthrax_bomb_flight_reg.record_detonation();
        }
        // Wave 790: ClusterMines drop + detonate (no dual flight).
        for ev in crate::game_logic::host_cluster_mines_drop_log::drain_drops() {
            use crate::game_logic::host_cluster_mines_flight::CLUSTER_MINES_BOMB_OBJECT;
            let drop_pos = glam::Vec3::new(ev.target.x, 80.0, ev.target.z);
            if let Some(bid) =
                match logic.apply_host_object_id_op(crate::game_logic::HostObjectIdOp::Create {
                    template: CLUSTER_MINES_BOMB_OBJECT.to_string(),
                    team: ev.team,
                    spawn_at: drop_pos,
                }) {
                    crate::game_logic::HostObjectIdResult::Created(id) => id,
                    _ => None,
                }
            {
                // Wave 942: post-create payload config via mutation authority.
                logic.apply_host_residual_mutation_op(
                    crate::game_logic::HostResidualMutationOp::ConfigureSpawnedPayload {
                        id: bid,
                        producer: ev.producer,
                        target: ev.target,
                        kind: crate::game_logic::SpawnedPayloadKind::ClusterMinesBomb,
                    },
                );
                logic.cluster_mines_flight_reg.record_drop();
            }
        }
        for ev in crate::game_logic::host_cluster_mines_drop_log::drain_dets() {
            let mines = logic.place_cluster_mines(ev.team, ev.pos, ev.producer);
            logic
                .cluster_mines_flight_reg
                .record_minefield(mines.len() as u32);
            // Wave 942: bomb destroy via mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::DestroyBomb {
                    id: ev.bomb,
                    mark_destroy: true,
                },
            );
        }
        // Wave 791: EMP Pulse drop + detonate + spheroid expire (no dual flight).
        for ev in crate::game_logic::host_emp_pulse_drop_log::drain_drops() {
            use crate::game_logic::host_emp_pulse::EMP_PULSE_BOMB_TEMPLATE;
            let drop_pos = glam::Vec3::new(ev.target.x, 80.0, ev.target.z);
            if let Some(bid) =
                match logic.apply_host_object_id_op(crate::game_logic::HostObjectIdOp::Create {
                    template: EMP_PULSE_BOMB_TEMPLATE.to_string(),
                    team: ev.team,
                    spawn_at: drop_pos,
                }) {
                    crate::game_logic::HostObjectIdResult::Created(id) => id,
                    _ => None,
                }
            {
                // Wave 942: post-create payload config via mutation authority.
                logic.apply_host_residual_mutation_op(
                    crate::game_logic::HostResidualMutationOp::ConfigureSpawnedPayload {
                        id: bid,
                        producer: ev.producer,
                        target: ev.target,
                        kind: crate::game_logic::SpawnedPayloadKind::EmpPulseBomb,
                    },
                );
                let _ = (ev.player_id, ev.caster_id);
                logic.emp_pulse_flight_reg.record_drop();
            }
        }
        for ev in crate::game_logic::host_emp_pulse_drop_log::drain_dets() {
            let player_id = ev
                .producer
                .and_then(|pid| logic.host_objects().get(&pid))
                .and_then(|o| {
                    logic
                        .get_players()
                        .iter()
                        .find(|(_, p)| p.team == o.team)
                        .map(|(id, _)| *id)
                })
                .unwrap_or(0);
            let _ = logic.apply_emp_pulse_at(player_id, ev.pos, ev.producer);
            logic.emp_pulse_flight_reg.record_detonation();
            // Wave 942: bomb destroy via mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::DestroyBomb {
                    id: ev.bomb,
                    mark_destroy: true,
                },
            );
        }
        for ev in crate::game_logic::host_emp_pulse_drop_log::drain_spheroid_expires() {
            // Wave 942: emp spheroid expire via mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::LethalExpire {
                    id: ev.id,
                    position: None,
                    effectively_dead: true,
                    clear: crate::game_logic::ObjectIdentityClear::EmpPulseSpheroid,
                    mark_destroy_team: Some(None),
                },
            );
        }
        logic.apply_due_emp_pulse_disables();
        // Wave 792: A10 drop + detonate (no dual flight).
        // Keep host pending registry in sync with GW-consumed drops.
        logic.a10_strike_flight_reg.pending_drops = shadow.a10_pending_drops.clone();
        for ev in crate::game_logic::host_a10_strike_drop_log::drain_drops() {
            use crate::game_logic::special_power_strikes::A10_PAYLOAD_TEMPLATE;
            use crate::game_logic::{KindOf, ThingTemplate};
            if !logic.templates.contains_key(A10_PAYLOAD_TEMPLATE) {
                let mut t = ThingTemplate::new(A10_PAYLOAD_TEMPLATE);
                t.set_health(40.0).add_kind_of(KindOf::Projectile);
                logic.templates.insert(A10_PAYLOAD_TEMPLATE.to_string(), t);
            }
            let drop_pos = ev.spawn;
            if let Some(mid) =
                match logic.apply_host_object_id_op(crate::game_logic::HostObjectIdOp::Create {
                    template: A10_PAYLOAD_TEMPLATE.to_string(),
                    team: ev.team,
                    spawn_at: drop_pos,
                }) {
                    crate::game_logic::HostObjectIdResult::Created(id) => id,
                    _ => None,
                }
            {
                // Wave 942: post-create payload config via mutation authority.
                logic.apply_host_residual_mutation_op(
                    crate::game_logic::HostResidualMutationOp::ConfigureSpawnedPayload {
                        id: mid,
                        producer: ev.producer,
                        target: ev.target,
                        kind: crate::game_logic::SpawnedPayloadKind::A10StrikeMissile,
                    },
                );
                logic.a10_strike_flight_reg.record_drop();
            }
        }
        for ev in crate::game_logic::host_a10_strike_drop_log::drain_dets() {
            use crate::game_logic::combat::DamageType;
            use crate::game_logic::special_power_strikes::{
                A10_MISSILE_PRIMARY_DAMAGE, A10_MISSILE_PRIMARY_RADIUS,
            };
            let _ = logic.apply_fuel_air_radius_damage(
                ev.missile,
                ev.producer,
                ev.team,
                ev.pos,
                A10_MISSILE_PRIMARY_DAMAGE,
                A10_MISSILE_PRIMARY_RADIUS,
                DamageType::Explosive,
            );
            logic.a10_strike_flight_reg.record_impact();
            // Wave 942: bomb destroy via mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::DestroyBomb {
                    id: ev.missile,
                    mark_destroy: true,
                },
            );
        }
        for ev in crate::game_logic::host_a10_strike_drop_log::drain_vulcans() {
            use crate::game_logic::combat::DamageType;
            use crate::game_logic::special_power_strikes::{
                A10_VULCAN_PRIMARY_DAMAGE, A10_VULCAN_PRIMARY_RADIUS,
            };
            let _ = logic.apply_fuel_air_radius_damage(
                ev.jet,
                ev.producer,
                ev.team,
                ev.pos,
                A10_VULCAN_PRIMARY_DAMAGE,
                A10_VULCAN_PRIMARY_RADIUS,
                DamageType::Bullet,
            );
        }
        for ev in crate::game_logic::host_a10_strike_drop_log::drain_dive_starts() {
            use crate::game_logic::audio_dispatch_impl::resolve_per_unit_sound;
            use crate::game_logic::host_a10_strike_flight::A10_START_DIVE_SOUND;
            use crate::game_logic::special_power_strikes::A10_TRANSPORT;
            if let Some(name) = resolve_per_unit_sound(A10_TRANSPORT, A10_START_DIVE_SOUND) {
                logic.queue_audio_event(
                    crate::game_logic::AudioEventRequest::new(&name)
                        .with_object(ev.jet)
                        .with_position(ev.pos)
                        .with_priority(160),
                );
            }
        }
        // Wave 793: ArtilleryBarrage drop + detonate (no dual flight).
        logic.artillery_barrage_flight_reg.pending_drops = shadow.artillery_pending_drops.clone();
        for ev in crate::game_logic::host_artillery_barrage_drop_log::drain_drops() {
            use crate::game_logic::special_power_strikes::ARTILLERY_BARRAGE_SHELL_OBJECT;
            use crate::game_logic::{KindOf, ThingTemplate};
            if !logic.templates.contains_key(ARTILLERY_BARRAGE_SHELL_OBJECT) {
                let mut t = ThingTemplate::new(ARTILLERY_BARRAGE_SHELL_OBJECT);
                t.set_health(50.0).add_kind_of(KindOf::Projectile);
                logic
                    .templates
                    .insert(ARTILLERY_BARRAGE_SHELL_OBJECT.to_string(), t);
            }
            let drop_pos = glam::Vec3::new(ev.target.x, 100.0, ev.target.z);
            if let Some(sid) =
                match logic.apply_host_object_id_op(crate::game_logic::HostObjectIdOp::Create {
                    template: ARTILLERY_BARRAGE_SHELL_OBJECT.to_string(),
                    team: ev.team,
                    spawn_at: drop_pos,
                }) {
                    crate::game_logic::HostObjectIdResult::Created(id) => id,
                    _ => None,
                }
            {
                // Wave 942: post-create payload config via mutation authority.
                logic.apply_host_residual_mutation_op(
                    crate::game_logic::HostResidualMutationOp::ConfigureSpawnedPayload {
                        id: sid,
                        producer: ev.producer,
                        target: ev.target,
                        kind: crate::game_logic::SpawnedPayloadKind::ArtilleryBarrageShell,
                    },
                );
                logic.artillery_barrage_flight_reg.record_drop();
            }
        }
        for ev in crate::game_logic::host_artillery_barrage_drop_log::drain_dets() {
            use crate::game_logic::combat::DamageType;
            use crate::game_logic::special_power_strikes::{
                ARTILLERY_BARRAGE_DAMAGE, ARTILLERY_BARRAGE_RADIUS,
            };
            let _ = logic.apply_fuel_air_radius_damage(
                ev.shell,
                ev.producer,
                ev.team,
                ev.pos,
                ARTILLERY_BARRAGE_DAMAGE,
                ARTILLERY_BARRAGE_RADIUS,
                DamageType::Explosive,
            );
            logic.artillery_barrage_flight_reg.record_impact();
            // Wave 942: bomb destroy via mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::DestroyBomb {
                    id: ev.shell,
                    mark_destroy: true,
                },
            );
        }
        // Wave 794: CarpetBomb drop + detonate (no dual flight).
        logic.carpet_bomb_flight_reg.pending_drops = shadow.carpet_pending_drops.clone();
        for ev in crate::game_logic::host_carpet_bomb_drop_log::drain_drops() {
            use crate::game_logic::special_power_strikes::CARPET_BOMB_PAYLOAD_OBJECT;
            use crate::game_logic::{KindOf, ThingTemplate};
            if !logic.templates.contains_key(CARPET_BOMB_PAYLOAD_OBJECT) {
                let mut t = ThingTemplate::new(CARPET_BOMB_PAYLOAD_OBJECT);
                t.set_health(100.0).add_kind_of(KindOf::Projectile);
                logic
                    .templates
                    .insert(CARPET_BOMB_PAYLOAD_OBJECT.to_string(), t);
            }
            let drop_pos = glam::Vec3::new(ev.target.x, 80.0, ev.target.z);
            if let Some(bid) =
                match logic.apply_host_object_id_op(crate::game_logic::HostObjectIdOp::Create {
                    template: CARPET_BOMB_PAYLOAD_OBJECT.to_string(),
                    team: ev.team,
                    spawn_at: drop_pos,
                }) {
                    crate::game_logic::HostObjectIdResult::Created(id) => id,
                    _ => None,
                }
            {
                // Wave 942: post-create payload config via mutation authority.
                logic.apply_host_residual_mutation_op(
                    crate::game_logic::HostResidualMutationOp::ConfigureSpawnedPayload {
                        id: bid,
                        producer: ev.producer,
                        target: ev.target,
                        kind: crate::game_logic::SpawnedPayloadKind::CarpetBomb,
                    },
                );
                logic.carpet_bomb_flight_reg.record_drop();
            }
        }
        for ev in crate::game_logic::host_carpet_bomb_drop_log::drain_dets() {
            use crate::game_logic::combat::DamageType;
            use crate::game_logic::special_power_strikes::{
                CARPET_BOMB_DAMAGE, CARPET_BOMB_RADIUS,
            };
            let _ = logic.apply_fuel_air_radius_damage(
                ev.bomb,
                ev.producer,
                ev.team,
                ev.pos,
                CARPET_BOMB_DAMAGE,
                CARPET_BOMB_RADIUS,
                DamageType::Explosive,
            );
            logic.carpet_bomb_flight_reg.record_impact();
            // Wave 942: bomb destroy via mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::DestroyBomb {
                    id: ev.bomb,
                    mark_destroy: true,
                },
            );
        }
        // Wave 795: Leaflet B52 drop + container ground (no dual flight).
        for ev in crate::game_logic::host_leaflet_b52_drop_log::drain_drops() {
            use crate::game_logic::host_leaflet_drop::LEAFLET_CONTAINER_OBJECT;
            let drop_pos = glam::Vec3::new(ev.target.x, 80.0, ev.target.z);
            if let Some(cid) =
                match logic.apply_host_object_id_op(crate::game_logic::HostObjectIdOp::Create {
                    template: LEAFLET_CONTAINER_OBJECT.to_string(),
                    team: ev.team,
                    spawn_at: drop_pos,
                }) {
                    crate::game_logic::HostObjectIdResult::Created(id) => id,
                    _ => None,
                }
            {
                // Wave 942: post-create payload config via mutation authority.
                logic.apply_host_residual_mutation_op(
                    crate::game_logic::HostResidualMutationOp::ConfigureSpawnedPayload {
                        id: cid,
                        producer: ev.producer,
                        target: ev.target,
                        kind: crate::game_logic::SpawnedPayloadKind::LeafletContainer,
                    },
                );
                logic.host_leaflet_drops.containers_dropped = logic
                    .host_leaflet_drops
                    .containers_dropped
                    .saturating_add(1);
            }
        }
        for ev in crate::game_logic::host_leaflet_b52_drop_log::drain_ground() {
            // Wave 942: lethal expire residual via mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::LethalExpire {
                    id: ev.id,
                    position: None,
                    effectively_dead: true,
                    clear: crate::game_logic::ObjectIdentityClear::LeafletContainer,
                    mark_destroy_team: Some(None),
                },
            );
        }
        // Wave 796: Paradrop cargo drop + parachute ground (no dual flight).
        for ev in crate::game_logic::host_paradrop_cargo_drop_log::drain_drops() {
            use crate::game_logic::host_paradrop::PARADROP_PARACHUTE_CONTAINER;
            let drop_pos = glam::Vec3::new(ev.target.x, 100.0, ev.target.z);
            if let Some(pid) =
                match logic.apply_host_object_id_op(crate::game_logic::HostObjectIdOp::Create {
                    template: PARADROP_PARACHUTE_CONTAINER.to_string(),
                    team: ev.team,
                    spawn_at: drop_pos,
                }) {
                    crate::game_logic::HostObjectIdResult::Created(id) => id,
                    _ => None,
                }
            {
                // Wave 942: post-create payload config via mutation authority.
                logic.apply_host_residual_mutation_op(
                    crate::game_logic::HostResidualMutationOp::ConfigureSpawnedPayload {
                        id: pid,
                        producer: ev.producer,
                        target: ev.target,
                        kind: crate::game_logic::SpawnedPayloadKind::ParadropParachute,
                    },
                );
                logic.host_paradrops.parachutes_dropped =
                    logic.host_paradrops.parachutes_dropped.saturating_add(1);
            }
        }
        for ev in crate::game_logic::host_paradrop_cargo_drop_log::drain_ground() {
            // Wave 942: lethal expire residual via mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::LethalExpire {
                    id: ev.id,
                    position: None,
                    effectively_dead: true,
                    clear: crate::game_logic::ObjectIdentityClear::ParadropCargo,
                    mark_destroy_team: Some(None),
                },
            );
        }
        // Wave 797: AuroraBomb projectile arrive/stale destroy (no dual flight).
        for ev in crate::game_logic::host_aurora_bomb_projectile_log::drain_destroys() {
            // Wave 942: lethal expire residual via mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::LethalExpire {
                    id: ev.id,
                    position: ev.snap_aim.map(|a| glam::Vec3::new(a[0], a[1], a[2])),
                    effectively_dead: true,
                    clear: crate::game_logic::ObjectIdentityClear::AuroraBombProjectile,
                    mark_destroy_team: Some(None),
                },
            );
        }
        // Wave 798: ToxinStream projectile stream + impact (no dual flight).
        for ev in crate::game_logic::host_toxin_stream_projectile_log::drain_streams() {
            use crate::game_logic::host_toxin_tractor::TOXIN_STREAM_NAME;
            logic.projectile_streams.add_projectile(
                ev.shooter,
                TOXIN_STREAM_NAME,
                ev.pos,
                ev.intended,
                Some(ev.aim),
                logic.frame,
            );
        }
        for ev in crate::game_logic::host_toxin_stream_projectile_log::drain_impacts() {
            let source_team = ev
                .source
                .and_then(|sid| logic.host_objects().get(&sid).map(|o| o.team))
                .unwrap_or(ev.team);
            // Wave 942: projectile lethal expire via mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::LethalExpire {
                    id: ev.id,
                    position: None,
                    effectively_dead: true,
                    clear: crate::game_logic::ObjectIdentityClear::ToxinStreamProjectile,
                    mark_destroy_team: Some(None),
                },
            );
            logic.apply_toxin_tractor_stream_at(ev.pos, ev.source, ev.intended, source_team);
        }
        // Wave 799: AngryMob projectile impact (no dual flight).
        for ev in crate::game_logic::host_angry_mob_projectile_log::drain_impacts() {
            use crate::game_logic::host_angry_mob::AngryMobProjectileKind;
            let team = logic.host_objects().get(&ev.id).map(|o| o.team);
            // Wave 942: projectile lethal expire via mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::LethalExpire {
                    id: ev.id,
                    position: Some(ev.pos),
                    effectively_dead: true,
                    clear: crate::game_logic::ObjectIdentityClear::AngryMobProjectile,
                    mark_destroy_team: Some(team),
                },
            );
            let kind = AngryMobProjectileKind::from_u8(ev.kind);
            let _ = logic.apply_angry_mob_projectile_at(ev.pos, ev.source, ev.intended, kind);
        }
        // Wave 800: SCUD/Neutron/Nuke shell impacts (no dual flight).
        for ev in crate::game_logic::host_cannon_shell_projectile_log::drain_impacts() {
            use crate::game_logic::host_cannon_shell_projectile_log::CannonShellKind;
            // Wave 942: cannon shell lethal expire via mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::LethalExpire {
                    id: ev.id,
                    position: Some(ev.pos),
                    effectively_dead: true,
                    clear: crate::game_logic::ObjectIdentityClear::CannonShellProjectile,
                    mark_destroy_team: Some(Some(ev.team)),
                },
            );
            match ev.kind {
                CannonShellKind::Scud { toxin } => {
                    let _ = logic.apply_scud_area_at(ev.pos, ev.source, ev.team, toxin);
                }
                CannonShellKind::Neutron => {
                    let caster_team = ev
                        .source
                        .and_then(|sid| logic.host_objects().get(&sid).map(|s| s.team))
                        .unwrap_or(ev.team);
                    let _ = logic.apply_neutron_blast_at(ev.pos, caster_team, ev.source, true);
                }
                CannonShellKind::Nuke => {
                    let _ = logic.apply_nuke_cannon_primary_at(ev.pos, ev.source, ev.team);
                }
            }
        }
        // Wave 801: AngryMob member destroy when nexus lost (no dual follow).
        for id in crate::game_logic::host_angry_mob_member_follow_log::drain_destroys() {
            // Wave 942: lethal expire residual via mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::LethalExpire {
                    id: id,
                    position: None,
                    effectively_dead: true,
                    clear: crate::game_logic::ObjectIdentityClear::AngryMobMember,
                    mark_destroy_team: Some(None),
                },
            );
        }
        // Wave 802: field-object lifetime expire (no dual timer).
        for ev in crate::game_logic::host_field_object_expire_log::drain() {
            use crate::game_logic::host_field_object_expire_log::FieldObjectKind;
            // Wave 942: field object expire via mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::LethalExpire {
                    id: ev.id,
                    position: None,
                    effectively_dead: true,
                    clear: crate::game_logic::ObjectIdentityClear::FieldObject(ev.kind),
                    mark_destroy_team: Some(ev.team),
                },
            );
            // Wave 806: countermeasure flare producer bookkeeping.
            if matches!(ev.kind, FieldObjectKind::CountermeasureFlare) {
                if let Some(pid) = ev.producer {
                    logic.countermeasures.note_flare_expired(pid);
                }
            }
            if matches!(ev.kind, FieldObjectKind::MoneyCrate) {
                logic.host_money_crates.forget(ev.id);
            }
        }
        // Wave 803: Inferno shell impact + SpySatellite ping expire.
        for ev in crate::game_logic::host_inferno_shell_projectile_log::drain_impacts() {
            // Wave 942: projectile lethal expire via mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::LethalExpire {
                    id: ev.id,
                    position: None,
                    effectively_dead: true,
                    clear: crate::game_logic::ObjectIdentityClear::InfernoShellProjectile,
                    mark_destroy_team: Some(None),
                },
            );
            let _ = logic.apply_inferno_shell_residual_at(ev.pos, ev.source, ev.intended);
        }
        for id in crate::game_logic::host_spy_satellite_ping_log::drain_expires() {
            // Wave 942: lethal expire residual via mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::LethalExpire {
                    id: id,
                    position: None,
                    effectively_dead: true,
                    clear: crate::game_logic::ObjectIdentityClear::SpySatellitePing,
                    mark_destroy_team: Some(None),
                },
            );
        }
        // Wave 804: Flashbang impact + Comanche rocket expire.
        for ev in crate::game_logic::host_flashbang_comanche_helix_projectile_log::drain_flashbang()
        {
            // Wave 942: projectile lethal expire via mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::LethalExpire {
                    id: ev.id,
                    position: Some(ev.pos),
                    effectively_dead: true,
                    clear: crate::game_logic::ObjectIdentityClear::FlashbangGrenadeProjectile,
                    mark_destroy_team: Some(None),
                },
            );
            let _ = logic.apply_ranger_residual_at(ev.pos, ev.source, ev.intended, true);
        }
        for id in
            crate::game_logic::host_flashbang_comanche_helix_projectile_log::drain_comanche_expires(
            )
        {
            // Wave 942: comanche rocket expire via mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::LethalExpire {
                    id: id,
                    position: None,
                    effectively_dead: true,
                    clear: crate::game_logic::ObjectIdentityClear::ComancheRocketPodProjectile,
                    mark_destroy_team: Some(None),
                },
            );
        }
        // Wave 805: Scorpion missile impact residual.
        for ev in crate::game_logic::host_scorpion_missile_projectile_log::drain_impacts() {
            // Wave 942: projectile lethal expire via mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::LethalExpire {
                    id: ev.id,
                    position: Some(ev.pos),
                    effectively_dead: true,
                    clear: crate::game_logic::ObjectIdentityClear::ScorpionMissileProjectile,
                    mark_destroy_team: Some(None),
                },
            );
            let _ = logic.apply_scorpion_residual_at(ev.pos, ev.source, ev.intended, ev.slot);
        }
        // Wave 807: Sticky bomb / booby-trap attach follow + orphan destroy.
        for ev in crate::game_logic::host_sticky_booby_attach_log::drain_sticky_follows() {
            // Wave 942: sticky follow position via mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::SetPosition {
                    id: ev.id,
                    position: ev.pos,
                    sticky_follow_tick: true,
                },
            );
        }
        for ev in crate::game_logic::host_sticky_booby_attach_log::drain_sticky_destroys() {
            logic.sticky_bomb_target_deaths = logic.sticky_bomb_target_deaths.saturating_add(1);
            logic.destroy_object(ev.id);
        }
        for ev in crate::game_logic::host_sticky_booby_attach_log::drain_booby_follows() {
            // Wave 942: booby follow position via mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::SetPosition {
                    id: ev.id,
                    position: ev.pos,
                    sticky_follow_tick: false,
                },
            );
        }
        for ev in crate::game_logic::host_sticky_booby_attach_log::drain_booby_destroys() {
            logic.destroy_booby_trap_special_object(ev.id);
        }
        // Wave 810: Power plant rods completion residual.
        for ev in crate::game_logic::host_power_plant_rods_log::drain_completes() {
            // Wave 942: power-plant rods complete via mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::PowerPlantRodsComplete {
                    id: ev.id,
                    model_condition_bits: ev.model_condition_bits,
                },
            );
        }
        // Wave 812: Battlemaster horde status residual.
        for ev in crate::game_logic::host_battlemaster_horde_log::drain() {
            // Wave 942: battlemaster horde residual via mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::SetWeaponBonusHorde {
                    id: ev.id,
                    now_horde: ev.now_horde,
                    was_horde: ev.was_horde,
                    grant: crate::game_logic::HordeGrantCounter::Battlemaster,
                },
            );
        }
        // Wave 813: China infantry horde status residual.
        for ev in crate::game_logic::host_china_infantry_horde_log::drain() {
            use crate::game_logic::host_china_infantry_horde_log::ChinaInfantryHordeKind;
            // Wave 942: china infantry horde residual via mutation authority.
            let grant = match ev.kind {
                ChinaInfantryHordeKind::RedGuard => crate::game_logic::HordeGrantCounter::RedGuard,
                ChinaInfantryHordeKind::TankHunter => {
                    crate::game_logic::HordeGrantCounter::TankHunter
                }
                ChinaInfantryHordeKind::Minigunner => {
                    crate::game_logic::HordeGrantCounter::Minigunner
                }
            };
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::SetWeaponBonusHorde {
                    id: ev.id,
                    now_horde: ev.now_horde,
                    was_horde: ev.was_horde,
                    grant,
                },
            );
        }
        // Wave 814: Stinger hive respawn residual.
        for ev in crate::game_logic::host_stinger_hive_log::drain() {
            // Wave 942: stinger hive slave residual via mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::ApplyStingerHiveState {
                    id: ev.id,
                    hive_slave_count: ev.hive_slave_count,
                    hive_slave_hp: ev.hive_slave_hp,
                    hive_slave_respawn_frame: ev.hive_slave_respawn_frame,
                    slaves_alive: ev.slaves_alive,
                    slaves_hp: ev.slaves_hp,
                },
            );
        }
        // Wave 818: player radar transitions residual.
        for ev in crate::game_logic::host_player_radar_log::drain() {
            use crate::game_logic::host_radar::{RADAR_OFFLINE_AUDIO, RADAR_ONLINE_AUDIO};
            if let Some(p) = logic.get_player_mut(ev.player_id) {
                p.set_radar_state(ev.radar_count, p.radar_disabled);
            }
            let (came_online, went_offline) = logic.host_radar.record_player_radar(
                ev.radar_count.max(0) as u32,
                ev.had_radar,
                ev.has_radar,
            );
            if came_online {
                logic.queue_audio_event(
                    crate::game_logic::game_logic::AudioEventRequest::new(RADAR_ONLINE_AUDIO)
                        .with_priority(130),
                );
            } else if went_offline {
                logic.queue_audio_event(
                    crate::game_logic::game_logic::AudioEventRequest::new(RADAR_OFFLINE_AUDIO)
                        .with_priority(130),
                );
            }
        }
        // Wave 820: fire-spread tick residual.
        for ev in crate::game_logic::host_fire_spread_log::drain() {
            logic.apply_fire_spread_tick_event(ev);
        }

        // Wave 815: ACTIVELY_CONSTRUCTING model bit residual.
        for ev in crate::game_logic::host_actively_constructing_log::drain() {
            // Wave 942: model-condition residual via mutation authority.
            logic.apply_host_residual_mutation_op(
                crate::game_logic::HostResidualMutationOp::SetModelConditionBits {
                    id: ev.id,
                    bits: ev.model_condition_bits,
                    count_update: true,
                },
            );
        }
        // Wave 819: dozer bored service acquire residual.
        for ev in crate::game_logic::host_dozer_bored_log::drain() {
            logic.process_dozer_bored_event(ev.id);
        }
        // Wave 821: black market / oil derrick AutoDeposit residual.
        for ev in crate::game_logic::host_auto_deposit_log::drain() {
            logic.apply_auto_deposit_event(ev);
        }

        // Wave 822: China Hacker HackInternet residual.
        for ev in crate::game_logic::host_hacker_income_log::drain() {
            logic.apply_hacker_income_event(ev);
        }

        // Wave 823–827/940: post-writeback sole-tick residuals via single authority batch.
        logic.apply_post_writeback_sole_ticks();

        // Wave 634: drain combat-status ready log after GW writeback.
        let _cst_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::CombatStatus);
    }
    let _prod_wb = shadow.writeback_production_to_host(logic);
    // Wave 714/937: same-frame host complete/spawn via production authority boundary after GW writeback.
    let _ = logic.apply_production_authority_op(
        crate::game_logic::ProductionAuthorityOp::ApplyCompletionsAfterReadyWriteback {
            dt: 1.0 / 30.0,
        },
    );
    let _ = shadow.writeback_production_door_to_host(logic);
    // Wave 627/937: drain production-door ready log via production authority boundary after GW writeback.
    let _door_ready = logic.apply_production_authority_op(
        crate::game_logic::ProductionAuthorityOp::ApplyDoorReadyCompletions,
    );
    let _ = shadow.writeback_body_damage_to_host(logic);
    // Wave 623: drain body-damage ready log after GW body-state writeback.
    let _body_ready =
        logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::BodyDamage);
    let _ = shadow.writeback_death_type_to_host(logic);
    // Wave 632: drain death-type ready log after GW writeback.
    let _dt_ready = logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::DeathType);
    let _ = shadow.writeback_radar_extend_to_host(logic);
    // Wave 625: drain radar-extend ready log after GW writeback.
    let _radar_ready =
        logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::RadarExtend);
    let _ = shadow.writeback_shock_stun_to_host(logic);
    // Wave 662: drain shock-stun ready log after GW writeback.
    let _w662_ready = logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::ShockStun);
    let _ = shadow.writeback_rebuild_producer_to_host(logic);
    // Wave 626: drain construction-complete-clear ready log after GW writeback.
    let _ccc_ready = logic
        .apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::ConstructionCompleteClear);
    let _ = shadow.writeback_sole_healing_to_host(logic);
    // Wave 663: drain sole-healing ready log after GW writeback.
    let _w663_ready =
        logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::SoleHealing);
    let _ = shadow.writeback_hijacker_to_host(logic);
    // Wave 647: drain hijacker ready log after GW writeback.
    let _hj_ready = logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Hijacker);
    let _ = shadow.writeback_ai_mood_to_host(logic);
    // Wave 645: drain AI-mood ready log after GW writeback.
    let _mood_ready = logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::AiMood);
    let _ = shadow.writeback_ai_request_to_host(logic);
    // Wave 648: drain AI-request ready log after GW writeback.
    let _air_ready = logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::AiRequest);
    let _ = shadow.writeback_hijacker_to_host(logic);
    // Wave 647: drain hijacker ready log after GW writeback.
    let _hj_ready = logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Hijacker);
    let _construction_wb = shadow.writeback_construction_to_host(logic);
    // Wave 715/938: same-frame host construction complete via post-writeback authority.
    logic.apply_post_writeback_complete_op(
        crate::game_logic::PostWritebackCompleteOp::ConstructionCompletionsAfterReadyWriteback,
    );
    // Wave 716/938: same-frame host sell finish via post-writeback authority.
    logic.apply_post_writeback_complete_op(
        crate::game_logic::PostWritebackCompleteOp::SellCompletionsAfterReadyWriteback,
    );
    let _owner_wb = shadow.writeback_owner_to_host(logic);
    // Wave 629: drain owner-ready log after GW owner writeback.
    let _owner_ready = logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Owner);
    let mut writebacks = 0usize;
    // HP last-writer: damage mutations and/or absolute heal SetHealth events.
    if auth && (!events.is_empty() || !heal_events.is_empty() || !experience_events.is_empty()) {
        let (mut queued, mut applied) = (0usize, 0usize);
        if !events.is_empty() {
            // Wave 684: skip GW re-apply when post-logic eager path already ran.
            if !early_damage_applied {
                let pair = shadow.apply_host_damage_events(&events);
                queued = pair.0;
                applied = pair.1;
            } else {
                queued = events.len();
                applied = events.len();
            }
            // Host objects with no shadow entity mapping would otherwise lose combat HP.
            // `ev.amount` is already post-armor — apply raw HP via mutation authority (Wave 943).
            if queued < events.len() || early_damage_applied {
                let fallback = logic.apply_host_unmapped_damage_fallback(&events, |id| {
                    shadow.entity_for_host(id).is_some()
                });
                if fallback > 0 {
                    log::trace!(
                        "damage authority host fallback applied={fallback} unmapped of {}",
                        events.len()
                    );
                }
            }
        }
        if !events.is_empty() || !heal_events.is_empty() {
            writebacks = shadow.writeback_health_to_host(logic);
        }
        let _xp_wb = shadow.writeback_experience_to_host(logic);
        // Wave 622: drain veterancy ready log after GW XP/level writeback.
        let _vet_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Veterancy);
        let _wbonus_wb = shadow.writeback_weapon_bonus_to_host(logic);
        // Wave 658: drain weapon-bonus ready log after GW writeback.
        let _w658_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::WeaponBonus);
        let _ff_wb = shadow.writeback_faerie_fire_to_host(logic);
        // Wave 676: drain faerie-fire ready log after GW writeback.
        let _w676_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::FaerieFire);
        let _rp_wb = shadow.writeback_repulsor_to_host(logic);
        // Wave 661: drain repulsor ready log after GW writeback.
        let _w661_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Repulsor);
        let _dt_wb = shadow.writeback_disable_timers_to_host(logic);
        // Wave 677: drain disable-timers ready log after GW writeback.
        let _w677_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::DisableTimers);
        let _wslot_wb = shadow.writeback_weapon_slot_to_host(logic);
        // Wave 657: drain weapon-slot ready log after GW writeback.
        let _w657_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::WeaponSlot);
        let _epow_wb = shadow.writeback_entity_power_to_host(logic);
        // Wave 674: drain entity-power ready log after GW writeback.
        let _w674_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::EntityPower);
        let _tur_wb = shadow.writeback_turret_to_host(logic);
        // Wave 673: drain turret ready log after GW writeback.
        let _w673_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Turret);
        let _ = shadow.writeback_stealth_delay_to_host(logic);
        // Wave 651: drain stealth-delay ready log after GW writeback.
        let _sd_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::StealthDelay);
        let _ = shadow.writeback_combat_attack_to_host(logic);
        // Wave 643: drain combat-attack ready log after GW writeback.
        let _ca_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::CombatAttack);
        let _ = shadow.writeback_fire_intent_to_host(logic);
        // Wave 640: drain fire-intent ready log after GW writeback.
        let _fi_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::FireIntent);
        let _ = shadow.writeback_locomotor_to_host(logic);
        // Wave 646: drain locomotor ready log after GW writeback.
        let _loco_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Locomotor);
        let _ = shadow.writeback_ai_request_to_host(logic);
        // Wave 648: drain AI-request ready log after GW writeback.
        let _air_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::AiRequest);
        let _ = shadow.writeback_hijacker_to_host(logic);
        // Wave 647: drain hijacker ready log after GW writeback.
        let _hj_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Hijacker);
        let _tloc_wb = shadow.writeback_target_location_to_host(logic);
        // Wave 672: drain target-location ready log after GW writeback.
        let _w672_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::TargetLocation);
        let _det_wb = shadow.writeback_detector_to_host(logic);
        // Wave 671: drain detector ready log after GW writeback.
        let _w671_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Detector);
        let _cf_wb = shadow.writeback_continuous_fire_to_host(logic);
        // Wave 670: drain continuous-fire ready log after GW writeback.
        let _w670_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::ContinuousFire);
        let _ = shadow.writeback_combat_attack_to_host(logic);
        // Wave 643: drain combat-attack ready log after GW writeback.
        let _ca_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::CombatAttack);
        let _ = shadow.writeback_fire_intent_to_host(logic);
        // Wave 640: drain fire-intent ready log after GW writeback.
        let _fi_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::FireIntent);
        let _ = shadow.writeback_locomotor_to_host(logic);
        // Wave 646: drain locomotor ready log after GW writeback.
        let _loco_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Locomotor);
        let _ = shadow.writeback_ai_request_to_host(logic);
        // Wave 648: drain AI-request ready log after GW writeback.
        let _air_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::AiRequest);
        let _ = shadow.writeback_hijacker_to_host(logic);
        // Wave 647: drain hijacker ready log after GW writeback.
        let _hj_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Hijacker);
        let _guard_wb = shadow.writeback_guard_to_host(logic);
        // Wave 669: drain guard ready log after GW writeback.
        let _w669_ready = logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Guard);
        let _ = shadow.writeback_ai_request_to_host(logic);
        // Wave 648: drain AI-request ready log after GW writeback.
        let _air_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::AiRequest);
        let _ = shadow.writeback_hijacker_to_host(logic);
        // Wave 647: drain hijacker ready log after GW writeback.
        let _hj_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Hijacker);
        let _ai_st_wb = shadow.writeback_ai_state_to_host(logic);
        // Wave 630: drain AI-state ready log after GW writeback.
        let _ai_st_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::AiState);
        let _att_wb = shadow.writeback_ai_attitude_to_host(logic);
        // Wave 659: drain AI-attitude ready log after GW writeback.
        let _w659_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::AiAttitude);
        let _wset_wb = shadow.writeback_weapon_set_to_host(logic);
        // Wave 642: drain weapon-set ready log after GW writeback.
        let _wset_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::WeaponSet);
        let _oc_wb = shadow.writeback_overcharge_to_host(logic);
        // Wave 668: drain overcharge ready log after GW writeback.
        let _w668_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Overcharge);
        let _cap_wb = shadow.writeback_contain_capacity_to_host(logic);
        let _hive_wb = shadow.writeback_hive_to_host(logic);
        // Wave 667: drain hive ready log after GW writeback.
        let _w667_ready = logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Hive);
        let _ = shadow.writeback_hijacker_to_host(logic);
        // Wave 647: drain hijacker ready log after GW writeback.
        let _hj_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Hijacker);
        let _stf_wb = shadow.writeback_stealth_flags_to_host(logic);
        // Wave 652: drain stealth-flags ready log after GW writeback.
        let _st_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::StealthFlags);
        let _ = shadow.writeback_stealth_delay_to_host(logic);
        // Wave 651: drain stealth-delay ready log after GW writeback.
        let _sd_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::StealthDelay);
        let _ = shadow.writeback_combat_attack_to_host(logic);
        // Wave 643: drain combat-attack ready log after GW writeback.
        let _ca_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::CombatAttack);
        let _ = shadow.writeback_fire_intent_to_host(logic);
        // Wave 640: drain fire-intent ready log after GW writeback.
        let _fi_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::FireIntent);
        let _ = shadow.writeback_locomotor_to_host(logic);
        // Wave 646: drain locomotor ready log after GW writeback.
        let _loco_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Locomotor);
        let _ = shadow.writeback_ai_request_to_host(logic);
        // Wave 648: drain AI-request ready log after GW writeback.
        let _air_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::AiRequest);
        let _ = shadow.writeback_hijacker_to_host(logic);
        // Wave 647: drain hijacker ready log after GW writeback.
        let _hj_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Hijacker);
        let _ol_wb = shadow.writeback_overlord_to_host(logic);
        // Wave 666: drain overlord ready log after GW writeback.
        let _w666_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Overlord);
        let _cs_wb = shadow.writeback_command_set_to_host(logic);
        // Wave 644: drain command-set ready log after GW writeback.
        let _cs_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::CommandSet);
        let _dg_wb = shadow.writeback_disguise_to_host(logic);
        // Wave 653: drain disguise ready log after GW writeback.
        let _di_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Disguise);
        let _vc_wb = shadow.writeback_vision_camo_to_host(logic);
        // Wave 654: drain vision-camo ready log after GW writeback.
        let _vi_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::VisionCamo);
        let _ = shadow.writeback_stealth_delay_to_host(logic);
        // Wave 651: drain stealth-delay ready log after GW writeback.
        let _sd_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::StealthDelay);
        let _ = shadow.writeback_combat_attack_to_host(logic);
        // Wave 643: drain combat-attack ready log after GW writeback.
        let _ca_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::CombatAttack);
        let _ = shadow.writeback_fire_intent_to_host(logic);
        // Wave 640: drain fire-intent ready log after GW writeback.
        let _fi_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::FireIntent);
        let _ = shadow.writeback_locomotor_to_host(logic);
        // Wave 646: drain locomotor ready log after GW writeback.
        let _loco_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Locomotor);
        let _ = shadow.writeback_ai_request_to_host(logic);
        // Wave 648: drain AI-request ready log after GW writeback.
        let _air_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::AiRequest);
        let _ = shadow.writeback_hijacker_to_host(logic);
        // Wave 647: drain hijacker ready log after GW writeback.
        let _hj_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Hijacker);
        let _ws_wb = shadow.writeback_weapon_stats_to_host(logic);
        // Wave 635: drain weapon-stats ready log after GW writeback.
        let _ws_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::WeaponStats);
        let _ = shadow.writeback_fire_intent_to_host(logic);
        // Wave 640: drain fire-intent ready log after GW writeback.
        let _fi_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::FireIntent);
        let _mv_wb = shadow.writeback_movement_to_host(logic);
        // Wave 637: drain movement ready log after GW writeback.
        let _mv_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Movement);
        let _ = shadow.writeback_locomotor_to_host(logic);
        // Wave 646: drain locomotor ready log after GW writeback.
        let _loco_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Locomotor);
        let _ = shadow.writeback_ai_request_to_host(logic);
        // Wave 648: drain AI-request ready log after GW writeback.
        let _air_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::AiRequest);
        let _ = shadow.writeback_hijacker_to_host(logic);
        // Wave 647: drain hijacker ready log after GW writeback.
        let _hj_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Hijacker);
        let _ = shadow.writeback_physics_motive_to_host(logic);
        // Wave 649: drain physics-motive ready log after GW writeback.
        let _pm_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::PhysicsMotive);
        let _ = shadow.writeback_locomotor_to_host(logic);
        // Wave 646: drain locomotor ready log after GW writeback.
        let _loco_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Locomotor);
        let _ = shadow.writeback_ai_request_to_host(logic);
        // Wave 648: drain AI-request ready log after GW writeback.
        let _air_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::AiRequest);
        let _ = shadow.writeback_hijacker_to_host(logic);
        // Wave 647: drain hijacker ready log after GW writeback.
        let _hj_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Hijacker);
        let _ = shadow.writeback_bounce_land_to_host(logic);
        // Wave 650: drain bounce-land ready log after GW writeback.
        let _bl_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::BounceLand);
        let _sr_wb = shadow.writeback_selection_radius_to_host(logic);
        // Wave 655: drain selection-radius ready log after GW writeback.
        let _w655_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::SelectionRadius);
        let _mc_wb = shadow.writeback_model_condition_to_host(logic);
        // Wave 633: drain model-condition ready log after GW writeback.
        let _mc_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::ModelCondition);
        let _dmc_wb = shadow.writeback_demo_mine_cheer_to_host(logic);
        // Wave 665: drain demo-mine-cheer ready log after GW writeback.
        let _w665_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::DemoMineCheer);
        let _cv_wb = shadow.writeback_crush_vision_to_host(logic);
        // Wave 664: drain crush-vision ready log after GW writeback.
        let _w664_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::CrushVision);
        let _bt_wb = shadow.writeback_building_type_to_host(logic);
        // Wave 675: drain building-type ready log after GW writeback.
        let _w675_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::BuildingType);
        let _id_wb = shadow.writeback_identity_to_host(logic);
        // Wave 660: drain identity ready log after GW writeback.
        let _w660_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Identity);
        let _gh_wb = shadow.writeback_ground_height_to_host(logic);
        // Wave 656: drain ground-height ready log after GW writeback.
        let _w656_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::GroundHeight);

        let _cst_wb = shadow.writeback_combat_status_to_host(logic);
        // Wave 634: drain combat-status ready log after GW writeback.
        let _cst_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::CombatStatus);
        log::trace!(
            "gameworld_damage_authority events={} queued={} applied={} writebacks={}",
            events.len(),
            queued,
            applied,
            writebacks
        );
    } else if !events.is_empty() {
        log::trace!(
            "gameworld_shadow session saw {} damage events (health via host sync)",
            events.len()
        );
    }
    let mut econ_wb = 0usize;
    if gameworld_economy_authority_live() {
        let econ_events = crate::game_logic::host_economy_log::drain();
        if !econ_events.is_empty() {
            // Keep pre-tick shadow supplies when re-applying absolute events.
            // (sync already copied host post-change supplies when write_health path
            //  also refreshed players — re-apply is idempotent absolute set.)
            let (_q, _a) = shadow.apply_host_economy_events(&econ_events);
        }
        econ_wb = shadow.writeback_economy_to_host(logic);
        // Wave 631: drain economy ready log after GW writeback.
        let _econ_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Economy);
        let _upg_wb = shadow.writeback_completed_upgrades_to_host(logic);
        // Wave 624: drain upgrade-ready log after GW completed-upgrade writeback.
        let _upg_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::Upgrade);
        let _ss_wb = shadow.writeback_stored_supplies_to_host(logic);
        // Wave 641: drain stored-supplies ready log after GW writeback.
        let _ss_ready =
            logic.apply_ready_log_drain_op(crate::game_logic::ReadyLogDrainOp::StoredSupplies);
    } else {
        // Avoid unbounded growth when economy authority off.
        let _ = crate::game_logic::host_economy_log::drain();
    }
    if gameworld_deferred_destroy_enabled() {
        let removed = shadow.world_mut().process_destroy_list();
        if removed > 0 {
            shadow.invalidate_dead_entity_maps();
        }
    }
    let mut probe = shadow.probe(logic);
    if !events.is_empty() || econ_wb > 0 || !production_events.is_empty() {
        probe.detail = format!(
            "{}|dmg_events={}|spawns={}/{}|destroy={}/{}|prod={}|auth={}|wb={}|econ_wb={}",
            probe.detail,
            events.len(),
            spawn_events.len(),
            spawns_applied,
            destroy_events.len(),
            dest_q,
            production_events.len(),
            auth,
            writebacks,
            econ_wb
        );
    }
    probe
}
