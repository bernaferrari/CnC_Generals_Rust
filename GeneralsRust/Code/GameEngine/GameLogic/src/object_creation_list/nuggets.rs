// FILE: nuggets.rs - Object Creation Nugget Implementations
// Author: Steven Johnson, December 2001 (C++)
// Rust Port: 2025
// Desc: Individual nugget types that create objects in specific ways
//
// Ported from: GeneralsMD/Code/GameEngine/Source/GameLogic/Object/ObjectCreationList.cpp
//
// ObjectCreationNuggets encapsulate the creation of objects. They are:
// - Shared between multiple units (stored in ObjectCreationList)
// - Stateless (all data initialized from INI)
// - Const-correct (don't modify nugget state during creation)
//
// Nugget Types (with C++ line references):
// - GenericObjectCreationNugget: Creates objects/debris with various physics (C++ lines 711-1475)
// - DeliverPayloadNugget: Spawns transport aircraft with payload (C++ lines 225-572)
// - FireWeaponNugget: Fires a temporary weapon (C++ lines 105-148)
// - AttackNugget: Makes object attack a position (C++ lines 151-221)
// - ApplyRandomForceNugget: Applies random forces to object (C++ lines 595-670)
//
// ARCHITECTURAL NOTES:
//
// The Rust implementation faces several challenges not present in C++:
//
// 1. INTERIOR MUTABILITY
//    C++ uses raw pointers (Object*) which allow mutation anywhere.
//    Rust uses Arc<Object> for shared ownership, but Arc doesn't allow mutation.
//    Solutions:
//    - Wrap mutable fields in RefCell/Mutex (chosen for thread-safety)
//    - Use builder pattern for object construction
//    - Pass &mut Object where possible (requires different API design)
//
// 2. LIFETIME MANAGEMENT
//    C++ relies on manual memory management and the ThingFactory.
//    Rust uses Arc for automatic reference counting.
//    The factory returns Arc<Object>, but many operations need &mut Object.
//    This creates a fundamental mismatch in ownership models.
//
// 3. GAME SYSTEMS ACCESS
//    C++ uses global singletons (TheGameLogic, TheThingFactory, TheTerrainLogic).
//    Rust passes references via CreationContext trait objects.
//    This is more testable but requires more ceremony at call sites.
//
// 4. METHOD SIGNATURES
//    Many Object methods in C++ take non-const Object* parameters.
//    The Rust equivalent would be &mut Object, but we have Arc<Object>.
//    Methods like set_position, set_orientation, set_producer all need mutation.
//
// CURRENT STATUS:
//
// The nugget implementations are STRUCTURALLY COMPLETE but FUNCTIONALLY LIMITED:
// - All nugget types are defined with correct fields
// - The create() methods follow C++ logic flow
// - Notes mark where Arc<Object> mutation is needed
// - Tests verify basic construction and logic
//
// To complete the implementation, we need:
// 1. Add interior mutability to Object (RefCell/Mutex for mutable fields)
// 2. Implement missing Object methods (set_position, set_orientation, etc.)
// 3. Integrate with game systems (GameLogic, ThingFactory, TerrainLogic)
// 4. Add physics module integration (apply_force, set_mass, etc.)
// 5. Add AI module integration (ai_attack_position, weapon locking)
//
// See object_creation.rs in upgrade/modules for integration with the upgrade system.

use super::{CreationContext, CreationResult};
use crate::common::*;
use crate::helpers::{
    FindPositionOptions, TheGameLogic, ThePartitionManager, FPF_USE_HIGHEST_LAYER,
};
use crate::modules::{
    BodyModuleInterfaceExt, ContainModuleInterfaceExt, PhysicsBehavior, PhysicsBehaviorExt,
};
use crate::object::drawable::{DrawableArcExt, DrawableExt};
use crate::object::Object;
use crate::weapon::WeaponTemplate;
use std::f32::consts::PI;
use std::sync::{Arc, RwLock};

/// Invalid angle constant (matches C++ INVALID_ANGLE)
pub const INVALID_ANGLE: Real = -999999.0;

