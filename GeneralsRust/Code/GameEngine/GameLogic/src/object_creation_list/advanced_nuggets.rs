// FILE: advanced_nuggets.rs - Advanced Object Creation Nuggets
// Author: Steven Johnson, December 2001 (C++)
// Rust Port: 2025
// Desc: Complex nugget types for special powers, weapons, and reinforcements
//
// Advanced Nugget Types:
// - DeliverPayloadNugget: Transport aircraft spawning with payload (airstrikes, paradrops)
// - FireWeaponNugget: Fire temporary weapon at target
// - AttackNugget: Make object attack a position
// - ApplyRandomForceNugget: Apply random physical forces

use super::nuggets::{ObjectCreationNugget, INVALID_ANGLE};
use super::{CreationContext, CreationResult};
use crate::common::*;
use crate::modules::{AIUpdateInterfaceExt, ContainModuleInterfaceExt, PhysicsBehaviorExt};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::update::DeliverPayloadData;
use crate::object::{Object, ObjectScriptStatusBit};
use crate::weapon::{WeaponLockType, WeaponSlotType};
use std::f32::consts::PI;
use std::sync::{Arc, RwLock};

fn set_special_power_creator(object: &Arc<RwLock<Object>>, creator_id: ObjectID) {
    let object_id = object
        .read()
        .ok()
        .map(|guard| guard.get_id())
        .unwrap_or(INVALID_ID);
    set_special_power_creator_id(object_id, creator_id);
}

fn set_special_power_creator_id(object_id: ObjectID, creator_id: ObjectID) {
    if object_id == INVALID_ID {
        return;
    }
    let _ = OBJECT_REGISTRY.with_object_mut(object_id, |guard| {
        guard.set_special_power_completion_creator(creator_id);
    });
}

/// Payload information for delivery
/// Matches C++ Payload struct (ObjectCreationList.cpp:551-555)
#[derive(Debug, Clone)]
pub struct Payload {
    pub payload_name: String,
    pub payload_count: Int,
}

/// DeliverPayloadNugget - spawns transport aircraft with payload
/// Matches C++ DeliverPayloadNugget (ObjectCreationList.cpp:225-572)
///
/// Used for:
/// - A-10 Thunderbolt strikes
/// - Carpet bombing runs
/// - Paratroop drops
/// - Napalm strikes
/// - Fuel Air Bomb delivery
#[derive(Debug, Clone)]
pub struct DeliverPayloadNugget {
    // Transport that carries payload
    pub transport_name: String,
    pub start_at_preferred_height: bool,
    pub start_at_max_speed: bool,

    // Formation parameters (for multiple transports)
    pub formation_size: UnsignedInt,
    pub formation_spacing: Real,
    pub convergence_factor: Real, // 0.0 = spread, 1.0 = converge to same point
    pub error_radius: Real,       // Random targeting error
    pub delay_delivery_frames_max: UnsignedInt,

    // Payload objects to deliver
    pub payload: Vec<Payload>,
    pub put_in_container_name: String,

    // AI delivery parameters
    pub data: DeliverPayloadData,
}

impl Default for DeliverPayloadNugget {
    fn default() -> Self {
        Self {
            transport_name: String::new(),
            start_at_preferred_height: true,
            start_at_max_speed: false,
            formation_size: 1,
            formation_spacing: 25.0,
            convergence_factor: 0.0,
            error_radius: 0.0,
            delay_delivery_frames_max: 0,
            payload: Vec::new(),
            put_in_container_name: String::new(),
            data: DeliverPayloadData::default(),
        }
    }
}

