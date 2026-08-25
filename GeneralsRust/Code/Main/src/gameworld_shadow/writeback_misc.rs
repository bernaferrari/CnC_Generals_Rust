//! Remaining writeback_* for identity, movement, stealth, and combat residuals.

use super::*;
use crate::game_logic::{GameLogic, ObjectId, Team};
use gamelogic::world::entities::{EntityId, EntityProductionItem, TemplateRef, Transform};
use gamelogic::world::{GameWorld, PlayerId, WorldMutation, WorldSnapshot};
use std::collections::{HashMap, HashSet};

impl GameWorldShadow {
    pub fn writeback_ground_height_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_ground_height_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let Some(obj) = logic.host_objects().get(&ObjectId(hid)) else {
                continue;
            };
            let changed = (obj.ground_height - ent.ground_height).abs() > f32::EPSILON
                || obj.ground_height_from_terrain != ent.ground_height_from_terrain;
            if !changed {
                continue;
            }
            // Wave 945: ground-height writeback via host writeback authority.
            if !logic.apply_host_writeback_op(crate::game_logic::HostWritebackOp::GroundHeight {
                id: ObjectId(hid),
                height: ent.ground_height,
                from_terrain: ent.ground_height_from_terrain,
            }) {
                continue;
            }
            // Wave 656: GameWorld ground-height last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_ground_height_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_identity_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_identity_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let color_changed = obj
                .team_color
                .iter()
                .zip(ent.team_color.iter())
                .any(|(a, b)| (*a - *b).abs() > f32::EPSILON);
            if obj.name == ent.display_name && !color_changed {
                continue;
            }
            obj.name = ent.display_name.clone();
            obj.team_color = ent.team_color;
            // Wave 660: GameWorld identity last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_identity_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_building_type_to_host(&self, logic: &mut GameLogic) -> usize {
        use crate::game_logic::{BuildingData, BuildingType as B};
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_building_type_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let host_is = obj.building_data.is_some();
            let host_ord = obj
                .building_data
                .as_ref()
                .map(|bd| Self::host_building_type_ordinal(bd.building_type))
                .unwrap_or(255);
            if host_is == ent.is_building && host_ord == ent.building_type_ordinal {
                continue;
            }
            if !ent.is_building || ent.building_type_ordinal == 255 {
                if obj.building_data.is_some() {
                    // Do not destroy building_data payload on flag-only clear; leave host ownership.
                }
            } else {
                let bt = match ent.building_type_ordinal {
                    0 => B::CommandCenter,
                    1 => B::Barracks,
                    2 => B::WarFactory,
                    3 => B::Airfield,
                    4 => B::RepairPad,
                    5 => B::HealPad,
                    6 => B::SupplyCenter,
                    7 => B::PowerPlant,
                    8 => B::DefenseTurret,
                    9 => B::SupplyDropZone,
                    10 => B::Palace,
                    11 => B::Propaganda,
                    12 => B::Bunker,
                    _ => B::CommandCenter,
                };
                if let Some(bd) = obj.building_data.as_mut() {
                    if bd.building_type != bt {
                        bd.building_type = bt;
                        // Wave 675: GameWorld building-type last-write residual —
                        // host applies presentation bookkeeping from ready log.
                        ready.push(ObjectId(hid));
                        updated += 1;
                    }
                } else {
                    obj.building_data = Some(BuildingData::new(bt));
                    // Wave 675: GameWorld building-type last-write residual —
                    // host applies presentation bookkeeping from ready log.
                    ready.push(ObjectId(hid));
                    updated += 1;
                }
            }
        }
        for oid in ready {
            crate::game_logic::host_building_type_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_crush_vision_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_crush_vision_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let changed = obj.crusher_level != ent.crusher_level
                || obj.crushable_level != ent.crushable_level
                || (obj.vision_range - ent.vision_range).abs() > f32::EPSILON
                || (obj.shroud_clearing_range - ent.shroud_clearing_range).abs() > f32::EPSILON
                || obj.front_crushed != ent.front_crushed
                || obj.back_crushed != ent.back_crushed;
            if !changed {
                continue;
            }
            obj.crusher_level = ent.crusher_level;
            obj.crushable_level = ent.crushable_level;
            obj.vision_range = ent.vision_range;
            obj.shroud_clearing_range = ent.shroud_clearing_range;
            obj.front_crushed = ent.front_crushed;
            obj.back_crushed = ent.back_crushed;
            // Wave 664: GameWorld crush-vision last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_crush_vision_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_demo_mine_cheer_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_demo_mine_cheer_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let host_has_mine = obj.mine_data.is_some();
            let changed = obj.demo_suicided_detonating != ent.demo_suicided_detonating
                || host_has_mine != ent.has_mine_data
                || (obj.cheer_timer - ent.cheer_timer).abs() > f32::EPSILON;
            if !changed {
                continue;
            }
            obj.demo_suicided_detonating = ent.demo_suicided_detonating;
            obj.cheer_timer = ent.cheer_timer;
            // has_mine_data is a present-flag mirror only; do not invent/destroy HostMineData here.
            // Flag-only writeback: if entity says no mine and host has mine_data left to status, leave payload.
            // Cheer/demo flags are authoritative from GameWorld last-writer residual.
            let _ = host_has_mine; // presence is logged host→entity; entity→host keeps payload ownership on Main.
            // Wave 665: GameWorld demo-mine-cheer last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_demo_mine_cheer_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_model_condition_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<(ObjectId, u128, u128)> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 757: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_model_condition_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            if obj.model_condition_bits == ent.model_condition_bits {
                continue;
            }
            let prev = obj.model_condition_bits;
            // Direct assign — avoid host_model_condition_log re-entry during writeback.
            // Keep `obj.model_condition_bits = ent.model_condition_bits` for Wave 486
            // residual source markers (door visual path).
            obj.model_condition_bits = ent.model_condition_bits;
            obj.power_plant_rods_extended = ent.power_plant_rods_extended;
            obj.power_plant_rods_done_frame = ent.power_plant_rods_done_frame;
            // Wave 633: GameWorld model-condition last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push((ObjectId(hid), prev, ent.model_condition_bits));
            updated += 1;
        }
        for (oid, prev, next) in ready {
            crate::game_logic::host_model_condition_ready_log::record(oid, prev, next);
        }
        updated
    }

    pub fn writeback_movement_to_host(&self, logic: &mut GameLogic) -> usize {
        use glam::Vec3;
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 757: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_movement_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let host_v = [
                obj.movement.velocity.x,
                obj.movement.velocity.y,
                obj.movement.velocity.z,
            ];
            let host_idx = obj.movement.current_path_index.min(u16::MAX as usize) as u16;
            let host_len = obj.movement.path.len().min(u16::MAX as usize) as u16;
            let vel_changed = host_v
                .iter()
                .zip(ent.velocity.iter())
                .any(|(a, b)| (*a - *b).abs() > f32::EPSILON);
            let path_changed = if ent.path_waypoints.is_empty() {
                ent.path_len == 0 && !obj.movement.path.is_empty()
            } else {
                obj.movement.path.len() != ent.path_waypoints.len()
                    || obj
                        .movement
                        .path
                        .iter()
                        .zip(ent.path_waypoints.iter())
                        .any(|(p, e)| {
                            (p.x - e[0]).abs() > f32::EPSILON
                                || (p.y - e[1]).abs() > f32::EPSILON
                                || (p.z - e[2]).abs() > f32::EPSILON
                        })
            };
            let flags_changed = obj.waiting_for_path != ent.waiting_for_path
                || obj.locomotor_surfaces != ent.locomotor_surfaces
                || obj.is_attack_path != ent.is_attack_path
                || obj.is_blocked_and_stuck != ent.is_blocked_and_stuck
                || obj.is_braking != ent.is_braking
                || obj.is_safe_path != ent.is_safe_path
                || obj.queue_for_path_frames != ent.queue_for_path_frames
                || obj.path_timestamp != ent.path_timestamp
                || (obj.cur_max_blocked_speed - ent.cur_max_blocked_speed).abs() > f32::EPSILON
                || obj.num_frames_blocked != ent.num_frames_blocked
                || obj.is_blocked != ent.is_blocked
                || obj.move_away_from.map(|id| id.0) != ent.move_away_from_id
                || obj.requested_victim_id.map(|id| id.0) != ent.requested_victim_id;
            let changed = vel_changed
                || (obj.movement.max_speed - ent.move_max_speed).abs() > f32::EPSILON
                || host_idx != ent.path_index
                || host_len != ent.path_len
                || path_changed
                || flags_changed;
            if !changed {
                continue;
            }
            obj.movement.velocity = Vec3::new(ent.velocity[0], ent.velocity[1], ent.velocity[2]);
            obj.movement.max_speed = ent.move_max_speed;
            obj.movement.current_path_index = ent.path_index as usize;
            if !ent.path_waypoints.is_empty() {
                obj.movement.path = ent
                    .path_waypoints
                    .iter()
                    .map(|p| Vec3::new(p[0], p[1], p[2]))
                    .collect();
            } else if ent.path_len == 0 {
                obj.movement.path.clear();
            }
            obj.waiting_for_path = ent.waiting_for_path;
            obj.locomotor_surfaces = ent.locomotor_surfaces;
            obj.is_attack_path = ent.is_attack_path;
            obj.is_blocked_and_stuck = ent.is_blocked_and_stuck;
            obj.is_braking = ent.is_braking;
            obj.is_safe_path = ent.is_safe_path;
            obj.queue_for_path_frames = ent.queue_for_path_frames;
            obj.path_timestamp = ent.path_timestamp;
            obj.cur_max_blocked_speed = ent.cur_max_blocked_speed;
            obj.num_frames_blocked = ent.num_frames_blocked;
            obj.is_blocked = ent.is_blocked;
            obj.move_away_from = ent.move_away_from_id.map(ObjectId);
            obj.requested_victim_id = ent.requested_victim_id.map(ObjectId);
            // Wave 637: GameWorld movement last-write residual —
            // host applies path/presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_movement_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_physics_motive_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 757: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_physics_motive_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let host_dest = obj.move_away_destination.map(|p| [p.x, p.y, p.z]);
            let changed = obj.motive_frames_remaining != ent.motive_frames_remaining
                || (obj.physics_mass - ent.physics_mass).abs() > f32::EPSILON
                || (obj.physics_accel.x - ent.physics_accel[0]).abs() > f32::EPSILON
                || (obj.physics_accel.y - ent.physics_accel[1]).abs() > f32::EPSILON
                || (obj.physics_accel.z - ent.physics_accel[2]).abs() > f32::EPSILON
                || (obj.forward_friction - ent.forward_friction).abs() > f32::EPSILON
                || (obj.lateral_friction - ent.lateral_friction).abs() > f32::EPSILON
                || (obj.z_friction - ent.z_friction).abs() > f32::EPSILON
                || obj.can_path_through_units != ent.can_path_through_units
                || obj.ignore_collisions_until_frame != ent.ignore_collisions_until_frame
                || obj.is_panicking != ent.is_panicking
                || obj.move_away_frames != ent.move_away_frames
                || (obj.aerodynamic_friction - ent.aerodynamic_friction).abs() > f32::EPSILON
                || (obj.extra_friction - ent.extra_friction).abs() > f32::EPSILON
                || obj.apply_friction_2d_when_airborne != ent.apply_friction_2d_when_airborne
                || (obj.center_of_mass_offset - ent.center_of_mass_offset).abs() > f32::EPSILON
                || (obj.pitch_roll_yaw_factor - ent.pitch_roll_yaw_factor).abs() > f32::EPSILON
                || host_dest != ent.move_away_destination
                || obj.request_other_move_away.map(|id| id.0) != ent.request_other_move_away_id
                || obj.immune_to_falling_damage != ent.immune_to_falling_damage
                || obj.physics_current_overlap.map(|id| id.0) != ent.physics_current_overlap_id
                || obj.physics_previous_overlap.map(|id| id.0) != ent.physics_previous_overlap_id;
            if !changed {
                continue;
            }
            obj.motive_frames_remaining = ent.motive_frames_remaining;
            obj.physics_mass = ent.physics_mass;
            obj.physics_accel = glam::Vec3::new(
                ent.physics_accel[0],
                ent.physics_accel[1],
                ent.physics_accel[2],
            );
            obj.forward_friction = ent.forward_friction;
            obj.lateral_friction = ent.lateral_friction;
            obj.z_friction = ent.z_friction;
            obj.can_path_through_units = ent.can_path_through_units;
            obj.ignore_collisions_until_frame = ent.ignore_collisions_until_frame;
            obj.is_panicking = ent.is_panicking;
            obj.move_away_frames = ent.move_away_frames;
            obj.aerodynamic_friction = ent.aerodynamic_friction;
            obj.extra_friction = ent.extra_friction;
            obj.apply_friction_2d_when_airborne = ent.apply_friction_2d_when_airborne;
            obj.center_of_mass_offset = ent.center_of_mass_offset;
            obj.pitch_roll_yaw_factor = ent.pitch_roll_yaw_factor;
            obj.move_away_destination = ent
                .move_away_destination
                .map(|p| glam::Vec3::new(p[0], p[1], p[2]));
            obj.request_other_move_away = ent.request_other_move_away_id.map(ObjectId);
            obj.immune_to_falling_damage = ent.immune_to_falling_damage;
            obj.physics_current_overlap = ent.physics_current_overlap_id.map(ObjectId);
            obj.physics_previous_overlap = ent.physics_previous_overlap_id.map(ObjectId);
            // Wave 649: GameWorld physics-motive last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_physics_motive_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_locomotor_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 757: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_locomotor_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let changed = obj.is_approach_path != ent.is_approach_path
                || obj.on_invalid_movement_terrain != ent.on_invalid_movement_terrain
                || obj.was_airborne_last_frame != ent.was_airborne_last_frame
                || obj.can_move_backward != ent.can_move_backward
                || obj.moving_backwards != ent.moving_backwards
                || obj.no_slow_down_as_approaching_dest != ent.no_slow_down_as_approaching_dest
                || (obj.turn_pivot_offset - ent.turn_pivot_offset).abs() > f32::EPSILON
                || (obj.wander_width_factor - ent.wander_width_factor).abs() > f32::EPSILON
                || obj.loco_apply_2d_friction_airborne != ent.loco_apply_2d_friction_airborne
                || obj.allow_motive_force_while_airborne != ent.allow_motive_force_while_airborne
                || (obj.loco_extra_2d_friction - ent.loco_extra_2d_friction).abs() > f32::EPSILON
                || (obj.loco_preferred_height - ent.loco_preferred_height).abs() > f32::EPSILON
                || (obj.loco_preferred_height_damping - ent.loco_preferred_height_damping).abs()
                    > f32::EPSILON
                || obj.loco_appearance.to_ordinal() != ent.loco_appearance_ordinal
                || obj.loco_behavior_z.to_ordinal() != ent.loco_behavior_z_ordinal
                || (obj.min_turn_speed - ent.min_turn_speed).abs() > f32::EPSILON
                || obj.physics_turning.to_ordinal() != ent.physics_turning_ordinal;
            if !changed {
                continue;
            }
            obj.is_approach_path = ent.is_approach_path;
            obj.on_invalid_movement_terrain = ent.on_invalid_movement_terrain;
            obj.was_airborne_last_frame = ent.was_airborne_last_frame;
            obj.can_move_backward = ent.can_move_backward;
            obj.moving_backwards = ent.moving_backwards;
            obj.no_slow_down_as_approaching_dest = ent.no_slow_down_as_approaching_dest;
            obj.turn_pivot_offset = ent.turn_pivot_offset;
            obj.wander_width_factor = ent.wander_width_factor;
            obj.loco_apply_2d_friction_airborne = ent.loco_apply_2d_friction_airborne;
            obj.allow_motive_force_while_airborne = ent.allow_motive_force_while_airborne;
            obj.loco_extra_2d_friction = ent.loco_extra_2d_friction;
            obj.loco_preferred_height = ent.loco_preferred_height;
            obj.loco_preferred_height_damping = ent.loco_preferred_height_damping;
            obj.loco_appearance =
                crate::game_logic::LocomotorAppearance::from_ordinal(ent.loco_appearance_ordinal);
            obj.loco_behavior_z =
                crate::game_logic::LocomotorBehaviorZ::from_ordinal(ent.loco_behavior_z_ordinal);
            obj.min_turn_speed = ent.min_turn_speed;
            obj.physics_turning =
                crate::game_logic::PhysicsTurningType::from_ordinal(ent.physics_turning_ordinal);
            // Wave 646: GameWorld locomotor last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_locomotor_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_bounce_land_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_bounce_land_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let changed = obj.kill_when_resting_on_ground != ent.kill_when_resting_on_ground
                || obj.bounce_land_events != ent.bounce_land_events
                || (obj.last_bounce_fall_dy - ent.last_bounce_fall_dy).abs() > f32::EPSILON
                || obj.bounce_sound_name != ent.bounce_sound_name
                || (obj.last_bounce_volume - ent.last_bounce_volume).abs() > f32::EPSILON
                || obj.bounce_audio_pending != ent.bounce_audio_pending
                || obj.allow_collide_force != ent.allow_collide_force
                || obj.last_collidee.map(|id| id.0) != ent.last_collidee_id
                || obj.ignore_collisions_with.map(|id| id.0) != ent.ignore_collisions_with_id;
            if !changed {
                continue;
            }
            obj.kill_when_resting_on_ground = ent.kill_when_resting_on_ground;
            obj.bounce_land_events = ent.bounce_land_events;
            obj.last_bounce_fall_dy = ent.last_bounce_fall_dy;
            obj.bounce_sound_name = ent.bounce_sound_name.clone();
            obj.last_bounce_volume = ent.last_bounce_volume;
            obj.bounce_audio_pending = ent.bounce_audio_pending;
            obj.allow_collide_force = ent.allow_collide_force;
            obj.last_collidee = ent.last_collidee_id.map(ObjectId);
            obj.ignore_collisions_with = ent.ignore_collisions_with_id.map(ObjectId);
            // Wave 650: GameWorld bounce-land last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_bounce_land_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_weapon_stats_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 757: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_weapon_stats_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let mut changed = false;
            if let Some(w) = obj.weapon.as_mut() {
                if (w.damage - ent.weapon_damage).abs() > f32::EPSILON
                    || (w.range - ent.weapon_range).abs() > f32::EPSILON
                    || (w.min_range - ent.weapon_min_range).abs() > f32::EPSILON
                    || (w.reload_time - ent.weapon_reload_time).abs() > f32::EPSILON
                    || (w.last_fire_time - ent.weapon_last_fire_time).abs() > f32::EPSILON
                    || w.ammo.unwrap_or(u32::MAX) != ent.weapon_ammo
                    || w.can_target_air != ent.weapon_can_target_air
                    || w.can_target_ground != ent.weapon_can_target_ground
                    || (w.projectile_speed - ent.weapon_projectile_speed).abs() > f32::EPSILON
                {
                    w.damage = ent.weapon_damage;
                    w.range = ent.weapon_range;
                    w.min_range = ent.weapon_min_range;
                    w.reload_time = ent.weapon_reload_time;
                    w.last_fire_time = ent.weapon_last_fire_time;
                    w.ammo = if ent.weapon_ammo == u32::MAX {
                        None
                    } else {
                        Some(ent.weapon_ammo)
                    };
                    w.can_target_air = ent.weapon_can_target_air;
                    w.can_target_ground = ent.weapon_can_target_ground;
                    w.projectile_speed = ent.weapon_projectile_speed;
                    changed = true;
                }
            }
            if let Some(w) = obj.secondary_weapon.as_mut() {
                if (w.damage - ent.secondary_weapon_damage).abs() > f32::EPSILON
                    || (w.range - ent.secondary_weapon_range).abs() > f32::EPSILON
                {
                    w.damage = ent.secondary_weapon_damage;
                    w.range = ent.secondary_weapon_range;
                    changed = true;
                }
            }
            if obj.leech_range_active_primary != ent.leech_range_active_primary
                || obj.leech_range_active_secondary != ent.leech_range_active_secondary
            {
                obj.leech_range_active_primary = ent.leech_range_active_primary;
                obj.leech_range_active_secondary = ent.leech_range_active_secondary;
                changed = true;
            }
            if self.writeback_extra_weapon_slots(eid, obj) {
                changed = true;
            }
            if changed {
                // Wave 635: GameWorld weapon-stats last-write residual —
                // host applies presentation bookkeeping from ready log.
                ready.push(ObjectId(hid));
                updated += 1;
            }
        }
        for oid in ready {
            crate::game_logic::host_weapon_stats_ready_log::record(oid);
        }
        updated
    }

    fn writeback_extra_weapon_slots(
        &self,
        eid: EntityId,
        obj: &mut crate::game_logic::Object,
    ) -> bool {
        use gamelogic::world::{
            WEAPON_SLOT_MINE_CLEAR, WEAPON_SLOT_SECONDARY, WEAPON_SLOT_TERTIARY,
        };
        let mut changed = false;
        let apply = |host: &mut Option<crate::game_logic::Weapon>,
                     facts: Option<gamelogic::world::WeaponSlotFacts>| {
            let Some(f) = facts else {
                return false;
            };
            if !f.present {
                return false;
            }
            let Some(w) = host.as_mut() else {
                return false;
            };
            let mut slot_changed = false;
            if w.clip_size != f.clip_size {
                w.clip_size = f.clip_size;
                slot_changed = true;
            }
            let ammo = if f.ammo == u32::MAX {
                None
            } else {
                Some(f.ammo)
            };
            if w.ammo != ammo {
                w.ammo = ammo;
                slot_changed = true;
            }
            if (w.reload_time - f.reload_time).abs() > f32::EPSILON {
                w.reload_time = f.reload_time;
                slot_changed = true;
            }
            if (w.last_fire_time - f.last_fire_time).abs() > f32::EPSILON {
                w.last_fire_time = f.last_fire_time;
                slot_changed = true;
            }
            slot_changed
        };
        changed |= apply(
            &mut obj.secondary_weapon,
            self.world.weapon_slots().slot(eid, WEAPON_SLOT_SECONDARY),
        );
        changed |= apply(
            &mut obj.tertiary_weapon,
            self.world.weapon_slots().slot(eid, WEAPON_SLOT_TERTIARY),
        );
        changed |= apply(
            &mut obj.mine_clearing_primary_weapon,
            self.world.weapon_slots().slot(eid, WEAPON_SLOT_MINE_CLEAR),
        );
        if let Some(f) = self.world.weapon_slots().slot(eid, WEAPON_SLOT_TERTIARY) {
            if f.present && f.lock_type != 0 {
                obj.weapon_lock_slot = WEAPON_SLOT_TERTIARY;
            }
        }
        changed
    }

    pub fn writeback_vision_camo_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_vision_camo_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let changed = obj.vision_spied_mask != ent.vision_spied_mask
                || (obj.camo_friendly_opacity - ent.camo_friendly_opacity).abs() > f32::EPSILON
                || obj.camo_stealth_look != ent.camo_stealth_look;
            if !changed {
                continue;
            }
            obj.vision_spied_mask = ent.vision_spied_mask;
            obj.camo_friendly_opacity = ent.camo_friendly_opacity;
            obj.camo_stealth_look = ent.camo_stealth_look;
            // Wave 654: GameWorld vision-camo last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_vision_camo_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_disguise_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_disguise_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let host_tpl = obj.disguise_as_template.clone().unwrap_or_default();
            let host_team = obj
                .disguise_as_team
                .map(|t| match t {
                    Team::USA => 0u8,
                    Team::China => 1,
                    Team::GLA => 2,
                    Team::Neutral => 3,
                })
                .unwrap_or(255);
            if host_tpl == ent.disguise_as_template && host_team == ent.disguise_as_team_ordinal {
                continue;
            }
            obj.disguise_as_template = if ent.disguise_as_template.is_empty() {
                None
            } else {
                Some(ent.disguise_as_template.clone())
            };
            obj.disguise_as_team = match ent.disguise_as_team_ordinal {
                0 => Some(Team::USA),
                1 => Some(Team::China),
                2 => Some(Team::GLA),
                3 => Some(Team::Neutral),
                _ => None,
            };
            // Wave 653: GameWorld disguise last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_disguise_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_overlord_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_overlord_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let host_cap = match obj.overlord_bunker_capacity {
                Some(n) => n.min(u16::MAX as usize - 1) as u16,
                None => u16::MAX,
            };
            let changed = obj.has_overlord_gattling_addon != ent.has_overlord_gattling_addon
                || obj.has_overlord_propaganda_addon != ent.has_overlord_propaganda_addon
                || host_cap != ent.overlord_bunker_capacity
                || obj.is_helix_transport != ent.is_helix_transport;
            if !changed {
                continue;
            }
            obj.has_overlord_gattling_addon = ent.has_overlord_gattling_addon;
            obj.has_overlord_propaganda_addon = ent.has_overlord_propaganda_addon;
            obj.is_helix_transport = ent.is_helix_transport;
            obj.overlord_bunker_capacity = if ent.overlord_bunker_capacity == u16::MAX {
                None
            } else {
                Some(ent.overlord_bunker_capacity as usize)
            };
            // Wave 666: GameWorld overlord last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_overlord_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_stealth_flags_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 757: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_stealth_flags_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let changed = obj.innate_stealth != ent.innate_stealth
                || obj.stealth_breaks_on_attack != ent.stealth_breaks_on_attack
                || obj.stealth_breaks_on_move != ent.stealth_breaks_on_move
                || obj.is_tunnel_network != ent.is_tunnel_network
                || obj.passengers_allowed_to_fire != ent.passengers_allowed_to_fire;
            if !changed {
                continue;
            }
            obj.innate_stealth = ent.innate_stealth;
            obj.stealth_breaks_on_attack = ent.stealth_breaks_on_attack;
            obj.stealth_breaks_on_move = ent.stealth_breaks_on_move;
            obj.is_tunnel_network = ent.is_tunnel_network;
            obj.passengers_allowed_to_fire = ent.passengers_allowed_to_fire;
            // Wave 652: GameWorld stealth-flags last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_stealth_flags_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_stealth_delay_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_stealth_delay_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let changed = obj.stealth_allowed_frame != ent.stealth_allowed_frame
                || obj.stealth_delay_pending != ent.stealth_delay_pending
                || obj.stealth_delay_frames != ent.stealth_delay_frames
                || obj.stealth_breaks_on_damage != ent.stealth_breaks_on_damage
                || obj.detection_expires_frame != ent.detection_expires_frame
                || (obj.camo_opacity_pulse_phase - ent.camo_opacity_pulse_phase).abs()
                    > f32::EPSILON
                || (obj.camo_heat_vision_opacity - ent.camo_heat_vision_opacity).abs()
                    > f32::EPSILON
                || obj.camo_net_sub_object_shown != ent.camo_net_sub_object_shown
                || obj.camo_net_sub_object_observer_visible
                    != ent.camo_net_sub_object_observer_visible;
            if !changed {
                continue;
            }
            obj.stealth_allowed_frame = ent.stealth_allowed_frame;
            obj.stealth_delay_pending = ent.stealth_delay_pending;
            obj.stealth_delay_frames = ent.stealth_delay_frames;
            obj.stealth_breaks_on_damage = ent.stealth_breaks_on_damage;
            obj.detection_expires_frame = ent.detection_expires_frame;
            obj.camo_opacity_pulse_phase = ent.camo_opacity_pulse_phase;
            obj.camo_heat_vision_opacity = ent.camo_heat_vision_opacity;
            obj.camo_net_sub_object_shown = ent.camo_net_sub_object_shown;
            obj.camo_net_sub_object_observer_visible = ent.camo_net_sub_object_observer_visible;
            // Wave 651: GameWorld stealth-delay last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_stealth_delay_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_hive_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_hive_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let changed = obj.hive_slave_count != ent.hive_slave_count
                || (obj.hive_slave_hp - ent.hive_slave_hp).abs() > 1e-4;
            if !changed {
                continue;
            }
            obj.hive_slave_count = ent.hive_slave_count;
            obj.hive_slave_hp = ent.hive_slave_hp.max(0.0);
            for i in 0..3 {
                obj.hive_slaves[i].alive = ent.hive_slaves_alive[i];
                obj.hive_slaves[i].hp = ent.hive_slaves_hp[i].max(0.0);
            }
            // Wave 667: GameWorld hive last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_hive_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_hijacker_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_hijacker_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let host_vehicle = obj.hijack_vehicle_id.map(|id| id.0).unwrap_or(0);
            let host_eject = obj.hijacker_eject_pos.map(|p| [p.x, p.y, p.z]);
            let changed = host_vehicle != ent.hijack_vehicle_host
                || obj.hijacker_in_vehicle != ent.hijacker_in_vehicle
                || obj.hijacker_update_active != ent.hijacker_update_active
                || obj.hijacker_was_airborne != ent.hijacker_was_airborne
                || host_eject != ent.hijacker_eject_pos
                || obj.hive_slave_respawn_frame != ent.hive_slave_respawn_frame
                || obj.next_detection_scan_frame != ent.next_detection_scan_frame;
            if !changed {
                continue;
            }
            obj.hijack_vehicle_id = if ent.hijack_vehicle_host == 0 {
                None
            } else {
                Some(ObjectId(ent.hijack_vehicle_host))
            };
            obj.hijacker_in_vehicle = ent.hijacker_in_vehicle;
            obj.hijacker_update_active = ent.hijacker_update_active;
            obj.hijacker_was_airborne = ent.hijacker_was_airborne;
            obj.hijacker_eject_pos = ent
                .hijacker_eject_pos
                .map(|p| glam::Vec3::new(p[0], p[1], p[2]));
            obj.hive_slave_respawn_frame = ent.hive_slave_respawn_frame;
            obj.next_detection_scan_frame = ent.next_detection_scan_frame;
            // Wave 647: GameWorld hijacker last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_hijacker_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_contain_capacity_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 759: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_contain_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let host_garrison = obj
                .building_data
                .as_ref()
                .map(|bd| bd.max_garrison.min(u16::MAX as usize) as u16)
                .unwrap_or(0);
            let changed =
                obj.max_transport != ent.max_transport || host_garrison != ent.max_garrison;
            if !changed {
                continue;
            }
            obj.max_transport = ent.max_transport;
            if ent.max_garrison > 0 || obj.building_data.is_some() {
                if let Some(bd) = obj.building_data.as_mut() {
                    bd.max_garrison = ent.max_garrison as usize;
                } else if ent.max_garrison > 0 {
                    let mut bd = crate::game_logic::buildings::BuildingData::new(
                        crate::game_logic::buildings::BuildingType::Bunker,
                    );
                    bd.max_garrison = ent.max_garrison as usize;
                    obj.building_data = Some(bd);
                }
            }
            updated += 1;
        }
        updated
    }

    pub fn writeback_overcharge_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_overcharge_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let Some(obj) = logic.host_objects().get(&ObjectId(hid)) else {
                continue;
            };
            if obj.overcharge_enabled == ent.overcharge_enabled {
                continue;
            }
            // Wave 945: overcharge writeback via host writeback authority.
            if !logic.apply_host_writeback_op(crate::game_logic::HostWritebackOp::Overcharge {
                id: ObjectId(hid),
                enabled: ent.overcharge_enabled,
            }) {
                continue;
            }
            // Wave 668: GameWorld overcharge last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_overcharge_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_weapon_set_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 757: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_weapon_set_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            if obj.weapon_set_player_upgrade == ent.weapon_set_player_upgrade
                && obj.armed_riders_upgrade_weapon_set == ent.armed_riders_upgrade_weapon_set
            {
                continue;
            }
            obj.weapon_set_player_upgrade = ent.weapon_set_player_upgrade;
            obj.armed_riders_upgrade_weapon_set = ent.armed_riders_upgrade_weapon_set;
            // Wave 642: GameWorld weapon-set last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_weapon_set_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_ai_attitude_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_ai_attitude_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let Some(obj) = logic.host_objects().get(&ObjectId(hid)) else {
                continue;
            };
            if obj.ai_attitude == ent.ai_attitude {
                continue;
            }
            // Wave 945: AI-attitude writeback via host writeback authority.
            if !logic.apply_host_writeback_op(crate::game_logic::HostWritebackOp::AiAttitude {
                id: ObjectId(hid),
                attitude: ent.ai_attitude,
            }) {
                continue;
            }
            // Wave 659: GameWorld AI-attitude last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_ai_attitude_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_guard_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_guard_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let Some(obj) = logic.host_objects().get(&ObjectId(hid)) else {
                continue;
            };
            let host_pos = obj.guard_position.map(|p| [p.x, p.y, p.z]);
            let host_tgt = obj.guard_target.map(|id| id.0).unwrap_or(0);
            let changed = host_pos != ent.guard_position
                || host_tgt != ent.guard_target_host
                || (obj.guard_radius - ent.guard_radius).abs() > f32::EPSILON;
            if !changed {
                continue;
            }
            // Wave 945: guard writeback via host writeback authority.
            if !logic.apply_host_writeback_op(crate::game_logic::HostWritebackOp::Guard {
                id: ObjectId(hid),
                position: ent
                    .guard_position
                    .map(|p| glam::Vec3::new(p[0], p[1], p[2])),
                target: if ent.guard_target_host == 0 {
                    None
                } else {
                    Some(ObjectId(ent.guard_target_host))
                },
                radius: ent.guard_radius,
            }) {
                continue;
            }
            // Wave 669: GameWorld guard last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_guard_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_continuous_fire_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 755: under coupled tick, host continuous-fire log is mid-frame
            // authority until apply drains it. Do not stomp host from a stale
            // entity while pending host clear/set is still unapplied.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_continuous_fire_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let host_consec = obj.continuous_fire_consecutive.min(u16::MAX as u32) as u16;
            let changed = obj.continuous_fire_level != ent.continuous_fire_level
                || host_consec != ent.continuous_fire_consecutive
                || obj.continuous_fire_coast_until_frame != ent.continuous_fire_coast_until_frame
                || obj.frame_to_force_reload != ent.frame_to_force_reload;
            if !changed {
                continue;
            }
            obj.continuous_fire_level = ent.continuous_fire_level;
            obj.continuous_fire_consecutive = ent.continuous_fire_consecutive as u32;
            obj.continuous_fire_coast_until_frame = ent.continuous_fire_coast_until_frame;
            obj.frame_to_force_reload = ent.frame_to_force_reload;
            // Wave 670: GameWorld continuous-fire last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_continuous_fire_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_combat_attack_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 757: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_combat_attack_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let changed = obj.pre_attack_target.map(|id| id.0).unwrap_or(0)
                != ent.pre_attack_target_host
                || (obj.pre_attack_ready_at - ent.pre_attack_ready_at).abs() > f32::EPSILON
                || obj.consecutive_shots_at_target != ent.consecutive_shots_at_target
                || obj.max_shots_to_fire != ent.max_shots_to_fire
                || obj.attack_substate.to_ordinal() != ent.attack_substate_ordinal
                || obj.approach_timestamp != ent.approach_timestamp
                || obj.continuous_fire_victim != ent.continuous_fire_victim
                || obj.maintain_pos_valid != ent.maintain_pos_valid
                || obj.maintain_pos.map(|p| [p.x, p.y, p.z]) != ent.maintain_pos
                || obj.temporary_move_frames != ent.temporary_move_frames
                || (obj.group_speed_factor - ent.group_speed_factor).abs() > f32::EPSILON;
            if !changed {
                continue;
            }
            obj.pre_attack_target = if ent.pre_attack_target_host == 0 {
                None
            } else {
                Some(ObjectId(ent.pre_attack_target_host))
            };
            obj.pre_attack_ready_at = ent.pre_attack_ready_at;
            obj.consecutive_shots_at_target = ent.consecutive_shots_at_target;
            obj.max_shots_to_fire = ent.max_shots_to_fire;
            obj.attack_substate =
                crate::game_logic::AttackSubState::from_ordinal(ent.attack_substate_ordinal);
            obj.approach_timestamp = ent.approach_timestamp;
            obj.continuous_fire_victim = ent.continuous_fire_victim;
            obj.maintain_pos_valid = ent.maintain_pos_valid;
            obj.maintain_pos = ent.maintain_pos.map(|p| glam::Vec3::new(p[0], p[1], p[2]));
            obj.temporary_move_frames = ent.temporary_move_frames;
            obj.group_speed_factor = ent.group_speed_factor;
            // Wave 643: GameWorld combat-attack last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_combat_attack_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_fire_intent_to_host(&self, logic: &mut GameLogic) -> usize {
        if !gameworld_ai_attack_authority_enabled() {
            return 0;
        }
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_fire_intent_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let changed = obj.last_fire_victim_host != ent.last_fire_victim_host
                || obj.last_fire_slot != ent.last_fire_slot
                || (obj.last_fire_damage - ent.last_fire_damage).abs() > f32::EPSILON
                || (obj.last_fire_range - ent.last_fire_range).abs() > f32::EPSILON
                || (obj.last_fire_sim_time - ent.last_fire_sim_time).abs() > f32::EPSILON
                || obj.last_fire_frame != ent.last_fire_frame
                || obj.fire_intent_count != ent.fire_intent_count;
            if !changed {
                continue;
            }
            obj.last_fire_victim_host = ent.last_fire_victim_host;
            obj.last_fire_slot = ent.last_fire_slot;
            obj.last_fire_damage = ent.last_fire_damage;
            obj.last_fire_range = ent.last_fire_range;
            obj.last_fire_sim_time = ent.last_fire_sim_time;
            obj.last_fire_frame = ent.last_fire_frame;
            obj.fire_intent_count = ent.fire_intent_count;
            // Wave 640: GameWorld fire-intent last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_fire_intent_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_detector_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_detector_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let Some(obj) = logic.host_objects().get(&ObjectId(hid)) else {
                continue;
            };
            let changed = obj.is_detector != ent.is_detector
                || (obj.detection_range - ent.detection_range).abs() > 1e-4
                || obj.detection_rate_frames != ent.detection_rate_frames;
            if !changed {
                continue;
            }
            // Wave 945: detector writeback via host writeback authority.
            if !logic.apply_host_writeback_op(crate::game_logic::HostWritebackOp::Detector {
                id: ObjectId(hid),
                is_detector: ent.is_detector,
                range: ent.detection_range,
                rate_frames: ent.detection_rate_frames,
            }) {
                continue;
            }
            // Wave 671: GameWorld detector last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_detector_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_target_location_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_target_location_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let Some(obj) = logic.host_objects().get(&ObjectId(hid)) else {
                continue;
            };
            let host_loc = obj.target_location.map(|p| [p.x, p.y, p.z]);
            let ent_loc = ent.target_location;
            let same = match (host_loc, ent_loc) {
                (None, None) => true,
                (Some(a), Some(b)) => {
                    (a[0] - b[0]).abs() <= 1e-4
                        && (a[1] - b[1]).abs() <= 1e-4
                        && (a[2] - b[2]).abs() <= 1e-4
                }
                _ => false,
            };
            if same {
                continue;
            }
            // Wave 945: target-location writeback via host writeback authority.
            if !logic.apply_host_writeback_op(crate::game_logic::HostWritebackOp::TargetLocation {
                id: ObjectId(hid),
                location: ent_loc.map(|p| glam::Vec3::new(p[0], p[1], p[2])),
            }) {
                continue;
            }
            // Wave 672: GameWorld target-location last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_target_location_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_turret_to_host(&self, logic: &mut GameLogic) -> usize {
        use crate::game_logic::object::TurretSubState;
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_turret_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let host_tgt = obj.turret_target_id.map(|id| id.0).unwrap_or(0);
            let changed = (obj.turret_angle_deg - ent.turret_angle_deg).abs() > f32::EPSILON
                || (obj.turret_pitch_deg - ent.turret_pitch_deg).abs() > f32::EPSILON
                || obj.turret_holding != ent.turret_holding
                || obj.turret_idle_scanning != ent.turret_idle_scanning
                || (obj.turret_turn_rate_rad - ent.turret_turn_rate_rad).abs() > f32::EPSILON
                || obj.turret_recenter_frames != ent.turret_recenter_frames
                || obj.turret_hold_until_frame != ent.turret_hold_until_frame
                || obj.turret_idle_recentering != ent.turret_idle_recentering
                || obj.turret_enabled != ent.turret_enabled
                || obj.turret_rotating != ent.turret_rotating
                || (obj.turret_natural_angle_deg - ent.turret_natural_angle_deg).abs()
                    > f32::EPSILON
                || (obj.turret_natural_pitch_deg - ent.turret_natural_pitch_deg).abs()
                    > f32::EPSILON
                || host_tgt != ent.turret_target_host
                || obj.turret_force_attacking != ent.turret_force_attacking
                || obj.turret_mood_target != ent.turret_mood_target
                || obj.turret_idle_scan_next_frame != ent.turret_idle_scan_next_frame
                || (obj.turret_idle_scan_desired_angle_deg
                    - ent.turret_idle_scan_desired_angle_deg)
                    .abs()
                    > f32::EPSILON
                || obj.turret_idle_scan_index != ent.turret_idle_scan_index
                || obj.turret_substate.ordinal() != ent.turret_substate;
            if !changed {
                continue;
            }
            obj.turret_angle_deg = ent.turret_angle_deg;
            obj.turret_pitch_deg = ent.turret_pitch_deg;
            obj.turret_holding = ent.turret_holding;
            obj.turret_idle_scanning = ent.turret_idle_scanning;
            obj.turret_turn_rate_rad = ent.turret_turn_rate_rad;
            obj.turret_recenter_frames = ent.turret_recenter_frames;
            obj.turret_hold_until_frame = ent.turret_hold_until_frame;
            obj.turret_idle_recentering = ent.turret_idle_recentering;
            obj.turret_enabled = ent.turret_enabled;
            obj.turret_rotating = ent.turret_rotating;
            obj.turret_natural_angle_deg = ent.turret_natural_angle_deg;
            obj.turret_natural_pitch_deg = ent.turret_natural_pitch_deg;
            obj.turret_target_id = if ent.turret_target_host == 0 {
                None
            } else {
                Some(ObjectId(ent.turret_target_host))
            };
            obj.turret_force_attacking = ent.turret_force_attacking;
            obj.turret_mood_target = ent.turret_mood_target;
            obj.turret_idle_scan_next_frame = ent.turret_idle_scan_next_frame;
            obj.turret_idle_scan_desired_angle_deg = ent.turret_idle_scan_desired_angle_deg;
            obj.turret_idle_scan_index = ent.turret_idle_scan_index;
            obj.turret_substate = TurretSubState::from_ordinal(ent.turret_substate);
            // Wave 673: GameWorld turret last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_turret_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_entity_power_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_entity_power_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let Some(obj) = logic.host_objects().get(&ObjectId(hid)) else {
                continue;
            };
            if obj.power_provided == ent.power_provided && obj.power_consumed == ent.power_consumed
            {
                continue;
            }
            // Wave 945: entity-power writeback via host writeback authority.
            if !logic.apply_host_writeback_op(crate::game_logic::HostWritebackOp::EntityPower {
                id: ObjectId(hid),
                provided: ent.power_provided,
                consumed: ent.power_consumed,
            }) {
                continue;
            }
            // Wave 674: GameWorld entity-power last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_entity_power_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_weapon_slot_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_weapon_slot_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let Some(obj) = logic.host_objects().get(&ObjectId(hid)) else {
                continue;
            };
            if obj.active_weapon_slot == ent.active_weapon_slot {
                continue;
            }
            // Wave 945: weapon-slot writeback via host writeback authority.
            if !logic.apply_host_writeback_op(crate::game_logic::HostWritebackOp::WeaponSlot {
                id: ObjectId(hid),
                slot: ent.active_weapon_slot,
            }) {
                continue;
            }
            // Wave 657: GameWorld weapon-slot last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_weapon_slot_ready_log::record(oid);
        }
        updated
    }

    /// Write shadow weapon-bonus pack back onto host Object residual flags.
    pub fn writeback_weapon_bonus_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 755: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_weapon_bonus_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let changed = obj.weapon_bonus_enthusiastic != ent.weapon_bonus_enthusiastic
                || obj.weapon_bonus_subliminal != ent.weapon_bonus_subliminal
                || obj.weapon_bonus_horde != ent.weapon_bonus_horde
                || obj.weapon_bonus_nationalism != ent.weapon_bonus_nationalism
                || obj.weapon_bonus_fanaticism != ent.weapon_bonus_fanaticism
                || obj.last_horde_refresh_frame != ent.last_horde_refresh_frame
                || obj.horde_next_wake_frame != ent.horde_next_wake_frame
                || obj.horde_wake_initialized != ent.horde_wake_initialized
                || obj.weapon_bonus_frenzy != ent.weapon_bonus_frenzy
                || obj.weapon_bonus_frenzy_level != ent.weapon_bonus_frenzy_level
                || obj.weapon_bonus_battle_plan_bombardment
                    != ent.weapon_bonus_battle_plan_bombardment
                || obj.weapon_bonus_battle_plan_hold_the_line
                    != ent.weapon_bonus_battle_plan_hold_the_line
                || obj.weapon_bonus_battle_plan_search_and_destroy
                    != ent.weapon_bonus_battle_plan_search_and_destroy
                || obj.weapon_bonus_frenzy_until_frame != ent.weapon_bonus_frenzy_until_frame
                || (obj.battle_plan_sight_scalar_applied - ent.battle_plan_sight_scalar_applied)
                    .abs()
                    > f32::EPSILON;
            if !changed {
                continue;
            }
            obj.weapon_bonus_enthusiastic = ent.weapon_bonus_enthusiastic;
            obj.weapon_bonus_subliminal = ent.weapon_bonus_subliminal;
            obj.weapon_bonus_horde = ent.weapon_bonus_horde;
            obj.weapon_bonus_nationalism = ent.weapon_bonus_nationalism;
            obj.weapon_bonus_fanaticism = ent.weapon_bonus_fanaticism;
            obj.last_horde_refresh_frame = ent.last_horde_refresh_frame;
            obj.horde_next_wake_frame = ent.horde_next_wake_frame;
            obj.horde_wake_initialized = ent.horde_wake_initialized;

            obj.weapon_bonus_frenzy = ent.weapon_bonus_frenzy;
            obj.weapon_bonus_frenzy_level = ent.weapon_bonus_frenzy_level;
            obj.weapon_bonus_battle_plan_bombardment = ent.weapon_bonus_battle_plan_bombardment;
            obj.weapon_bonus_battle_plan_hold_the_line = ent.weapon_bonus_battle_plan_hold_the_line;
            obj.weapon_bonus_battle_plan_search_and_destroy =
                ent.weapon_bonus_battle_plan_search_and_destroy;
            obj.weapon_bonus_frenzy_until_frame = ent.weapon_bonus_frenzy_until_frame;
            obj.battle_plan_sight_scalar_applied = ent.battle_plan_sight_scalar_applied;
            // Wave 658: GameWorld weapon-bonus last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_weapon_bonus_ready_log::record(oid);
        }
        updated
    }

    /// Write shadow Entity::experience_points back onto host Object::experience.current.
    pub fn writeback_experience_to_host(&self, logic: &mut GameLogic) -> usize {
        use crate::game_logic::VeterancyLevel as V;
        let mut updated = 0usize;
        let mut level_ups: Vec<(ObjectId, u8, u8, f32)> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            // Wave 757: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_experience_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let Some(obj) = logic.host_objects().get(&ObjectId(hid)) else {
                continue;
            };
            let pts = ent.experience_points.max(0.0);
            let want_level = match ent.veterancy_ordinal {
                1 => V::Veteran,
                2 => V::Elite,
                3 => V::Heroic,
                _ => V::Rookie,
            };
            let pts_changed = (obj.experience.current - pts).abs() > 0.000_1;
            let level_changed = obj.experience.level != want_level;
            if !pts_changed && !level_changed {
                continue;
            }
            let prev_ord = if level_changed {
                Self::host_veterancy_ordinal(obj.experience.level)
            } else {
                0
            };
            // Wave 944: experience writeback via host writeback authority.
            if !logic.apply_host_writeback_op(crate::game_logic::HostWritebackOp::Experience {
                id: ObjectId(hid),
                points: pts_changed.then_some(pts),
                level: level_changed.then_some(want_level),
            }) {
                continue;
            }
            if level_changed {
                let new_ord = ent.veterancy_ordinal.min(3);
                // Wave 622: GameWorld sole XP/level last-write residual —
                // host applies combat bonuses from ready log.
                if crate::gameworld_shadow::gameworld_damage_authority_live() && new_ord > prev_ord
                {
                    level_ups.push((ObjectId(hid), prev_ord, new_ord, pts));
                }
            }
            updated += 1;
        }
        for (oid, prev, next, pts) in level_ups {
            crate::game_logic::host_veterancy_ready_log::record(oid, prev, next, pts);
        }
        updated
    }
}