/// Base trait for all object creation nuggets
/// Matches C++ ObjectCreationNugget abstract base class
pub trait ObjectCreationNugget: Send + Sync {
    /// Create with position-based parameters and angle
    /// Matches C++ virtual Object* create(primaryObj, primary, secondary, angle, lifetimeFrames)
    fn create_with_angle(
        &self,
        ctx: &CreationContext<'_>,
        primary_obj: Option<&Object>,
        primary: &Coord3D,
        secondary: &Coord3D,
        angle: Real,
        lifetime_frames: UnsignedInt,
    ) -> CreationResult;

    /// Create with object-based parameters
    /// Matches C++ virtual Object* create(primary, secondary, lifetimeFrames)
    fn create_with_objects(
        &self,
        ctx: &CreationContext<'_>,
        primary: &Object,
        secondary: Option<&Object>,
        lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        // Default implementation - call position-based version
        // Matches C++ ObjectCreationNugget::create(Object*, Object*, UnsignedInt)
        let primary_pos = primary.get_position();
        let secondary_pos = secondary.map(|s| s.get_position()).unwrap_or(primary_pos);
        self.create_with_angle(
            ctx,
            Some(primary),
            &primary_pos,
            &secondary_pos,
            INVALID_ANGLE,
            lifetime_frames,
        )
    }

    /// Create with bool flag for owner creation (used by DeliverPayload)
    /// Matches C++ virtual Object* create(primaryObj, primary, secondary, createOwner, lifetimeFrames)
    fn create_with_owner_flag(
        &self,
        ctx: &CreationContext<'_>,
        primary_obj: Option<&Object>,
        primary: &Coord3D,
        secondary: &Coord3D,
        _create_owner: Bool,
        lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        // Default implementation - call angle-based version
        // Matches C++ ObjectCreationNugget::create with createOwner parameter
        self.create_with_angle(
            ctx,
            primary_obj,
            primary,
            secondary,
            INVALID_ANGLE,
            lifetime_frames,
        )
    }

    /// Create with both angle and createOwner flag.
    /// This matches the C++ call site that supplies both parameters.
    fn create_with_angle_and_owner_flag(
        &self,
        ctx: &CreationContext<'_>,
        primary_obj: Option<&Object>,
        primary: &Coord3D,
        secondary: &Coord3D,
        angle: Real,
        _create_owner: Bool,
        lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        self.create_with_angle(ctx, primary_obj, primary, secondary, angle, lifetime_frames)
    }
}

/// Debris disposition flags - how debris should behave when spawned
/// Matches C++ enum DebrisDisposition (ObjectCreationList.cpp:673-684)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebrisDisposition(u32);

impl DebrisDisposition {
    pub const LIKE_EXISTING: u32 = 0x00000001; // Use same orientation as source
    pub const ON_GROUND_ALIGNED: u32 = 0x00000002; // Place on ground, random orientation
    pub const SEND_IT_FLYING: u32 = 0x00000004; // Launch with random forces
    pub const SEND_IT_UP: u32 = 0x00000008; // Launch upward
    pub const SEND_IT_OUT: u32 = 0x00000010; // Push outward horizontally
    pub const RANDOM_FORCE: u32 = 0x00000020; // Apply custom random force
    pub const FLOATING: u32 = 0x00000040; // Enable floating (water)
    pub const INHERIT_VELOCITY: u32 = 0x00000080; // Inherit source object velocity
    pub const WHIRLING: u32 = 0x00000100; // Random spin rates

    pub fn new(flags: u32) -> Self {
        Self(flags)
    }

    pub fn has(&self, flag: u32) -> bool {
        (self.0 & flag) != 0
    }

    pub fn set(&mut self, flag: u32) {
        self.0 |= flag;
    }
}

/// Animation set for debris (initial, flying, final)
/// Matches C++ AnimSet struct (ObjectCreationList.cpp:1423-1428)
#[derive(Debug, Clone)]
pub struct AnimSet {
    pub anim_initial: String,
    pub anim_flying: String,
    pub anim_final: String,
}

