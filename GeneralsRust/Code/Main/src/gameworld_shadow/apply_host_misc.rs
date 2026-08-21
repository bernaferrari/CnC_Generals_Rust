//! Remaining apply_host_* movement/identity/status batches and paired writebacks.

use super::*;
use crate::game_logic::{GameLogic, ObjectId, Team};
use gamelogic::world::entities::{EntityId, EntityProductionItem, TemplateRef, Transform};
use gamelogic::world::{GameWorld, PlayerId, WorldMutation, WorldSnapshot};
use std::collections::{HashMap, HashSet};

impl GameWorldShadow {
    pub fn apply_host_movement_events(
        &mut self,
        events: &[crate::game_logic::host_movement_log::HostMovementEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetMovement {
                    target: eid,
                    velocity: ev.velocity,
                    max_speed: ev.max_speed,
                    path_index: ev.path_index,
                    path_len: ev.path_len,
                    path_waypoints: ev.path_waypoints.clone(),
                    waiting_for_path: ev.waiting_for_path,
                    locomotor_surfaces: ev.locomotor_surfaces,
                    is_attack_path: ev.is_attack_path,
                    is_blocked_and_stuck: ev.is_blocked_and_stuck,
                    is_braking: ev.is_braking,
                    is_safe_path: ev.is_safe_path,
                    queue_for_path_frames: ev.queue_for_path_frames,
                    path_timestamp: ev.path_timestamp,
                    cur_max_blocked_speed: ev.cur_max_blocked_speed,
                    num_frames_blocked: ev.num_frames_blocked,
                    is_blocked: ev.is_blocked,
                    move_away_from_id: ev.move_away_from_id,
                    requested_victim_id: ev.requested_victim_id,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_physics_motive_events(
        &mut self,
        events: &[crate::game_logic::host_physics_motive_log::HostPhysicsMotiveEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetPhysicsMotive {
                    target: eid,
                    motive_frames_remaining: ev.motive_frames_remaining,
                    physics_mass: ev.physics_mass,
                    physics_accel: ev.physics_accel,
                    forward_friction: ev.forward_friction,
                    lateral_friction: ev.lateral_friction,
                    z_friction: ev.z_friction,
                    can_path_through_units: ev.can_path_through_units,
                    ignore_collisions_until_frame: ev.ignore_collisions_until_frame,
                    is_panicking: ev.is_panicking,
                    move_away_frames: ev.move_away_frames,
                    aerodynamic_friction: ev.aerodynamic_friction,
                    extra_friction: ev.extra_friction,
                    apply_friction_2d_when_airborne: ev.apply_friction_2d_when_airborne,
                    center_of_mass_offset: ev.center_of_mass_offset,
                    pitch_roll_yaw_factor: ev.pitch_roll_yaw_factor,
                    move_away_destination: ev.move_away_destination,
                    request_other_move_away_id: ev.request_other_move_away_id,
                    immune_to_falling_damage: ev.immune_to_falling_damage,
                    physics_current_overlap_id: ev.physics_current_overlap_id,
                    physics_previous_overlap_id: ev.physics_previous_overlap_id,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_locomotor_events(
        &mut self,
        events: &[crate::game_logic::host_locomotor_log::HostLocomotorEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetLocomotor {
                    target: eid,
                    is_approach_path: ev.is_approach_path,
                    on_invalid_movement_terrain: ev.on_invalid_movement_terrain,
                    was_airborne_last_frame: ev.was_airborne_last_frame,
                    can_move_backward: ev.can_move_backward,
                    moving_backwards: ev.moving_backwards,
                    no_slow_down_as_approaching_dest: ev.no_slow_down_as_approaching_dest,
                    turn_pivot_offset: ev.turn_pivot_offset,
                    wander_width_factor: ev.wander_width_factor,
                    loco_apply_2d_friction_airborne: ev.loco_apply_2d_friction_airborne,
                    allow_motive_force_while_airborne: ev.allow_motive_force_while_airborne,
                    loco_extra_2d_friction: ev.loco_extra_2d_friction,
                    loco_preferred_height: ev.loco_preferred_height,
                    loco_preferred_height_damping: ev.loco_preferred_height_damping,
                    loco_appearance_ordinal: ev.loco_appearance_ordinal,
                    loco_behavior_z_ordinal: ev.loco_behavior_z_ordinal,
                    min_turn_speed: ev.min_turn_speed,
                    physics_turning_ordinal: ev.physics_turning_ordinal,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_bounce_land_events(
        &mut self,
        events: &[crate::game_logic::host_bounce_land_log::HostBounceLandEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetBounceLand {
                    target: eid,
                    kill_when_resting_on_ground: ev.kill_when_resting_on_ground,
                    bounce_land_events: ev.bounce_land_events,
                    last_bounce_fall_dy: ev.last_bounce_fall_dy,
                    bounce_sound_name: ev.bounce_sound_name.clone(),
                    last_bounce_volume: ev.last_bounce_volume,
                    bounce_audio_pending: ev.bounce_audio_pending,
                    allow_collide_force: ev.allow_collide_force,
                    last_collidee_id: ev.last_collidee_id,
                    ignore_collisions_with_id: ev.ignore_collisions_with_id,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_selection_radius_events(
        &mut self,
        events: &[crate::game_logic::host_selection_radius_log::HostSelectionRadiusEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetSelectionRadius {
                    target: eid,
                    selection_radius: ev.selection_radius,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn writeback_selection_radius_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_selection_radius_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let Some(obj) = logic.host_objects().get(&ObjectId(hid)) else {
                continue;
            };
            if (obj.selection_radius - ent.selection_radius).abs() <= f32::EPSILON {
                continue;
            }
            // Wave 945: selection-radius writeback via host writeback authority.
            if !logic.apply_host_writeback_op(crate::game_logic::HostWritebackOp::SelectionRadius {
                id: ObjectId(hid),
                radius: ent.selection_radius,
            }) {
                continue;
            }
            // Wave 655: GameWorld selection-radius last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_selection_radius_ready_log::record(oid);
        }
        updated
    }

    pub fn apply_host_model_condition_events(
        &mut self,
        events: &[crate::game_logic::host_model_condition_log::HostModelConditionEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetModelCondition {
                    target: eid,
                    model_condition_bits: ev.model_condition_bits,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_formation_events(
        &mut self,
        events: &[crate::game_logic::host_formation_log::HostFormationEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetFormation {
                    target: eid,
                    formation_id: ev.formation_id,
                    formation_offset: ev.formation_offset,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_demo_mine_cheer_events(
        &mut self,
        events: &[crate::game_logic::host_demo_mine_cheer_log::HostDemoMineCheerEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetDemoMineCheer {
                    target: eid,
                    demo_suicided_detonating: ev.demo_suicided_detonating,
                    has_mine_data: ev.has_mine_data,
                    cheer_timer: ev.cheer_timer,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_crush_vision_events(
        &mut self,
        events: &[crate::game_logic::host_crush_vision_log::HostCrushVisionEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetCrushVision {
                    target: eid,
                    crusher_level: ev.crusher_level,
                    crushable_level: ev.crushable_level,
                    vision_range: ev.vision_range,
                    shroud_clearing_range: ev.shroud_clearing_range,
                    front_crushed: ev.front_crushed,
                    back_crushed: ev.back_crushed,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_building_type_events(
        &mut self,
        events: &[crate::game_logic::host_building_type_log::HostBuildingTypeEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetBuildingType {
                    target: eid,
                    is_building: ev.is_building,
                    building_type_ordinal: ev.building_type_ordinal,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_identity_events(
        &mut self,
        events: &[crate::game_logic::host_identity_log::HostIdentityEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetIdentity {
                    target: eid,
                    name: ev.name.clone(),
                    team_color: ev.team_color,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_ground_height_events(
        &mut self,
        events: &[crate::game_logic::host_ground_height_log::HostGroundHeightEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetGroundHeight {
                    target: eid,
                    ground_height: ev.ground_height,
                    from_terrain: ev.from_terrain,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_model_mesh_events(
        &mut self,
        events: &[crate::game_logic::host_model_mesh_log::HostModelMeshEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetModelMesh {
                    target: eid,
                    model_key: ev.model_key.clone(),
                    mesh_scale: ev.mesh_scale,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_fow_events(
        &mut self,
        events: &[crate::game_logic::host_fow_log::HostFowEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetFow {
                    target: eid,
                    visibility_alpha: ev.visibility_alpha,
                    is_explored: ev.is_explored,
                    visibility_falloff: ev.visibility_falloff,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_kind_of_events(
        &mut self,
        events: &[crate::game_logic::host_kind_of_log::HostKindOfEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetKindOfBits {
                    target: eid,
                    kind_of_bits: ev.kind_of_bits,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_faerie_fire_events(
        &mut self,
        events: &[crate::game_logic::host_faerie_fire_log::HostFaerieFireEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetFaerieFire {
                    target: eid,
                    active: ev.active,
                    until_frame: ev.until_frame,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn writeback_faerie_fire_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            // Wave 755: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_faerie_fire_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let Some(obj) = logic.host_objects().get(&ObjectId(hid)) else {
                continue;
            };
            let host_active = obj.is_faerie_fire();
            if host_active == ent.faerie_fire
                && obj.faerie_fire_until_frame == ent.faerie_fire_until_frame
            {
                continue;
            }
            // Wave 945: faerie-fire writeback via host writeback authority.
            if !logic.apply_host_writeback_op(crate::game_logic::HostWritebackOp::FaerieFire {
                id: ObjectId(hid),
                active: ent.faerie_fire,
                until_frame: ent.faerie_fire_until_frame,
            }) {
                continue;
            }
            // Wave 676: GameWorld faerie-fire last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_faerie_fire_ready_log::record(oid);
        }
        updated
    }

    pub fn apply_host_repulsor_events(
        &mut self,
        events: &[crate::game_logic::host_repulsor_log::HostRepulsorEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetRepulsor {
                    target: eid,
                    active: ev.active,
                    until_frame: ev.until_frame,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn writeback_repulsor_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            // Wave 756: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_repulsor_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let Some(obj) = logic.host_objects().get(&ObjectId(hid)) else {
                continue;
            };
            let host_active = obj.status.repulsor;
            if host_active == ent.repulsor && obj.repulsor_until_frame == ent.repulsor_until_frame {
                continue;
            }
            // Wave 945: repulsor writeback via host writeback authority.
            if !logic.apply_host_writeback_op(crate::game_logic::HostWritebackOp::Repulsor {
                id: ObjectId(hid),
                active: ent.repulsor,
                until_frame: ent.repulsor_until_frame,
            }) {
                continue;
            }
            // Wave 661: GameWorld repulsor last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_repulsor_ready_log::record(oid);
        }
        updated
    }

    pub fn apply_host_disable_timers_events(
        &mut self,
        events: &[crate::game_logic::host_disable_timers_log::HostDisableTimersEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetDisableTimers {
                    target: eid,
                    emp_until_frame: ev.emp_until_frame,
                    hacked_until_frame: ev.hacked_until_frame,
                    paralyzed_until_frame: ev.paralyzed_until_frame,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn writeback_disable_timers_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 756: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_disable_timers_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            if obj.status.disabled_emp_until_frame == ent.disabled_emp_until_frame
                && obj.status.disabled_hacked_until_frame == ent.disabled_hacked_until_frame
                && obj.status.disabled_paralyzed_until_frame == ent.disabled_paralyzed_until_frame
            {
                continue;
            }
            obj.status.disabled_emp_until_frame = ent.disabled_emp_until_frame;
            obj.status.disabled_hacked_until_frame = ent.disabled_hacked_until_frame;
            obj.status.disabled_paralyzed_until_frame = ent.disabled_paralyzed_until_frame;
            // Wave 677: GameWorld disable-timers last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_disable_timers_ready_log::record(oid);
        }
        updated
    }
}