impl DeliverPayloadNugget {
    /// Calculate formation offset vectors (CCW and CW perpendicular to approach)
    /// Matches C++ lines 271-298
    fn calculate_formation_vectors(
        primary: &Coord3D,
        secondary: &Coord3D,
    ) -> (Real, Real, Real, Real) {
        let dx = primary.x - secondary.x;
        let dy = primary.y - secondary.y;

        let length = (dx * dx + dy * dy).sqrt();
        if length < 0.001 {
            return (0.0, 0.0, 0.0, 0.0);
        }

        let dx_norm = dx / length;
        let dy_norm = dy / length;

        // Rotate 90 degrees CCW
        let radians = 90.0 * PI / 180.0;
        let s = radians.sin();
        let c = radians.cos();
        let ccw_x = dx_norm * c + dy_norm * -s + dx_norm;
        let ccw_y = dx_norm * s + dy_norm * c + dy_norm;

        // Rotate 90 degrees CW
        let s = (-radians).sin();
        let c = (-radians).cos();
        let cw_x = dx_norm * c + dy_norm * -s + dx_norm;
        let cw_y = dx_norm * s + dy_norm * c + dy_norm;

        (ccw_x, ccw_y, cw_x, cw_y)
    }

    /// Calculate offset for formation member
    /// Matches C++ lines 303-319
    fn calculate_formation_offset(
        formation_index: Int,
        formation_size: Int,
        formation_spacing: Real,
        ccw_x: Real,
        ccw_y: Real,
        cw_x: Real,
        cw_y: Real,
    ) -> Coord3D {
        if formation_size <= 1 {
            return Coord3D::new(0.0, 0.0, 0.0);
        }

        let offset_multiplier = ((formation_index + 1) / 2) as Real * formation_spacing;

        if formation_index % 2 == 1 {
            // Odd - use CCW
            Coord3D::new(ccw_x * offset_multiplier, ccw_y * offset_multiplier, 0.0)
        } else {
            // Even - use CW
            Coord3D::new(cw_x * offset_multiplier, cw_y * offset_multiplier, 0.0)
        }
    }
}

impl ObjectCreationNugget for DeliverPayloadNugget {
    fn create_with_angle(
        &self,
        ctx: &CreationContext<'_>,
        primary_obj: Option<&Object>,
        primary: &Coord3D,
        secondary: &Coord3D,
        _angle: Real,
        lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        self.create_with_owner_flag(ctx, primary_obj, primary, secondary, true, lifetime_frames)
    }