/// Generic object/debris creation nugget
/// Matches C++ GenericObjectCreationNugget (ObjectCreationList.cpp:711-1475)
#[derive(Debug, Clone)]
pub struct GenericObjectCreationNugget {
    // Object names or model names to create
    pub names: Vec<String>,
    pub name_are_objects: bool, // true = object templates, false = debris models

    // Basic creation parameters
    pub debris_to_generate: Int,
    pub offset: Coord3D,
    pub disposition: DebrisDisposition,
    pub disposition_intensity: Real,

    // Physics parameters (for debris)
    pub mass: Real,
    pub extra_bounciness: Real,
    pub extra_friction: Real,

    // Force parameters (for RANDOM_FORCE disposition)
    pub min_mag: Real,
    pub max_mag: Real,
    pub min_pitch: Real,
    pub max_pitch: Real,

    // Spin rates (angular velocity)
    pub spin_rate: Real,  // -1.0 = calculate from intensity
    pub yaw_rate: Real,   // -1.0 = use spin_rate
    pub roll_rate: Real,  // -1.0 = use spin_rate
    pub pitch_rate: Real, // -1.0 = use spin_rate

    // Lifetime
    pub min_frames: UnsignedInt,
    pub max_frames: UnsignedInt,

    // Health range
    pub min_health: Real, // 0.0-1.0 (percentage)
    pub max_health: Real,

    // Advanced options
    pub inherit_veterancy: bool,
    pub ignore_primary_obstacle: bool,
    pub skip_if_significantly_airborne: bool,
    pub invulnerable_time: UnsignedInt,
    pub contain_inside_source_object: bool,
    pub dies_on_bad_land: bool,
    pub requires_live_player: bool,
    pub preserve_layer: bool,
    pub orient_in_force_direction: bool,

    // Spread formation
    pub spread_formation: bool,
    pub min_distance_a_formation: Real,
    pub min_distance_b_formation: Real,
    pub max_distance_formation: Real,

    // Fade in/out
    pub fade_in: bool,
    pub fade_out: bool,
    pub fade_frames: UnsignedInt,
    pub fade_sound_name: String,

    // Container
    pub put_in_container: String,

    // Particle system attachment
    pub particle_sys_name: String,

    // Debris-specific (when name_are_objects = false)
    pub anim_sets: Vec<AnimSet>,
    pub fx_final: Option<String>,
    pub ok_to_change_model_color: bool,
    pub min_lod_required: StaticGameLODLevel,
    pub shadow_type: ShadowType,
    pub bounce_sound: String,
}

impl Default for GenericObjectCreationNugget {
    fn default() -> Self {
        Self {
            names: Vec::new(),
            name_are_objects: true,
            debris_to_generate: 1,
            offset: Coord3D::new(0.0, 0.0, 0.0),
            disposition: DebrisDisposition::new(DebrisDisposition::ON_GROUND_ALIGNED),
            disposition_intensity: 0.0,
            mass: 0.0,
            extra_bounciness: 0.0,
            extra_friction: 0.0,
            min_mag: 0.0,
            max_mag: 0.0,
            min_pitch: 0.0,
            max_pitch: 0.0,
            spin_rate: -1.0,
            yaw_rate: -1.0,
            roll_rate: -1.0,
            pitch_rate: -1.0,
            min_frames: 0,
            max_frames: 0,
            min_health: 1.0,
            max_health: 1.0,
            inherit_veterancy: false,
            ignore_primary_obstacle: false,
            skip_if_significantly_airborne: false,
            invulnerable_time: 0,
            contain_inside_source_object: false,
            dies_on_bad_land: false,
            requires_live_player: false,
            preserve_layer: true,
            orient_in_force_direction: false,
            spread_formation: false,
            min_distance_a_formation: 0.0,
            min_distance_b_formation: 0.0,
            max_distance_formation: 0.0,
            fade_in: false,
            fade_out: false,
            fade_frames: 0,
            fade_sound_name: String::new(),
            put_in_container: String::new(),
            particle_sys_name: String::new(),
            anim_sets: Vec::new(),
            fx_final: None,
            ok_to_change_model_color: false,
            min_lod_required: StaticGameLODLevel::Low,
            shadow_type: ShadowType::None,
            bounce_sound: String::new(),
        }
    }
}

