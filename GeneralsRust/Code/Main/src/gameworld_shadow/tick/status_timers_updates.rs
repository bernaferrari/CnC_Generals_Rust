//! Status timers: enemy-near / prone / float / anim-steer / radius-decal / checkpoint / smart-bomb.

use super::status_timers::EntityTickControl;
use crate::gameworld_shadow::GameWorldShadow;
use gamelogic::world::entities::EntityId;

impl GameWorldShadow {
    /// Waves 781–787: nearby scans and per-entity update residuals.
    ///
    /// Enemy-near / checkpoint scan paths may skip the rest of this entity's
    /// timers this frame (parity with the original `continue`).
    pub(super) fn tick_status_updates(&mut self, eid: EntityId, frame: u32) -> EntityTickControl {
        let hid = self.entity_to_host.get(&eid.get()).copied();
        let Some(e) = self.world.world_mut().entity_mut(eid) else {
            return EntityTickControl::Next { changed: false };
        };
        let mut changed = false;
        // Wave 781: EnemyNearUpdate residual (MODELCONDITION_ENEMYNEAR).
        if e.enemy_near_active {
            let was = e.enemy_near;
            if e.enemy_near_scan_delay == 0 {
                let delay_time = e.enemy_near_scan_delay_time.max(1);
                let sx = e.transform.position.x;
                let sz = e.transform.position.z;
                let vision = e.enemy_near_vision_range.max(e.vision_range);
                let my_team = e.team_ordinal;
                // Drop entity borrow before scan (scan needs world()).
                {
                    #[allow(dropping_references)]
                    drop(e);
                }
                let present = self.scan_enemy_near_present(eid, sx, sz, vision, my_team);
                if let Some(e2) = self.world.world_mut().entity_mut(eid) {
                    e2.enemy_near_scan_delay = delay_time;
                    e2.enemy_near = present;
                    if present && !was {
                        e2.enemy_near_model = true;
                    } else if !present && was {
                        e2.enemy_near_model = false;
                    }
                }
                return EntityTickControl::SkipRest;
            } else {
                e.enemy_near_scan_delay = e.enemy_near_scan_delay.saturating_sub(1);
                changed = true;
            }
        }
        // Wave 782: ProneUpdate residual (infantry cower countdown).
        if e.prone_active && e.prone_frames > 0 {
            e.prone_frames -= 1;
            if e.prone_frames == 0 {
                e.prone_model = false;
                e.prone_no_attack = false;
                if let Some(bit) =
                    crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
                        "PRONE",
                    )
                {
                    e.model_condition_bits &= !(1u128 << bit);
                }
            } else {
                e.prone_model = true;
                e.prone_no_attack = true;
                if let Some(bit) =
                    crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
                        "PRONE",
                    )
                {
                    e.model_condition_bits |= 1u128 << bit;
                }
            }
            changed = true;
        }
        // Wave 783: FloatUpdate residual (boat sway / optional water snap).
        if e.float_update_active {
            use crate::game_logic::host_float_update::{
                FLOAT_PITCH_PHASE, FLOAT_SWAY_AMP, FLOAT_YAW_PHASE, leftover_water_surface_y,
                publish_sway,
            };
            let angle = frame as f32;
            e.float_yaw = (angle * FLOAT_YAW_PHASE).sin() * FLOAT_SWAY_AMP;
            e.float_pitch = (angle * FLOAT_PITCH_PHASE).sin() * FLOAT_SWAY_AMP;
            // C++ Enabled: snap object Z to isUnderwater waterZ (not lakebed).
            if e.float_update_enabled {
                if let Some(wy) =
                    leftover_water_surface_y(e.transform.position.x, e.transform.position.z)
                {
                    e.transform.position.y = wy;
                }
            }
            if let Some(hid) = hid {
                publish_sway(hid, e.float_yaw, e.float_pitch);
            }
            changed = true;
        }
        // Wave 784: AnimationSteeringUpdate residual (turn anim conditions).
        if e.anim_steer_active {
            use crate::game_logic::host_animation_steering::HostAnimSteerTurnAnim;
            use crate::game_logic::object::PhysicsTurningType;
            if frame >= e.anim_steer_next_transition_frame {
                let turning = match e.physics_turning_ordinal {
                    -1 => PhysicsTurningType::TurnNegative,
                    1 => PhysicsTurningType::TurnPositive,
                    _ => PhysicsTurningType::TurnNone,
                };
                let cur = match e.anim_steer_turn {
                    1 => HostAnimSteerTurnAnim::CenterToRight,
                    2 => HostAnimSteerTurnAnim::CenterToLeft,
                    3 => HostAnimSteerTurnAnim::LeftToCenter,
                    4 => HostAnimSteerTurnAnim::RightToCenter,
                    _ => HostAnimSteerTurnAnim::Invalid,
                };
                let mut next = cur;
                let mut changed_anim: Option<&'static str> = None;
                let tf = e.anim_steer_transition_frames.max(1);
                match cur {
                    HostAnimSteerTurnAnim::Invalid => {
                        if turning == PhysicsTurningType::TurnNegative {
                            next = HostAnimSteerTurnAnim::CenterToRight;
                            e.anim_steer_next_transition_frame = frame.saturating_add(tf);
                            changed_anim = Some("CENTER_TO_RIGHT");
                        } else if turning == PhysicsTurningType::TurnPositive {
                            next = HostAnimSteerTurnAnim::CenterToLeft;
                            e.anim_steer_next_transition_frame = frame.saturating_add(tf);
                            changed_anim = Some("CENTER_TO_LEFT");
                        }
                    }
                    HostAnimSteerTurnAnim::CenterToRight => {
                        if turning != PhysicsTurningType::TurnNegative {
                            next = HostAnimSteerTurnAnim::RightToCenter;
                            e.anim_steer_next_transition_frame = frame.saturating_add(tf);
                            changed_anim = Some("RIGHT_TO_CENTER");
                        }
                    }
                    HostAnimSteerTurnAnim::CenterToLeft => {
                        if turning != PhysicsTurningType::TurnPositive {
                            next = HostAnimSteerTurnAnim::LeftToCenter;
                            e.anim_steer_next_transition_frame = frame.saturating_add(tf);
                            changed_anim = Some("LEFT_TO_CENTER");
                        }
                    }
                    HostAnimSteerTurnAnim::LeftToCenter | HostAnimSteerTurnAnim::RightToCenter => {
                        if turning == PhysicsTurningType::TurnNone {
                            next = HostAnimSteerTurnAnim::Invalid;
                            e.anim_steer_next_transition_frame = frame;
                            e.anim_steer_has_condition = false;
                        }
                    }
                }
                e.anim_steer_turn = next as u8;
                if let Some(_) = changed_anim {
                    e.anim_steer_has_condition = true;
                }
                changed = true;
            }
        }
        // Wave 785: RadiusDecalUpdate residual (SW delivery decal throb/kill).
        if e.radius_decal_awake {
            if e.radius_decal_kill_when_idle && !e.attacking {
                e.radius_decal_empty = true;
                e.radius_decal_awake = false;
                e.radius_decal_kill_when_idle = false;
                e.radius_decal_opacity = 0.0;
                changed = true;
            } else if !e.radius_decal_empty {
                let period = e.radius_decal_throb_frames.max(1);
                let phase = frame.saturating_sub(e.radius_decal_birth_frame) % (period * 2);
                let t = if phase <= period {
                    phase as f32 / period as f32
                } else {
                    2.0 - (phase as f32 / period as f32)
                };
                e.radius_decal_opacity = e.radius_decal_opacity_min
                    + (e.radius_decal_opacity_max - e.radius_decal_opacity_min) * t;
                changed = true;
            }
        }
        // Wave 786: CheckpointUpdate residual (gate open/close + path radius).
        if e.checkpoint_active {
            use crate::game_logic::host_checkpoint_update::CHECKPOINT_RADIUS_STEP;
            if e.checkpoint_scan_delay == 0 {
                let delay_time = e.checkpoint_scan_delay_time.max(1);
                let sx = e.transform.position.x;
                let sz = e.transform.position.z;
                let vision = e.checkpoint_vision_range.max(e.vision_range);
                let my_team = e.team_ordinal;
                // Drop entity borrow before scan (scan needs world()).
                {
                    #[allow(dropping_references)]
                    drop(e);
                }
                let (enemy, ally) = self.scan_checkpoint_near(eid, sx, sz, vision, my_team);
                if let Some(e2) = self.world.world_mut().entity_mut(eid) {
                    e2.checkpoint_scan_delay = delay_time;
                    let change =
                        e2.checkpoint_enemy_near != enemy || e2.checkpoint_ally_near != ally;
                    e2.checkpoint_enemy_near = enemy;
                    e2.checkpoint_ally_near = ally;
                    let open = ally && !enemy;
                    if change {
                        e2.checkpoint_open = open;
                        e2.checkpoint_door_anim = if open { 1 } else { 2 };
                    }
                    if e2.checkpoint_open {
                        if e2.checkpoint_path_radius > 0.0 {
                            e2.checkpoint_path_radius =
                                (e2.checkpoint_path_radius - CHECKPOINT_RADIUS_STEP).max(0.0);
                        }
                    } else if e2.checkpoint_path_radius < e2.checkpoint_max_minor_radius {
                        e2.checkpoint_path_radius = (e2.checkpoint_path_radius
                            + CHECKPOINT_RADIUS_STEP)
                            .min(e2.checkpoint_max_minor_radius);
                    }
                }
                return EntityTickControl::SkipRest;
            } else {
                e.checkpoint_scan_delay = e.checkpoint_scan_delay.saturating_sub(1);
                if e.checkpoint_open {
                    if e.checkpoint_path_radius > 0.0 {
                        e.checkpoint_path_radius =
                            (e.checkpoint_path_radius - CHECKPOINT_RADIUS_STEP).max(0.0);
                    }
                } else if e.checkpoint_path_radius < e.checkpoint_max_minor_radius {
                    e.checkpoint_path_radius = (e.checkpoint_path_radius + CHECKPOINT_RADIUS_STEP)
                        .min(e.checkpoint_max_minor_radius);
                }
                changed = true;
            }
        }
        // Wave 787: SmartBombTargetHomingUpdate residual (course fudge).
        if e.smart_bomb_homing_active && e.smart_bomb_target_received {
            use crate::game_logic::host_smart_bomb_target_homing::SMART_BOMB_SIGNIFICANTLY_ABOVE_TERRAIN;
            let hat = (e.transform.position.y - e.ground_height).max(0.0);
            if hat >= SMART_BOMB_SIGNIFICANTLY_ABOVE_TERRAIN {
                let status = e.smart_bomb_course_scalar.clamp(0.0, 1.0);
                let target_c = 1.0 - status;
                e.transform.position.x =
                    e.smart_bomb_target_x * target_c + e.transform.position.x * status;
                e.transform.position.z =
                    e.smart_bomb_target_z * target_c + e.transform.position.z * status;
                // Y altitude unchanged.
                changed = true;
            }
        }
        EntityTickControl::Next { changed }
    }
}