    fn create_with_owner_flag(
        &self,
        ctx: &CreationContext<'_>,
        primary_obj: Option<&Object>,
        primary: &Coord3D,
        secondary: &Coord3D,
        create_owner: Bool,
        _lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        let Some(primary_object) = primary_obj else {
            return None;
        };

        // Get owner team
        let Some(player) = primary_object.get_controlling_player() else {
            return None;
        };
        let Some(owner_arc) = player.read().ok().and_then(|p| p.get_default_team()) else {
            return None;
        };
        let Ok(owner) = owner_arc.read() else {
            return None;
        };

        // Calculate formation vectors if multiple transports
        let (ccw_x, ccw_y, cw_x, cw_y) = if self.formation_size > 1 {
            Self::calculate_formation_vectors(primary, secondary)
        } else {
            (0.0, 0.0, 0.0, 0.0)
        };

        let mut first_transport: Option<Arc<RwLock<Object>>> = None;

        // Create each transport in formation
        for formation_index in 0..self.formation_size as Int {
            // Calculate formation offset
            let offset = Self::calculate_formation_offset(
                formation_index,
                self.formation_size as Int,
                self.formation_spacing,
                ccw_x,
                ccw_y,
                cw_x,
                cw_y,
            );

            // Calculate positions
            let mut start_pos = *primary;
            start_pos.x += offset.x;
            start_pos.y += offset.y;

            let mut move_to_pos = *secondary;
            move_to_pos.x += offset.x;
            move_to_pos.y += offset.y;

            let mut target_pos = *secondary;
            target_pos.x += offset.x * (1.0 - self.convergence_factor);
            target_pos.y += offset.y * (1.0 - self.convergence_factor);

            // Apply random error to target (except first transport)
            if self.error_radius > 1.0 && formation_index > 0 {
                let random_radius = ctx.game_logic.random_value_real(0.0, self.error_radius);
                let random_angle = ctx.game_logic.random_value_real(0.0, PI * 2.0);
                target_pos.x += random_radius * random_angle.cos();
                target_pos.y += random_radius * random_angle.sin();
            }

            // Calculate orientation and adjust start position
            let orient = (move_to_pos.y - start_pos.y).atan2(move_to_pos.x - start_pos.x);
            if self.data.dist_to_target > 0.0 {
                const SLOP: Real = 1.5;
                start_pos.x -= orient.cos() * self.data.dist_to_target * SLOP;
                start_pos.y -= orient.sin() * self.data.dist_to_target * SLOP;
            }

            // Create or use existing transport
            let transport = if create_owner {
                // Create new transport
                let Some(transport_template) =
                    ctx.thing_factory.find_template(&self.transport_name)
                else {
                    return None;
                };

                let Ok(transport) = ctx.thing_factory.new_object(transport_template, &*owner)
                else {
                    return None;
                };

                if first_transport.is_none() {
                    first_transport = Some(Arc::clone(&transport));
                }

                // Set position, orientation, and producer
                if let Ok(mut transport_write) = transport.write() {
                    let _ = transport_write.set_position(&start_pos);
                    let _ = transport_write.set_orientation(orient);
                    transport_write.set_producer(Some(primary_object));
                    transport_write
                        .set_script_status(ObjectScriptStatusBit::ScriptTargetable, true);
                }

                // Apply random delivery delay
                if self.delay_delivery_frames_max > 0 {
                    let delay = ctx
                        .game_logic
                        .random_value(0, self.delay_delivery_frames_max as Int)
                        .max(0) as UnsignedInt;
                    if delay > 0 {
                        if let Ok(mut transport_write) = transport.write() {
                            transport_write.set_disabled_until(
                                DisabledType::DisabledDefault,
                                ctx.game_logic.get_frame().saturating_add(delay),
                            );
                        }
                    }
                }

                transport
            } else {
                // Use primary object as transport
                let Some(transport) = OBJECT_REGISTRY.get_object(primary_object.get_id()) else {
                    return None;
                };
                transport
            };

            // Notify special power tracking
            let transport_id = transport
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(INVALID_ID);
            if formation_index == 0 {
                set_special_power_creator_id(transport_id, primary_object.get_id());
            } else {
                set_special_power_creator_id(transport_id, INVALID_ID);
            }

            // Apply starting velocity if configured
            if self.start_at_max_speed && create_owner {
                if let Ok(transport_read) = transport.read() {
                    let physics = transport_read.get_physics();
                    let ai = transport_read.get_ai_update_interface();
                    let body = transport_read.get_body_module();
                    let (dir_x, dir_y) = transport_read.get_unit_direction_vector_2d();
                    drop(transport_read);

                    if let (Some(physics), Some(ai), Some(body)) = (physics, ai, body) {
                        if let Ok(body_guard) = body.lock() {
                            if let Some(locomotor) = ai.get_cur_locomotor() {
                                if let Ok(locomotor_guard) = locomotor.lock() {
                                    let max_speed = locomotor_guard.get_max_speed_for_condition(
                                        match body_guard.get_damage_state() {
                                            crate::common::BodyDamageType::Pristine => {
                                                crate::locomotor::BodyDamageType::Pristine
                                            }
                                            crate::common::BodyDamageType::Damaged => {
                                                crate::locomotor::BodyDamageType::Damaged
                                            }
                                            crate::common::BodyDamageType::ReallyDamaged => {
                                                crate::locomotor::BodyDamageType::ReallyDamaged
                                            }
                                            crate::common::BodyDamageType::Rubble => {
                                                crate::locomotor::BodyDamageType::Rubble
                                            }
                                        },
                                    );
                                    let mut starting_force = Vec3D::new(dir_x, dir_y, 0.0);
                                    let factor = max_speed * physics.get_mass();
                                    starting_force *= factor;
                                    physics.apply_motive_force(&starting_force);
                                }
                            }
                        }
                    }
                }
            }

            // Set up DeliverPayloadAIUpdate
            let mut has_deliver_payload_ai = false;
            if let Ok(transport_read) = transport.read() {
                if let Some(ai) = transport_read.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        if let Some(deliver_ai) = ai_guard.get_deliver_payload_ai_update_interface()
                        {
                            has_deliver_payload_ai = true;
                            let mut delivery_data = self.data.clone();
                            if formation_index > 0 {
                                delivery_data.delivery_decal_radius = 0.0;
                            }
                            deliver_ai.deliver_payload(&move_to_pos, &target_pos, &delivery_data);
                        }
                    }
                }
            }