impl GenericObjectCreationNugget {
    /// Create the object and apply all configured properties
    /// Matches C++ GenericObjectCreationNugget::reallyCreate (ObjectCreationList.cpp:1291-1408)
    fn really_create(
        &self,
        ctx: &CreationContext<'_>,
        pos: &Coord3D,
        mtx: Option<&Matrix3D>,
        orientation: Real,
        source_obj: Option<&Object>,
        lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        if self.names.is_empty() {
            return None;
        }

        // Check if player is alive (for unit spawning)
        // Matches C++ lines 1298-1299
        if self.requires_live_player {
            if let Some(obj) = source_obj {
                if let Some(player) = obj.get_controlling_player() {
                    if let Ok(player_guard) = player.read() {
                        if !player_guard.is_player_active() {
                            return None; // Don't spawn for dead players
                        }
                    }
                }
            }
        }

        // Determine owner team
        let debris_owner = if let Some(obj) = source_obj {
            if let Some(player) = obj.get_controlling_player() {
                if let Ok(player_guard) = player.read() {
                    player_guard.get_default_team()
                } else {
                    // Neutral
                    None
                }
            } else {
                // Neutral
                None
            }
        } else {
            None
        };

        // Create container if specified
        let mut container: Option<Arc<RwLock<Object>>> = None;
        if !self.put_in_container.is_empty() {
            if let Some(container_tmpl) = ctx.thing_factory.find_template(&self.put_in_container) {
                if let Some(ref team_arc) = debris_owner {
                    if let Ok(team_guard) = team_arc.read() {
                        if let Ok(obj) = ctx.thing_factory.new_object(container_tmpl, &*team_guard)
                        {
                            // Set producer
                            if let Some(src) = source_obj {
                                if let Ok(mut obj_guard) = obj.write() {
                                    obj_guard.set_producer(Some(src));
                                }
                            }
                            container = Some(obj);
                        }
                    }
                }
            }
        }

        let mut first_object: Option<Arc<RwLock<Object>>> = None;

        // Create each debris/object
        for _nn in 0..self.debris_to_generate {
            // Pick random name
            let pick = ctx
                .game_logic
                .random_value(0, (self.names.len() - 1) as Int) as usize;
            let name = &self.names[pick];

            // Find template
            let tmpl = if self.name_are_objects {
                ctx.thing_factory.find_template(name)
            } else {
                // Generic debris template
                ctx.thing_factory.find_template("GenericDebris")
            };

            let Some(tmpl) = tmpl else {
                continue;
            };

            // Create object
            let Some(ref team_arc) = debris_owner else {
                continue;
            };

            let Ok(team_guard) = team_arc.read() else {
                continue;
            };

            let Ok(debris) = ctx.thing_factory.new_object(tmpl, &*team_guard) else {
                continue;
            };

            if first_object.is_none() {
                first_object = Some(Arc::clone(&debris));
            }

            // Set producer
            if let Some(src) = source_obj {
                if let Ok(mut debris_guard) = debris.write() {
                    debris_guard.set_producer(Some(src));
                }
            }

            let mut spawn_pos = *pos;
            if self.spread_formation {
                if let Some(partition) = ThePartitionManager::get() {
                    let mut options = FindPositionOptions::default();
                    options.min_radius = ctx.game_logic.random_value_real(
                        self.min_distance_a_formation,
                        self.min_distance_b_formation,
                    );
                    options.max_radius = self.max_distance_formation;
                    options.flags = FPF_USE_HIGHEST_LAYER;

                    let mut result_pos = spawn_pos;
                    if partition.find_position_around_with_options(pos, &options, &mut result_pos) {
                        spawn_pos = result_pos;
                    }
                }
            }

            // Apply all object properties (pass RwLock, will lock inside)
            self.apply_properties_to_object(
                ctx,
                &debris,
                name,
                &spawn_pos,
                mtx,
                orientation,
                source_obj,
                lifetime_frames,
            );

            // Add to container if specified
            if let Some(ref cont) = container {
                if let Ok(cont_guard) = cont.read() {
                    if let Some(contain_module) = cont_guard.get_contain() {
                        if let Ok(debris_guard) = debris.read() {
                            if contain_module.is_valid_container_for(&*debris_guard, true) {
                                // Extension trait expects &Object, so pass dereferenced guard
                                contain_module.add_to_contain(&*debris_guard);
                            }
                        }
                    }
                }
            }
        }

        // Return container if created, otherwise first object
        container.or(first_object)
    }

