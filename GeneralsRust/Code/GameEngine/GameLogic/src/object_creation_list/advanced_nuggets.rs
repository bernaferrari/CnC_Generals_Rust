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

use super::nuggets::{ObjectCreationNugget, calc_random_force};
use super::{CreationContext, CreationResult};
use crate::common::*;
use crate::modules::{AIUpdateInterfaceExt, ContainModuleInterfaceExt, PhysicsBehaviorExt};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::update::DeliverPayloadData;
use crate::object::{Object, ObjectScriptStatusBit};
use crate::player::CMD_FROM_AI;
use crate::weapon::{WeaponLockType, WeaponSlotType};
use std::any::Any;
use std::f32::consts::PI;
use std::sync::{Arc, RwLock};

/// Wave 445: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

fn set_special_power_creator(object: &Arc<RwLock<Object>>, creator_id: ObjectID) {
    let object_id = object
        .read()
        .ok()
        .map(|guard| guard.get_id())
        .unwrap_or(INVALID_ID);
    set_special_power_creator_id(object_id, creator_id);
}

fn set_special_power_creator_id(object_id: ObjectID, creator_id: ObjectID) {
    // Wave 445: empty dual-world → no-op.
    if dual_world_registry_unavailable() {
        return;
    }

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

/// C++ DeliverPayloadNugget formation flight poses (start / moveTo / target / yaw).
#[derive(Debug, Clone, PartialEq)]
pub struct DeliverPayloadFormationPose {
    pub start_pos: Coord3D,
    pub move_to_pos: Coord3D,
    pub target_pos: Coord3D,
    pub orient: Real,
}

impl DeliverPayloadNugget {
    /// Calculate formation offset vectors (CCW and CW perpendicular to approach)
    /// Matches C++ ObjectCreationList.cpp:271-298
    pub fn calculate_formation_vectors(
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
    /// Matches C++ ObjectCreationList.cpp:303-319
    pub fn calculate_formation_offset(
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

    /// Full C++ formation flight matrix for one transport:
    /// offset → start/moveTo/target, optional error radius, distToTarget slop.
    pub fn formation_flight_pose(
        primary: &Coord3D,
        secondary: &Coord3D,
        formation_index: Int,
        formation_size: Int,
        formation_spacing: Real,
        convergence_factor: Real,
        dist_to_target: Real,
        error_sample: Option<(Real, Real)>,
    ) -> DeliverPayloadFormationPose {
        let (ccw_x, ccw_y, cw_x, cw_y) = if formation_size > 1 {
            Self::calculate_formation_vectors(primary, secondary)
        } else {
            (0.0, 0.0, 0.0, 0.0)
        };
        let offset = Self::calculate_formation_offset(
            formation_index,
            formation_size,
            formation_spacing,
            ccw_x,
            ccw_y,
            cw_x,
            cw_y,
        );

        let mut start_pos = *primary;
        start_pos.x += offset.x;
        start_pos.y += offset.y;

        let mut move_to_pos = *secondary;
        move_to_pos.x += offset.x;
        move_to_pos.y += offset.y;

        let mut target_pos = *secondary;
        target_pos.x += offset.x * (1.0 - convergence_factor);
        target_pos.y += offset.y * (1.0 - convergence_factor);

        // First guy is always spot-on; later members may scatter by error radius.
        if formation_index > 0 {
            if let Some((random_radius, random_angle)) = error_sample {
                target_pos.x += random_radius * random_angle.cos();
                target_pos.y += random_radius * random_angle.sin();
            }
        }

        let orient = (move_to_pos.y - start_pos.y).atan2(move_to_pos.x - start_pos.x);
        if dist_to_target > 0.0 {
            const SLOP: Real = 1.5;
            start_pos.x -= orient.cos() * dist_to_target * SLOP;
            start_pos.y -= orient.sin() * dist_to_target * SLOP;
        }

        DeliverPayloadFormationPose {
            start_pos,
            move_to_pos,
            target_pos,
            orient,
        }
    }
}

impl ObjectCreationNugget for DeliverPayloadNugget {
    fn as_any(&self) -> &dyn Any {
        self
    }

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
        // Wave 445: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

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

        let mut first_transport: Option<Arc<RwLock<Object>>> = None;

        // Create each transport in formation
        for formation_index in 0..self.formation_size as Int {
            let error_sample = if self.error_radius > 1.0 && formation_index > 0 {
                Some((
                    ctx.game_logic.random_value_real(0.0, self.error_radius),
                    ctx.game_logic.random_value_real(0.0, PI * 2.0),
                ))
            } else {
                None
            };
            let pose = Self::formation_flight_pose(
                primary,
                secondary,
                formation_index,
                self.formation_size as Int,
                self.formation_spacing,
                self.convergence_factor,
                self.data.dist_to_target,
                error_sample,
            );
            let mut start_pos = pose.start_pos;
            let move_to_pos = pose.move_to_pos;
            let target_pos = pose.target_pos;
            let orient = pose.orient;

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

                // Apply random delivery delay (C++ always setDisabledUntil when max > 0,
                // including a rolled delay of 0 frames).
                if self.delay_delivery_frames_max > 0 {
                    let delay = ctx
                        .game_logic
                        .random_value(0, self.delay_delivery_frames_max as Int)
                        .max(0) as UnsignedInt;
                    if let Ok(mut transport_write) = transport.write() {
                        transport_write.set_disabled_until(
                            DisabledType::DisabledDefault,
                            ctx.game_logic.get_frame().saturating_add(delay),
                        );
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

            // C++ only applies max-speed / deliverPayload / height / payload when the
            // transport has DeliverPayloadAIUpdate. Motive force is applied first.
            let has_deliver_payload_ai = transport
                .read()
                .ok()
                .and_then(|transport_read| transport_read.get_ai_update_interface())
                .and_then(|ai| {
                    ai.lock().ok().map(|mut ai_guard| {
                        ai_guard.get_deliver_payload_ai_update_interface().is_some()
                    })
                })
                .unwrap_or(false);

            if !has_deliver_payload_ai {
                log::warn!(
                    "DeliverPayloadNugget transport '{}' missing DeliverPayloadAIUpdate",
                    self.transport_name
                );
                continue;
            }

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

            if let Ok(transport_read) = transport.read() {
                if let Some(ai) = transport_read.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        if let Some(deliver_ai) = ai_guard.get_deliver_payload_ai_update_interface()
                        {
                            let mut delivery_data = self.data.clone();
                            if formation_index > 0 {
                                delivery_data.delivery_decal_radius = 0.0;
                            }
                            deliver_ai.deliver_payload(&move_to_pos, &target_pos, &delivery_data);
                        }
                    }
                }
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
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn create_with_angle(
        &self,
        _ctx: &CreationContext<'_>,
        primary_obj: Option<&Object>,
        _primary: &Coord3D,
        secondary: &Coord3D,
        _angle: Real,
        _lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        // C++ FireWeaponNugget::create requires primaryObj + primary + secondary.
        // Lifetime is unused (same as C++).
        let Some(primary_object) = primary_obj else {
            return None;
        };

        if let Some(ref weapon_name) = self.weapon {
            let _ = crate::helpers::TheWeaponStore::create_and_fire_temp_weapon(
                weapon_name,
                primary_object,
                secondary,
            );
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
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn create_with_angle(
        &self,
        _ctx: &CreationContext<'_>,
        primary_obj: Option<&Object>,
        _primary: &Coord3D,
        secondary: &Coord3D,
        _angle: Real,
        _lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        // C++ AttackNugget::create requires primaryObj + primary + secondary.
        // Lifetime is unused (same as C++).
        let Some(primary_object) = primary_obj else {
            return None;
        };

        // C++ only locks + attacks when the primary has an AIUpdateInterface.
        // Command source is CMD_FROM_AI (not script).
        if let Some(ai_arc) = primary_object.get_ai_update_interface() {
            if !dual_world_registry_unavailable() {
                let _ = OBJECT_REGISTRY.with_object_mut(primary_object.get_id(), |primary_write| {
                    primary_write
                        .set_weapon_lock(self.weapon_slot, WeaponLockType::LockedTemporarily);
                });
            }
            ai_arc.ai_attack_position(secondary, self.number_of_shots, CMD_FROM_AI);
        }

        // C++ always asks RadiusDecalUpdate to create the decal when the module exists.
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
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn create_with_angle(
        &self,
        _ctx: &CreationContext<'_>,
        _primary_obj: Option<&Object>,
        _primary: &Coord3D,
        _secondary: &Coord3D,
        _angle: Real,
        _lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        // C++ DEBUG_CRASH: must be called with an object, not a location.
        None
    }

    fn create_with_objects(
        &self,
        ctx: &CreationContext<'_>,
        primary: &Object,
        _secondary: Option<&Object>,
        _lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        // Lifetime is unused (same as C++).
        if let Some(physics) = primary.get_physics() {
            let force = calc_random_force(
                ctx,
                self.min_mag,
                self.max_mag,
                self.min_pitch,
                self.max_pitch,
            );
            physics.apply_force(&force);

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
    use crate::modules::{AIUpdateInterface, PhysicsBehavior};
    use crate::object_creation_list::{GameLogicContext, TerrainLogicContext, ThingFactoryContext};
    use std::sync::Mutex;

    struct TestLogic;
    impl GameLogicContext for TestLogic {
        fn get_frame(&self) -> UnsignedInt {
            0
        }
        fn random_value(&self, lo: Int, _hi: Int) -> Int {
            lo
        }
        fn random_value_real(&self, lo: Real, _hi: Real) -> Real {
            lo
        }
    }

    struct TestFactory;
    impl ThingFactoryContext for TestFactory {
        fn find_template(&self, _name: &str) -> Option<Arc<dyn crate::common::ThingTemplate>> {
            None
        }
        fn new_object(
            &self,
            _template: Arc<dyn crate::common::ThingTemplate>,
            _team: &Team,
        ) -> Result<Arc<RwLock<Object>>, GameError> {
            Err(GameError::SystemError("test factory".into()))
        }
    }

    struct TestTerrain;
    impl TerrainLogicContext for TestTerrain {
        fn get_ground_height(&self, _x: Real, _y: Real) -> Real {
            0.0
        }
        fn get_layer_height(&self, _x: Real, _y: Real, _layer: PathfindLayerEnum) -> Real {
            0.0
        }
        fn get_highest_layer_for_destination(&self, _pos: &Coord3D) -> PathfindLayerEnum {
            PathfindLayerEnum::Ground
        }
        fn is_underwater(
            &self,
            _x: Real,
            _y: Real,
            _water_z: &mut Real,
            _terrain_z: &mut Real,
        ) -> Bool {
            false
        }
        fn flatten_terrain(&self, _object: &Object) {}
        fn find_closest_edge_point(&self, pos: &Coord3D) -> Coord3D {
            *pos
        }
    }

    static TEST_LOGIC: TestLogic = TestLogic;
    static TEST_FACTORY: TestFactory = TestFactory;
    static TEST_TERRAIN: TestTerrain = TestTerrain;

    fn test_ctx() -> CreationContext<'static> {
        CreationContext {
            game_logic: &TEST_LOGIC,
            thing_factory: &TEST_FACTORY,
            terrain_logic: &TEST_TERRAIN,
        }
    }

    #[derive(Debug, Default, Clone)]
    struct AttackRecord {
        pos: Option<Coord3D>,
        shots: i32,
        source: Option<CommandSourceType>,
    }

    #[derive(Debug)]
    struct RecordingAi {
        record: Arc<Mutex<AttackRecord>>,
    }

    impl AIUpdateInterface for RecordingAi {
        fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
        fn is_moving(&self) -> bool {
            false
        }
        fn is_idle(&self) -> bool {
            true
        }
        fn set_movement_target(&mut self, _target: &Coord3D) -> Result<(), String> {
            Ok(())
        }
        fn execute_command(
            &mut self,
            command: &crate::ai::AiCommandParams,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            if command.cmd == crate::ai::AiCommandType::AttackPosition {
                let mut rec = self.record.lock().unwrap();
                rec.pos = Some(command.pos);
                rec.shots = command.int_value;
                rec.source = Some(command.cmd_source);
            }
            Ok(())
        }
    }

    #[derive(Debug, Default, Clone)]
    struct ForceRecord {
        force: Option<Coord3D>,
        yaw: Real,
        roll: Real,
        pitch: Real,
    }

    #[derive(Debug)]
    struct RecordingPhysics {
        record: Arc<Mutex<ForceRecord>>,
        vel: Vec3D,
    }

    impl PhysicsBehavior for RecordingPhysics {
        fn update(&mut self, _dt: f32) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
        fn get_velocity(&self) -> Vec3D {
            self.vel
        }
        fn set_velocity(&mut self, velocity: &Vec3D) {
            self.vel = *velocity;
        }
        fn is_on_ground(&self) -> bool {
            true
        }
        fn apply_force(&mut self, force: &Vec3D) {
            self.record.lock().unwrap().force = Some(*force);
        }
        fn set_yaw_rate(&mut self, rate: Real) {
            self.record.lock().unwrap().yaw = rate;
        }
        fn set_roll_rate(&mut self, rate: Real) {
            self.record.lock().unwrap().roll = rate;
        }
        fn set_pitch_rate(&mut self, rate: Real) {
            self.record.lock().unwrap().pitch = rate;
        }
    }

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
    fn formation_flight_pose_matches_cpp_lead_and_wingmen() {
        let primary = Coord3D::new(100.0, 0.0, 10.0);
        let secondary = Coord3D::new(0.0, 0.0, 10.0);
        let spacing = 25.0;

        // Lead plane: no offset, yaw faces move-to.
        let lead = DeliverPayloadNugget::formation_flight_pose(
            &primary, &secondary, 0, 3, spacing, 0.0, 0.0, None,
        );
        assert!((lead.start_pos.x - 100.0).abs() < 1e-4);
        assert!((lead.start_pos.y - 0.0).abs() < 1e-4);
        assert!((lead.move_to_pos.x - 0.0).abs() < 1e-4);
        assert!((lead.target_pos.x - 0.0).abs() < 1e-4);
        assert!((lead.orient - std::f32::consts::PI).abs() < 1e-3);

        // Index 1 (odd): CCW offset; multiplier = (1+1)/2 * spacing = 25.
        let (ccw_x, ccw_y, cw_x, cw_y) =
            DeliverPayloadNugget::calculate_formation_vectors(&primary, &secondary);
        let wing = DeliverPayloadNugget::formation_flight_pose(
            &primary, &secondary, 1, 3, spacing, 0.0, 0.0, None,
        );
        let expected_off = DeliverPayloadNugget::calculate_formation_offset(
            1, 3, spacing, ccw_x, ccw_y, cw_x, cw_y,
        );
        assert!((wing.start_pos.x - (primary.x + expected_off.x)).abs() < 1e-4);
        assert!((wing.start_pos.y - (primary.y + expected_off.y)).abs() < 1e-4);
        assert!((wing.move_to_pos.x - (secondary.x + expected_off.x)).abs() < 1e-4);
        assert!((wing.target_pos.x - (secondary.x + expected_off.x)).abs() < 1e-4);

        // Convergence 1.0: target stays on lead target (no offset).
        let conv = DeliverPayloadNugget::formation_flight_pose(
            &primary, &secondary, 1, 3, spacing, 1.0, 0.0, None,
        );
        assert!((conv.target_pos.x - secondary.x).abs() < 1e-4);
        assert!((conv.target_pos.y - secondary.y).abs() < 1e-4);

        // distToTarget slop pulls start back along heading.
        let slop = DeliverPayloadNugget::formation_flight_pose(
            &primary, &secondary, 0, 1, spacing, 0.0, 100.0, None,
        );
        let expected_back = 100.0 * 1.5;
        assert!((slop.start_pos.x - (100.0 + expected_back)).abs() < 1e-3);
        assert!((slop.orient - std::f32::consts::PI).abs() < 1e-3);

        // Error sample only applies after lead.
        let err = DeliverPayloadNugget::formation_flight_pose(
            &primary,
            &secondary,
            2,
            3,
            spacing,
            0.0,
            0.0,
            Some((10.0, 0.0)),
        );
        let off2 = DeliverPayloadNugget::calculate_formation_offset(
            2, 3, spacing, ccw_x, ccw_y, cw_x, cw_y,
        );
        assert!((err.target_pos.x - (secondary.x + off2.x + 10.0)).abs() < 1e-4);
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

    #[test]
    fn fire_weapon_create_returns_none_and_requires_primary() {
        let ctx = test_ctx();
        let nugget = FireWeaponNugget {
            weapon: Some("DemoTrapDetonationWeapon".into()),
        };
        let primary = Coord3D::new(1.0, 2.0, 3.0);
        let secondary = Coord3D::new(4.0, 5.0, 6.0);
        assert!(
            nugget
                .create_with_angle(&ctx, None, &primary, &secondary, 0.0, 12)
                .is_none()
        );

        let obj = Object::new_test(42, 100.0);
        assert!(
            nugget
                .create_with_angle(&ctx, Some(&obj), &primary, &secondary, 0.0, 12)
                .is_none()
        );
    }

    #[test]
    fn attack_create_locks_via_ai_and_uses_cmd_from_ai() {
        let ctx = test_ctx();
        let record = Arc::new(Mutex::new(AttackRecord::default()));
        let mut obj = Object::new_test(77, 100.0);
        let ai: Arc<Mutex<dyn AIUpdateInterface>> = Arc::new(Mutex::new(RecordingAi {
            record: Arc::clone(&record),
        }));
        obj.set_ai_update_interface(Some(ai));

        let nugget = AttackNugget {
            number_of_shots: 9,
            weapon_slot: WeaponSlotType::Primary,
            delivery_decal_template: RadiusDecalTemplate::default(),
            delivery_decal_radius: 200.0,
        };
        let primary = Coord3D::new(0.0, 0.0, 0.0);
        let secondary = Coord3D::new(10.0, 20.0, 0.0);
        assert!(
            nugget
                .create_with_angle(&ctx, Some(&obj), &primary, &secondary, 0.0, 0)
                .is_none()
        );

        let rec = record.lock().unwrap();
        assert_eq!(rec.shots, 9);
        assert_eq!(rec.source, Some(CommandSourceType::FromAi));
        assert_eq!(rec.pos, Some(secondary));
    }

    #[test]
    fn apply_random_force_create_applies_force_and_spin() {
        let ctx = test_ctx();
        let record = Arc::new(Mutex::new(ForceRecord::default()));
        let mut obj = Object::new_test(88, 100.0);
        let physics: Arc<Mutex<dyn PhysicsBehavior>> = Arc::new(Mutex::new(RecordingPhysics {
            record: Arc::clone(&record),
            vel: Vec3D::ZERO,
        }));
        obj.set_physics(Some(physics));

        let nugget = ApplyRandomForceNugget {
            spin_rate: 0.5,
            min_mag: 10.0,
            max_mag: 10.0,
            min_pitch: 0.0,
            max_pitch: 0.0,
        };
        assert!(
            nugget
                .create_with_angle(&ctx, Some(&obj), &Coord3D::ZERO, &Coord3D::ZERO, 0.0, 0)
                .is_none()
        );
        assert!(record.lock().unwrap().force.is_none());

        assert!(nugget.create_with_objects(&ctx, &obj, None, 7).is_none());
        let rec = record.lock().unwrap();
        let force = rec.force.expect("force applied");
        assert!((force.x - 10.0).abs() < 1e-4);
        assert!(force.y.abs() < 1e-4);
        assert!(force.z.abs() < 1e-4);
        assert!((rec.yaw + 0.5).abs() < 1e-4);
        assert!((rec.roll + 0.5).abs() < 1e-4);
        assert!((rec.pitch + 0.5).abs() < 1e-4);
    }
}