            if !has_deliver_payload_ai {
                log::warn!(
                    "DeliverPayloadNugget transport '{}' missing DeliverPayloadAIUpdate",
                    self.transport_name
                );
                continue;
            }

            if self.start_at_preferred_height && create_owner {
                let preferred_height = transport
                    .read()
                    .ok()
                    .and_then(|transport_read| transport_read.get_ai_update_interface())
                    .and_then(|ai| ai.get_preferred_height());
                if let Some(height) = preferred_height {
                    start_pos.z = ctx
                        .terrain_logic
                        .get_ground_height(start_pos.x, start_pos.y)
                        + height;
                    if let Ok(mut transport_write) = transport.write() {
                        let _ = transport_write.set_position(&start_pos);
                    }
                }
            }

            // Create and load payload objects into transport
            let put_in_container_tmpl = if !self.put_in_container_name.is_empty() {
                ctx.thing_factory.find_template(&self.put_in_container_name)
            } else {
                None
            };

            for payload_def in &self.payload {
                let Some(payload_tmpl) = ctx.thing_factory.find_template(&payload_def.payload_name)
                else {
                    return None;
                };

                for payload_index in 0..payload_def.payload_count {
                    let Ok(payload_obj) = ctx
                        .thing_factory
                        .new_object(Arc::clone(&payload_tmpl), &*owner)
                    else {
                        continue;
                    };

                    // Set position and producer
                    if let Ok(mut payload_write) = payload_obj.write() {
                        let _ = payload_write.set_position(&start_pos);
                        if let Ok(transport_read) = transport.read() {
                            payload_write.set_producer(Some(&*transport_read));
                        }
                    }

                    if formation_index == 0 && payload_index == 0 {
                        set_special_power_creator(&payload_obj, primary_object.get_id());
                    } else {
                        set_special_power_creator(&payload_obj, INVALID_ID);
                    }

                    // Optionally put payload in container first
                    let final_payload = if let Some(ref container_tmpl) = put_in_container_tmpl {
                        if let Ok(container) = ctx
                            .thing_factory
                            .new_object(Arc::clone(container_tmpl), &*owner)
                        {
                            if let Ok(mut container_write) = container.write() {
                                let _ = container_write.set_position(&start_pos);
                                if let Ok(transport_read) = transport.read() {
                                    container_write.set_producer(Some(&*transport_read));
                                }
                            }

                            if formation_index == 0 && payload_index == 0 {
                                set_special_power_creator(&container, primary_object.get_id());
                            } else {
                                set_special_power_creator(&container, INVALID_ID);
                            }

                            // Check if payload can be contained
                            let can_contain = if let Ok(container_read) = container.read() {
                                if let Some(contain) = container_read.get_contain() {
                                    if let Ok(payload_read) = payload_obj.read() {
                                        contain.is_valid_container_for(&*payload_read, true)
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                }
                            } else {
                                false
                            };

                            if can_contain {
                                // Add to container
                                if let Ok(container_read) = container.read() {
                                    if let Some(contain) = container_read.get_contain() {
                                        if let Ok(payload_read) = payload_obj.read() {
                                            contain.add_to_contain(&*payload_read);
                                        }
                                    }
                                }
                                container
                            } else {
                                payload_obj
                            }
                        } else {
                            payload_obj
                        }
                    } else {
                        payload_obj
                    };

                    // Add to transport
                    if let Ok(transport_read) = transport.read() {
                        if let Some(transport_contain) = transport_read.get_contain() {
                            if let Ok(final_payload_read) = final_payload.read() {
                                if transport_contain
                                    .is_valid_container_for(&*final_payload_read, true)
                                {
                                    // Extension trait expects &Object
                                    transport_contain.add_to_contain(&*final_payload_read);
                                }
                            }
                        }
                    }
                }
            }
        }