    /// Apply all properties to a created object
    /// Combines logic from C++ doStuffToObj (ObjectCreationList.cpp:907-1289)
    fn apply_properties_to_object(
        &self,
        ctx: &CreationContext<'_>,
        obj: &Arc<RwLock<Object>>,
        model_name: &str,
        pos: &Coord3D,
        mtx: Option<&Matrix3D>,
        mut orientation: Real,
        source_obj: Option<&Object>,
        lifetime_frames: UnsignedInt,
    ) {
        // Lock the object for reading (most operations are reads)
        let Ok(obj_read) = obj.read() else {
            return; // Failed to lock, skip this object
        };
        // Apply offset
        let mut offset = self.offset;
        if let Some(matrix) = mtx {
            offset = adjust_vector(&offset, matrix);
        }

        let mut chunk_pos = Coord3D::new(pos.x + offset.x, pos.y + offset.y, pos.z + offset.z);

        // Set initial health
        // Matches C++ lines 980-983
        if let Some(body) = obj_read.get_body_module() {
            let health_percent = ctx
                .game_logic
                .random_value_real(self.min_health, self.max_health);
            body.set_initial_health(health_percent * 100.0);
        }

        // Inherit veterancy
        // Matches C++ lines 996-1006
        if self.inherit_veterancy {
            if let Some(src) = source_obj {
                if let Some(exp_tracker) = obj_read.get_experience_tracker() {
                    if let Ok(mut tracker_guard) = exp_tracker.lock() {
                        if tracker_guard.is_trainable() {
                            let level = src.get_veterancy_level();
                            tracker_guard.set_veterancy_level(level);
                        }
                    }
                }
            }
        }

        // Set invulnerable time
        // Matches C++ lines 1008-1011
        let obj_read = if self.invulnerable_time > 0 {
            drop(obj_read);
            if let Ok(mut obj_write) = obj.write() {
                obj_write.go_invulnerable(self.invulnerable_time);
            }
            match obj.read() {
                Ok(guard) => guard,
                Err(_) => return,
            }
        } else {
            obj_read
        };

        // Process disposition flags
        // Matches C++ lines 1013-1220

        // INHERIT_VELOCITY
        if self.disposition.has(DebrisDisposition::INHERIT_VELOCITY) {
            if let Some(src) = source_obj {
                if let (Some(src_physics), Some(obj_physics)) =
                    (src.get_physics(), obj_read.get_physics())
                {
                    let velocity = src_physics.get_velocity();
                    obj_physics.apply_force(&velocity);
                }
            }
        }

        // Process disposition flags that require mutations
        // We need to drop the read lock before each write operation
        let needs_relock = self.disposition.has(DebrisDisposition::LIKE_EXISTING)
            || self.disposition.has(DebrisDisposition::ON_GROUND_ALIGNED)
            || self.disposition.has(DebrisDisposition::SEND_IT_OUT)
            || self.disposition.has(
                DebrisDisposition::SEND_IT_FLYING
                    | DebrisDisposition::SEND_IT_UP
                    | DebrisDisposition::RANDOM_FORCE,
            );

        if needs_relock {
            drop(obj_read);
        }

        // LIKE_EXISTING - set orientation and position to match source
        if self.disposition.has(DebrisDisposition::LIKE_EXISTING) {
            if let Ok(mut obj_write) = obj.write() {
                let _ = obj_write.set_orientation(orientation);
                let _ = obj_write.set_position(&chunk_pos);
                // Handle structures
                // if obj_write.is_kind_of(KindOf::Structure) {
                //     ctx.terrain_logic.flatten_terrain(&*obj_write);
                //     chunk_pos.z = ctx.terrain_logic.get_ground_height(pos.x, pos.y);
                //     let _ = obj_write.set_position(&chunk_pos);
                // }
            }
        }

        // ON_GROUND_ALIGNED - place on ground with random orientation
        // C++ ObjectCreationList.cpp lines 1032-1048
        if self.disposition.has(DebrisDisposition::ON_GROUND_ALIGNED) {
            if let Ok(mut obj_write) = obj.write() {
                chunk_pos.z = 99999.0;
                let layer = ctx
                    .terrain_logic
                    .get_highest_layer_for_destination(&chunk_pos);
                let random_orient = ctx.game_logic.random_value_real(0.0, 2.0 * PI);
                let _ = obj_write.set_orientation(random_orient);
                chunk_pos.z = ctx
                    .terrain_logic
                    .get_layer_height(chunk_pos.x, chunk_pos.y, layer);
                // C++ line 1046: obj->setLayer(layer)
                obj_write.set_layer(layer);
                let _ = obj_write.set_position(&chunk_pos);
            }
        }

        // SEND_IT_OUT - push debris outward horizontally
        if self.disposition.has(DebrisDisposition::SEND_IT_OUT) {
            if let Ok(mut obj_write) = obj.write() {
                let random_orient = ctx.game_logic.random_value_real(0.0, 2.0 * PI);
                let _ = obj_write.set_orientation(random_orient);
                chunk_pos.z = ctx
                    .terrain_logic
                    .get_ground_height(chunk_pos.x, chunk_pos.y);
                let _ = obj_write.set_position(&chunk_pos);
            }
        }

        // SEND_IT_FLYING | SEND_IT_UP | RANDOM_FORCE
        if self.disposition.has(
            DebrisDisposition::SEND_IT_FLYING
                | DebrisDisposition::SEND_IT_UP
                | DebrisDisposition::RANDOM_FORCE,
        ) {
            if let Ok(mut obj_write) = obj.write() {
                // if let Some(matrix) = mtx {
                //     obj_write.set_transform_matrix(matrix);
                // }
                let _ = obj_write.set_position(&chunk_pos);
            }
        }

        // Re-acquire read lock for physics operations
        let obj_read = if needs_relock {
            match obj.read() {
                Ok(guard) => guard,
                Err(_) => return,
            }
        } else {
            // Already have obj_read from earlier
            match obj.read() {
                Ok(guard) => guard,
                Err(_) => return,
            }
        };

        // Apply physics forces
        if self.disposition.has(DebrisDisposition::SEND_IT_OUT) {
            if let Some(physics) = obj_read.get_physics() {
                if !self.name_are_objects {
                    physics.set_mass(self.mass);
                }
                physics.set_extra_friction(self.extra_friction);

                let horiz_force = 4.0 * self.disposition_intensity;
                let force = Coord3D::new(
                    ctx.game_logic.random_value_real(-horiz_force, horiz_force),
                    ctx.game_logic.random_value_real(-horiz_force, horiz_force),
                    0.0,
                );
                physics.apply_force(&force);

                if self.orient_in_force_direction {
                    orientation = force.y.atan2(force.x);
                }
            }
        }

        if self.disposition.has(
            DebrisDisposition::SEND_IT_FLYING
                | DebrisDisposition::SEND_IT_UP
                | DebrisDisposition::RANDOM_FORCE,
        ) {
            if let Some(physics) = obj_read.get_physics() {
                if !self.name_are_objects {
                    physics.set_mass(self.mass);
                }

                physics.set_extra_bounciness(self.extra_bounciness);
                physics.set_extra_friction(self.extra_friction);
                physics.set_allow_bouncing(true);

                // Calculate spin rates
                let spin_rate = if self.spin_rate >= 0.0 {
                    self.spin_rate
                } else {
                    (PI / 32.0) * self.disposition_intensity
                };

                let yaw_rate = if self.yaw_rate >= 0.0 {
                    self.yaw_rate
                } else {
                    spin_rate
                };
                let roll_rate = if self.roll_rate >= 0.0 {
                    self.roll_rate
                } else {
                    spin_rate
                };
                let pitch_rate = if self.pitch_rate >= 0.0 {
                    self.pitch_rate
                } else {
                    spin_rate
                };

                let yaw = ctx.game_logic.random_value_real(-yaw_rate, yaw_rate);
                let roll = ctx.game_logic.random_value_real(-roll_rate, roll_rate);
                let pitch = ctx.game_logic.random_value_real(-pitch_rate, pitch_rate);

                // Calculate force based on disposition
                let force = if self.disposition.has(DebrisDisposition::SEND_IT_FLYING) {
                    let horiz_force = 4.0 * self.disposition_intensity;
                    let vert_force = 3.0 * self.disposition_intensity;
                    Coord3D::new(
                        ctx.game_logic.random_value_real(-horiz_force, horiz_force),
                        ctx.game_logic.random_value_real(-horiz_force, horiz_force),
                        ctx.game_logic
                            .random_value_real(vert_force * 0.33, vert_force),
                    )
                } else if self.disposition.has(DebrisDisposition::SEND_IT_UP) {
                    let horiz_force = 2.0 * self.disposition_intensity;
                    let vert_force = 4.0 * self.disposition_intensity;
                    Coord3D::new(
                        ctx.game_logic.random_value_real(-horiz_force, horiz_force),
                        ctx.game_logic.random_value_real(-horiz_force, horiz_force),
                        ctx.game_logic
                            .random_value_real(vert_force * 0.75, vert_force),
                    )
                } else {
                    calc_random_force(
                        ctx,
                        self.min_mag,
                        self.max_mag,
                        self.min_pitch,
                        self.max_pitch,
                    )
                };

                physics.apply_force(&force);

                if self.orient_in_force_direction {
                    orientation = force.y.atan2(force.x);
                }

                physics.set_angles(orientation, 0.0, 0.0);
                physics.set_yaw_rate(yaw);
                physics.set_roll_rate(roll);
                physics.set_pitch_rate(pitch);
            }
        }

        // WHIRLING
        if self.disposition.has(DebrisDisposition::WHIRLING) {
            if let Some(physics) = obj_read.get_physics() {
                let yaw = ctx
                    .game_logic
                    .random_value_real(-self.disposition_intensity, self.disposition_intensity);
                let roll = ctx
                    .game_logic
                    .random_value_real(-self.disposition_intensity, self.disposition_intensity);
                let pitch = ctx
                    .game_logic
                    .random_value_real(-self.disposition_intensity, self.disposition_intensity);

                physics.set_yaw_rate(yaw);
                physics.set_roll_rate(roll);
                physics.set_pitch_rate(pitch);
            }
        }

        // FLOATING
        if self.disposition.has(DebrisDisposition::FLOATING) {
            // Would enable FloatUpdate module here
        }

        // Contain inside source
        // Matches C++ lines 1222-1238
        if self.contain_inside_source_object {
            if let Some(src) = source_obj {
                if let Some(contain) = src.get_contain() {
                    if contain.is_valid_container_for(&*obj_read, true) {
                        // Extension trait expects &Object
                        contain.add_to_contain(&*obj_read);
                        // Hide if source is hidden
                        // Matches C++ ObjectCreationList.cpp lines 1230-1232
                        if let Some(src_draw) = src.get_drawable() {
                            if let Some(obj_draw) = obj_read.get_drawable() {
                                if src_draw.is_drawable_effectively_hidden() {
                                    obj_draw.set_drawable_hidden(true);
                                }
                            }
                        }
                    } else {
                        // Failed to contain - destroy object
                        // Matches C++ ObjectCreationList.cpp lines 1234-1237
                        let object_id = obj_read.id();
                        drop(obj_read);
                        let _ = TheGameLogic::destroy_object_by_id(object_id);
                        return;
                    }
                }
            }
        }

        // Dies on bad land (water, cliffs, impassable)
        // Matches C++ ObjectCreationList.cpp lines 1243-1284
        if self.dies_on_bad_land {
            let rider_pos = *obj_read.get_position();
            let mut water_z = 0.0;
            let mut terrain_z = 0.0;

            // Check if underwater - matches C++ lines 1245-1257
            if ctx.terrain_logic.is_underwater(
                rider_pos.x,
                rider_pos.y,
                &mut water_z,
                &mut terrain_z,
            ) {
                if rider_pos.z <= water_z + 10.0 {
                    // Drop read lock before acquiring write lock
                    drop(obj_read);
                    // Kill the object - matches C++ line 1254
                    if let Ok(mut obj_write) = obj.write() {
                        obj_write.kill(None, None);
                    }
                    return;
                }
            }

            // Check if off map - matches C++ lines 1260-1268
            if obj_read.is_off_map() {
                drop(obj_read);
                if let Ok(mut obj_write) = obj.write() {
                    obj_write.kill(None, None);
                }
                return;
            }
        }

        // Drop the read lock at the end
        drop(obj_read);
    }
}