        first_transport
    }
}

/// FireWeaponNugget - fires a temporary weapon
/// Matches C++ FireWeaponNugget (ObjectCreationList.cpp:105-148)
///
/// Used for effects that need to fire weapons without permanent objects
#[derive(Debug, Clone)]
pub struct FireWeaponNugget {
    pub weapon: Option<String>, // Weapon template name
}

impl Default for FireWeaponNugget {
    fn default() -> Self {
        Self { weapon: None }
    }
}

impl ObjectCreationNugget for FireWeaponNugget {
    fn create_with_angle(
        &self,
        _ctx: &CreationContext<'_>,
        primary_obj: Option<&Object>,
        _primary: &Coord3D,
        secondary: &Coord3D,
        _angle: Real,
        _lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        let Some(_primary_object) = primary_obj else {
            return None;
        };

        if let Some(ref weapon_name) = self.weapon {
            if crate::helpers::TheWeaponStore::get().is_some() {
                let _ = crate::helpers::TheWeaponStore::create_and_fire_temp_weapon(
                    weapon_name,
                    _primary_object,
                    secondary,
                );
            }
        }

        None // FireWeapon doesn't create objects, returns None
    }
}

/// AttackNugget - makes object attack a position
/// Matches C++ AttackNugget (ObjectCreationList.cpp:151-221)
///
/// Used for scripted attacks and special power targeting
#[derive(Debug, Clone)]
pub struct AttackNugget {
    pub number_of_shots: Int,
    pub weapon_slot: WeaponSlotType,
    pub delivery_decal_template: RadiusDecalTemplate,
    pub delivery_decal_radius: Real,
}

impl Default for AttackNugget {
    fn default() -> Self {
        Self {
            number_of_shots: 1,
            weapon_slot: WeaponSlotType::Primary,
            delivery_decal_template: RadiusDecalTemplate::default(),
            delivery_decal_radius: 0.0,
        }
    }
}

impl ObjectCreationNugget for AttackNugget {
    fn create_with_angle(
        &self,
        _ctx: &CreationContext<'_>,
        primary_obj: Option<&Object>,
        _primary: &Coord3D,
        secondary: &Coord3D,
        _angle: Real,
        _lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        let Some(primary_object) = primary_obj else {
            return None;
        };

        // Lock weapon and attack. The C++ nugget explicitly locks the current weapon slot
        // so that subsequent AI logic uses the desired weapon.
        let _ = OBJECT_REGISTRY.with_object_mut(primary_object.get_id(), |primary_write| {
            primary_write.set_weapon_lock(self.weapon_slot, WeaponLockType::LockedTemporarily);
        });

        if let Some(ai_arc) = primary_object.get_ai_update_interface() {
            ai_arc.ai_attack_position(
                secondary,
                self.number_of_shots,
                CommandSourceType::FromScript,
            );
        }

        // Set up delivery decal if specified
        if !self.delivery_decal_template.texture_name.is_empty() && self.delivery_decal_radius > 0.0
        {
            for behavior in primary_object.get_behavior_modules() {
                let Ok(mut behavior) = behavior.lock() else {
                    continue;
                };
                let Some(radius_update) = behavior.get_radius_decal_update_interface() else {
                    continue;
                };
                radius_update.create_radius_decal(
                    &self.delivery_decal_template,
                    self.delivery_decal_radius,
                    secondary,
                );
                radius_update.kill_when_no_longer_attacking(true);
                break;
            }
        }

        None // Attack doesn't create objects, returns None
    }
}

/// ApplyRandomForceNugget - applies random forces to an object
/// Matches C++ ApplyRandomForceNugget (ObjectCreationList.cpp:595-670)
///
/// Used for creating visual variety in explosions and impacts
#[derive(Debug, Clone)]
pub struct ApplyRandomForceNugget {
    pub spin_rate: Real,
    pub min_mag: Real,
    pub max_mag: Real,
    pub min_pitch: Real,
    pub max_pitch: Real,
}

impl Default for ApplyRandomForceNugget {
    fn default() -> Self {
        Self {
            spin_rate: 0.0,
            min_mag: 0.0,
            max_mag: 0.0,
            min_pitch: 0.0,
            max_pitch: 0.0,
        }
    }
}

impl ObjectCreationNugget for ApplyRandomForceNugget {
    fn create_with_angle(
        &self,
        _ctx: &CreationContext<'_>,
        _primary_obj: Option<&Object>,
        _primary: &Coord3D,
        _secondary: &Coord3D,
        _angle: Real,
        _lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        None // This nugget operates on existing objects in create_with_objects
    }

    fn create_with_objects(
        &self,
        ctx: &CreationContext<'_>,
        primary: &Object,
        _secondary: Option<&Object>,
        _lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        if let Some(physics) = primary.get_physics() {
            // Calculate random force
            let angle = ctx.game_logic.random_value_real(0.0, 2.0 * PI);
            let pitch = ctx
                .game_logic
                .random_value_real(self.min_pitch, self.max_pitch);
            let mag = ctx.game_logic.random_value_real(self.min_mag, self.max_mag);

            let horiz = mag * pitch.cos();
            let vert = mag * pitch.sin();

            let force = Coord3D::new(horiz * angle.cos(), horiz * angle.sin(), vert);

            physics.apply_force(&force);

            // Apply random spin
            let yaw = ctx
                .game_logic
                .random_value_real(-self.spin_rate, self.spin_rate);
            let roll = ctx
                .game_logic
                .random_value_real(-self.spin_rate, self.spin_rate);
            let pitch = ctx
                .game_logic
                .random_value_real(-self.spin_rate, self.spin_rate);

            physics.set_yaw_rate(yaw);
            physics.set_roll_rate(roll);
            physics.set_pitch_rate(pitch);
        }

        None // Doesn't create objects
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deliver_payload_default() {
        let nugget = DeliverPayloadNugget::default();
        assert_eq!(nugget.formation_size, 1);
        assert_eq!(nugget.start_at_preferred_height, true);
        assert_eq!(nugget.formation_spacing, 25.0);
    }

    #[test]
    fn test_formation_vectors() {
        let primary = Coord3D::new(100.0, 100.0, 0.0);
        let secondary = Coord3D::new(0.0, 0.0, 0.0);

        let (ccw_x, ccw_y, cw_x, cw_y) =
            DeliverPayloadNugget::calculate_formation_vectors(&primary, &secondary);

        // Vectors should be perpendicular to approach vector
        assert!(ccw_x.abs() > 0.0 || ccw_y.abs() > 0.0);
        assert!(cw_x.abs() > 0.0 || cw_y.abs() > 0.0);
    }

    #[test]
    fn test_fire_weapon_nugget() {
        let nugget = FireWeaponNugget {
            weapon: Some("TestWeapon".to_string()),
        };
        assert!(nugget.weapon.is_some());
    }

    #[test]
    fn test_attack_nugget_default() {
        let nugget = AttackNugget::default();
        assert_eq!(nugget.number_of_shots, 1);
        assert_eq!(nugget.weapon_slot, WeaponSlotType::Primary);
    }

    #[test]
    fn test_apply_force_nugget() {
        let nugget = ApplyRandomForceNugget {
            spin_rate: 1.0,
            min_mag: 10.0,
            max_mag: 20.0,
            min_pitch: 0.0,
            max_pitch: PI / 4.0,
        };
        assert_eq!(nugget.spin_rate, 1.0);
        assert_eq!(nugget.min_mag, 10.0);
    }
}