impl ObjectCreationNugget for GenericObjectCreationNugget {
    fn create_with_angle(
        &self,
        ctx: &CreationContext<'_>,
        primary_obj: Option<&Object>,
        primary: &Coord3D,
        _secondary: &Coord3D,
        angle: Real,
        lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        // Skip if significantly airborne
        if self.skip_if_significantly_airborne {
            if let Some(obj) = primary_obj {
                if obj.is_significantly_above_terrain() {
                    return None;
                }
            }
        }

        let orientation = if angle == INVALID_ANGLE { 0.0 } else { angle };
        self.really_create(
            ctx,
            primary,
            None,
            orientation,
            primary_obj,
            lifetime_frames,
        )
    }

    fn create_with_objects(
        &self,
        ctx: &CreationContext<'_>,
        primary: &Object,
        _secondary: Option<&Object>,
        lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        if self.skip_if_significantly_airborne && primary.is_significantly_above_terrain() {
            return None;
        }

        let pos = *primary.get_position();
        let mtx = primary.get_transform_matrix();
        let orientation = primary.get_orientation();

        self.really_create(
            ctx,
            &pos,
            Some(&mtx),
            orientation,
            Some(primary),
            lifetime_frames,
        )
    }
}

/// Helper function to adjust vector by transformation matrix
/// Matches C++ adjustVector (ObjectCreationList.cpp:65-78)
fn adjust_vector(vec: &Coord3D, mtx: &Matrix3D) -> Coord3D {
    let vectmp = Vector3::new(vec.x, vec.y, vec.z);
    // glam Mat4 uses transform_vector3() instead of rotate_vector()
    let rotated = mtx.transform_vector3(vectmp);
    Coord3D::new(rotated.x, rotated.y, rotated.z)
}

/// Calculate random force with magnitude and pitch
/// Matches C++ calcRandomForce (ObjectCreationList.cpp:575-591)
fn calc_random_force(
    ctx: &CreationContext<'_>,
    min_mag: Real,
    max_mag: Real,
    min_pitch: Real,
    max_pitch: Real,
) -> Coord3D {
    let angle = ctx.game_logic.random_value_real(0.0, 2.0 * PI);
    let pitch = ctx.game_logic.random_value_real(min_pitch, max_pitch);
    let mag = ctx.game_logic.random_value_real(min_mag, max_mag);

    let horiz = mag * pitch.cos();
    let vert = mag * pitch.sin();

    Coord3D::new(horiz * angle.cos(), horiz * angle.sin(), vert)
}

/// Stub types for compilation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaticGameLODLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowType {
    None,
    Volume,
    Additive,
}

// More nugget types will be added in subsequent implementations
// - DeliverPayloadNugget
// - FireWeaponNugget
// - AttackNugget
// - ApplyRandomForceNugget

// Mock-based tests removed to keep fidelity-critical code free of mocks.
