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
// Advanced types live in `advanced_nuggets.rs`:
// - DeliverPayloadNugget (C++ lines 225-572)
// - FireWeaponNugget (C++ lines 105-148)
// - AttackNugget (C++ lines 151-221)
// - ApplyRandomForceNugget (C++ lines 595-670)

use super::{CreationContext, CreationResult};
use crate::common::*;
use crate::helpers::{
    FPF_USE_HIGHEST_LAYER, FindPositionOptions, TheGameLogic, ThePartitionManager,
};
use crate::modules::{
    BodyModuleInterfaceExt, ContainModuleInterfaceExt, PhysicsBehavior, PhysicsBehaviorExt,
};
use crate::object::Object;
use crate::object::drawable::{DebrisDrawAnims, DrawableArcExt, DrawableExt, apply_debris_draw};
use crate::weapon::WeaponTemplate;
use std::any::Any;
use std::f32::consts::PI;
use std::sync::{Arc, RwLock};

pub use game_engine::common::ini::StaticGameLODLevel;

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

    /// Downcast helper for parse/create inspection (INI tests, typed field checks).
    fn as_any(&self) -> &dyn Any;
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
    /// C++ ShadowType bitfield (`TheShadowNames` / SHADOW_* flags).
    pub shadow_type: u32,
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
            shadow_type: 0,
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

        // C++ ObjectCreationList.cpp:1298-1299:
        // if (m_requiresLivePlayer && (!sourceObj || !sourceObj->getControllingPlayer()
        //     || !sourceObj->getControllingPlayer()->isPlayerActive())) return NULL;
        if self.requires_live_player {
            let Some(obj) = source_obj else {
                return None;
            };
            let Some(player) = obj.get_controlling_player() else {
                return None;
            };
            let Ok(player_guard) = player.read() else {
                return None;
            };
            if !player_guard.is_player_active() {
                return None;
            }
        }

        // Determine owner team.
        // C++ ObjectCreationList.cpp:1302-1305 — start from Neutral default team,
        // then overwrite when the source has a controlling player.
        let mut debris_owner = crate::player::ThePlayerList()
            .read()
            .ok()
            .and_then(|list| list.get_neutral_player())
            .and_then(|neutral| {
                neutral
                    .read()
                    .ok()
                    .and_then(|player| player.get_default_team())
            });

        if let Some(obj) = source_obj {
            if let Some(player) = obj.get_controlling_player() {
                if let Ok(player_guard) = player.read() {
                    debris_owner = player_guard.get_default_team();
                }
            }
        }

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
                // C++ ObjectCreationList.cpp:1334-1339 — skip generic debris when LOD asks.
                if crate::helpers::TheGameLODManager::is_debris_skipped() {
                    continue;
                }
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

            // C++ ObjectCreationList.cpp:1356-1361
            // preserveLayer copies the source pathfind layer when not stuffing into a container.
            if self.preserve_layer && source_obj.is_some() && container.is_none() {
                if let Some(src) = source_obj {
                    let layer = src.get_layer();
                    if layer != PathfindLayerEnum::Ground {
                        if let Ok(mut debris_guard) = debris.write() {
                            debris_guard.set_layer(layer);
                        }
                    }
                }
            }

            // C++ ObjectCreationList.cpp:1363-1364 — stuff into container *before* doStuffToObj.
            if let Some(ref cont) = container {
                if let Ok(cont_guard) = cont.read() {
                    if let Some(contain_module) = cont_guard.get_contain() {
                        if let Ok(debris_guard) = debris.read() {
                            if contain_module.is_valid_container_for(&*debris_guard, true) {
                                contain_module.add_to_contain(&*debris_guard);
                            }
                        }
                    }
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

            // C++ ObjectCreationList.cpp:1387-1401 — fade after doStuffToObj.
            self.apply_fade_to_object(&debris, source_obj);
        }

        // C++ ObjectCreationList.cpp:1404-1405 — doStuffToObj on the container last.
        if let Some(ref cont) = container {
            self.apply_properties_to_object(
                ctx,
                cont,
                "",
                pos,
                mtx,
                orientation,
                source_obj,
                lifetime_frames,
            );
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

        // C++ ObjectCreationList.cpp:918-934 — LifetimeUpdate override when the module exists.
        apply_lifetime_override(
            &*obj_read,
            lifetime_frames,
            self.min_frames,
            self.max_frames,
        );

        // C++ ObjectCreationList.cpp:936-951 — debris model/anim walk when !m_nameAreObjects.
        if !self.name_are_objects {
            apply_debris_model_and_anims(self, ctx, &*obj_read, model_name);
        }

        // Apply offset
        let mut offset = self.offset;
        if let Some(matrix) = mtx {
            offset = adjust_vector(&offset, matrix);
        }

        let mut chunk_pos = Coord3D::new(pos.x + offset.x, pos.y + offset.y, pos.z + offset.z);

        // C++ ObjectCreationList.cpp:962-970 — attach named particle system.
        // Fail-closed via helpers::attach_particle_system_to_object (no panic).
        if !self.particle_sys_name.is_empty() {
            let _ = crate::helpers::attach_particle_system_to_object(
                &self.particle_sys_name,
                obj_read.get_id(),
            );
        }

        // C++ ObjectCreationList.cpp:972-977
        if self.ignore_primary_obstacle {
            if let (Some(src), Some(physics)) = (source_obj, obj_read.get_physics()) {
                physics.set_ignore_collisions_with(src.get_id());
            }
        }

        // Set initial health
        // Matches C++ lines 980-983
        if let Some(body) = obj_read.get_body_module() {
            let health_percent = ctx
                .game_logic
                .random_value_real(self.min_health, self.max_health);
            body.set_initial_health(health_percent * 100.0);
        }

        // C++ ObjectCreationList.cpp:985-994 — first SlavedUpdateInterface::onEnslave.
        if let Some(src) = source_obj {
            notify_first_slaved_update(&*obj_read, src.get_id());
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
                            // C++ TheScriptEngine->transferObjectName(sourceObj->getName(), obj)
                            let _ = crate::scripting::engine::transfer_object_name(
                                src.get_name(),
                                obj_read.get_id(),
                            );
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
        // C++ ObjectCreationList.cpp:1023-1057
        if self.disposition.has(DebrisDisposition::LIKE_EXISTING) {
            if let Ok(mut obj_write) = obj.write() {
                if let Some(matrix) = mtx {
                    obj_write.set_transform_matrix(matrix);
                } else {
                    let _ = obj_write.set_orientation(orientation);
                }
                let _ = obj_write.set_position(&chunk_pos);
                if let Some(src) = source_obj {
                    if src.is_above_terrain() {
                        if let Some(physics) = obj_write.get_physics() {
                            physics.set_allow_to_fall(true);
                        }
                    }
                }
                if obj_write.is_kind_of(KindOf::Structure) {
                    ctx.terrain_logic.flatten_terrain(&*obj_write);
                    let mut adjusted_pos = *obj_write.get_position();
                    // C++ uses the original `pos` xy for ground height, not chunkPos.
                    adjusted_pos.z = ctx.terrain_logic.get_ground_height(pos.x, pos.y);
                    let _ = obj_write.set_position(&adjusted_pos);
                    add_object_to_pathfind_map(&*obj_write);
                }
            }
        }

        // ON_GROUND_ALIGNED - place on ground with random orientation
        // C++ ObjectCreationList.cpp:1061-1072
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
                // C++ 1068-1069: slightly above bridges / non-ground layers.
                if layer != PathfindLayerEnum::Ground {
                    chunk_pos.z += 1.0;
                }
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
                if let Some(matrix) = mtx {
                    obj_write.set_transform_matrix(matrix);
                }
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
                physics.set_bounce_sound(Some(crate::common::audio::AudioEventRts::new(
                    &self.bounce_sound,
                )));

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

        // FLOATING — C++ ObjectCreationList.cpp:1212-1220
        if self.disposition.has(DebrisDisposition::FLOATING) {
            enable_float_update(&*obj_read);
        }

        // Contain inside source
        // Matches C++ ObjectCreationList.cpp:1222-1238
        // C++ stillborns the new object whenever contain fails (no module, invalid
        // capacity, or null source). Fail-closed on try_lock / add_to_contain miss.
        if self.contain_inside_source_object {
            let contained_ok = source_obj
                .and_then(|src| src.get_contain().map(|contain| (src, contain)))
                .and_then(|(src, contain)| {
                    let mut contain_guard = contain.try_lock().ok()?;
                    if !contain_guard.is_valid_container_for(&*obj_read, true) {
                        return None;
                    }
                    contain_guard.add_to_contain(&*obj_read).ok()?;
                    // Need to hide if they are hidden.
                    // Matches C++ ObjectCreationList.cpp:1230-1232
                    if let Some(src_draw) = src.get_drawable() {
                        if let Some(obj_draw) = obj_read.get_drawable() {
                            if src_draw.is_drawable_effectively_hidden() {
                                obj_draw.set_drawable_hidden(true);
                            }
                        }
                    }
                    Some(())
                })
                .is_some();
            if !contained_ok {
                // DEBUG_ASSERTCRASH + TheGameLogic->destroyObject(obj)
                let object_id = obj_read.id();
                drop(obj_read);
                let _ = TheGameLogic::destroy_object_by_id(object_id);
                return;
            }
        }

        // Dies on bad land (water, cliffs, impassable)
        // Matches C++ ObjectCreationList.cpp lines 1243-1284
        if self.dies_on_bad_land {
            apply_dies_on_bad_land(ctx, obj, obj_read);
            return;
        }

        // Drop the read lock at the end
        drop(obj_read);
    }

    /// C++ ObjectCreationList.cpp:1387-1401
    fn apply_fade_to_object(&self, debris: &Arc<RwLock<Object>>, source_obj: Option<&Object>) {
        if self.fade_in {
            play_ocl_fade_sound(&self.fade_sound_name, source_obj);
            if let Ok(debris_guard) = debris.read() {
                if let Some(drawable) = debris_guard.get_drawable() {
                    drawable.fade_in(self.fade_frames);
                }
            }
        }
        if self.fade_out {
            play_ocl_fade_sound(&self.fade_sound_name, source_obj);
            if let Ok(debris_guard) = debris.read() {
                if let Some(drawable) = debris_guard.get_drawable() {
                    drawable.fade_out(self.fade_frames);
                }
            }
        }
    }
}

impl ObjectCreationNugget for GenericObjectCreationNugget {
    fn as_any(&self) -> &dyn Any {
        self
    }

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

fn play_ocl_fade_sound(fade_sound_name: &str, source_obj: Option<&Object>) {
    if fade_sound_name.is_empty() {
        return;
    }
    let Some(audio) = crate::helpers::TheAudio::get() else {
        return;
    };
    let mut fade_event = crate::common::audio::AudioEventRts::new(fade_sound_name);
    if let Some(src) = source_obj {
        fade_event.set_object_id(src.get_id());
    }
    let _ = audio.add_audio_event(&fade_event);
}

/// C++ ObjectCreationList.cpp:918-934
fn apply_lifetime_override(
    obj: &Object,
    lifetime_frames: UnsignedInt,
    min_frames: UnsignedInt,
    max_frames: UnsignedInt,
) {
    let apply = |lup: &mut crate::object::behavior::LifetimeUpdate| {
        if lifetime_frames != 0 {
            lup.set_lifetime_range(lifetime_frames, lifetime_frames);
        } else if max_frames > 0 {
            lup.set_lifetime_range(min_frames, max_frames);
        }
    };

    if let Some(handle) = obj.find_update_module("LifetimeUpdate") {
        if handle
            .with_module_downcast::<crate::object::behavior::lifetime_update::LifetimeUpdateModule, _, _>(
                |module| apply(module.behavior_mut()),
            )
            .is_some()
        {
            return;
        }
    }
    let _ = obj.with_update_behavior_downcast::<crate::object::behavior::LifetimeUpdate, _, _>(
        "LifetimeUpdate",
        apply,
    );
}

/// C++ ObjectCreationList.cpp:1212-1220
fn enable_float_update(obj: &Object) {
    if let Some(handle) = obj.find_update_module("FloatUpdate") {
        if handle
            .with_module_downcast::<crate::object::behavior::FloatUpdateModule, _, _>(|module| {
                module.behavior_mut().set_enabled(true);
            })
            .is_some()
        {
            return;
        }
    }
    let _ = obj.with_update_behavior_downcast::<crate::object::behavior::FloatUpdate, _, _>(
        "FloatUpdate",
        |float_update| {
            float_update.set_enabled(true);
        },
    );
}

/// C++ ObjectCreationList.cpp:936-951
fn apply_debris_model_and_anims(
    nugget: &GenericObjectCreationNugget,
    ctx: &CreationContext<'_>,
    obj: &Object,
    model_name: &str,
) {
    let Some(drawable) = obj.get_drawable() else {
        return;
    };
    let Ok(mut drawable_guard) = drawable.write() else {
        return;
    };

    let color = if nugget.ok_to_change_model_color {
        obj.get_indicator_color().to_argb_u32() as i32
    } else {
        0
    };

    let fx_arc = nugget
        .fx_final
        .as_deref()
        .and_then(crate::helpers::TheFXListStore::lookup_fx_list);

    let anims = if !nugget.anim_sets.is_empty() {
        let which = ctx
            .game_logic
            .random_value(0, (nugget.anim_sets.len() - 1) as Int) as usize;
        let set = &nugget.anim_sets[which];
        Some(DebrisDrawAnims {
            initial: &set.anim_initial,
            flying: &set.anim_flying,
            final_anim: &set.anim_final,
            final_fx: fx_arc.as_deref(),
        })
    } else {
        None
    };

    apply_debris_draw(
        &mut *drawable_guard,
        model_name,
        color,
        nugget.shadow_type,
        anims,
    );
}

/// C++ ObjectCreationList.cpp:985-994
fn notify_first_slaved_update(obj: &Object, source_id: ObjectID) {
    for module in obj.get_behavior_modules() {
        let Ok(mut guard) = module.lock() else {
            continue;
        };
        if let Some(sdu) = guard.get_slaved_update_interface() {
            let _ = sdu.on_enslave(source_id);
            break;
        }
    }
}

/// C++ TheAI->pathfinder()->addObjectToPathfindMap(obj)
fn add_object_to_pathfind_map(obj: &Object) {
    let id = obj.get_id();
    let pos = *obj.get_position();
    let Ok(ai) = crate::ai::THE_AI.read() else {
        return;
    };
    let Some(pathfinder) = ai.pathfinder() else {
        return;
    };
    let Ok(mut pf) = pathfinder.write() else {
        return;
    };
    pf.add_object_to_map(id, &[pos], false);
}

/// C++ ObjectCreationList.cpp:1243-1284
fn apply_dies_on_bad_land(
    ctx: &CreationContext<'_>,
    obj: &Arc<RwLock<Object>>,
    obj_read: std::sync::RwLockReadGuard<'_, Object>,
) {
    let rider_pos = *obj_read.get_position();
    let layer = obj_read.get_layer();
    let mut water_z = 0.0;
    let mut terrain_z = 0.0;

    let flooded =
        ctx.terrain_logic
            .is_underwater(rider_pos.x, rider_pos.y, &mut water_z, &mut terrain_z)
            && rider_pos.z <= water_z + 10.0
            && layer == PathfindLayerEnum::Ground;

    let cell_type = pathfind_cell_type_at(&rider_pos, layer);
    let off_map = obj_read.is_off_map();
    drop(obj_read);

    if flooded {
        // C++: don't call kill(); specify DAMAGE_WATER + DEATH_FLOODED.
        if let Ok(mut obj_write) = obj.write() {
            let mut damage_info = crate::damage::DamageInfo {
                input: crate::damage::DamageInfoInput {
                    damage_type: crate::damage::DamageType::Water,
                    death_type: crate::damage::DeathType::Flooded,
                    source_id: INVALID_ID,
                    amount: crate::damage::HUGE_DAMAGE_AMOUNT,
                    ..Default::default()
                },
                ..Default::default()
            };
            damage_info.sync_from_input();
            let _ = obj_write.attempt_damage(&mut damage_info);
        }
    }

    use crate::ai::pathfind_astar::PathfindCellType;
    if off_map
        || cell_type == PathfindCellType::Cliff
        || cell_type == PathfindCellType::Water
        || cell_type == PathfindCellType::Impassable
    {
        if let Ok(mut obj_write) = obj.write() {
            obj_write.kill(None, None);
        }
    }
}

fn pathfind_cell_type_at(
    pos: &Coord3D,
    layer: PathfindLayerEnum,
) -> crate::ai::pathfind_astar::PathfindCellType {
    use crate::ai::pathfind_astar::{PathfindCellType, PathfindLayerEnum as AStarLayer};

    // C++ ObjectCreationList.cpp:1262-1266
    //   Int cellX = REAL_TO_INT(obj->getPosition()->x / PATHFIND_CELL_SIZE);
    //   PathfindCell* cell = TheAI->pathfinder()->getCell(obj->getLayer(), cellX, cellY);
    //   PathfindCell::CellType cellType = cell ? cell->getType() : CELL_IMPASSABLE;
    // Wrapper get_cell_type_at_layer uses REAL_TO_INT (truncate toward zero).
    let astar_layer = match layer {
        PathfindLayerEnum::Invalid => AStarLayer::Invalid,
        PathfindLayerEnum::Ground => AStarLayer::Ground,
        _ => AStarLayer::Top,
    };

    crate::ai::THE_AI
        .read()
        .ok()
        .and_then(|ai| ai.pathfinder())
        .and_then(|pf| {
            pf.read()
                .ok()
                .and_then(|pf| pf.get_cell_type_at_layer(pos, astar_layer))
        })
        .unwrap_or(PathfindCellType::Impassable)
}

/// Helper function to adjust vector by transformation matrix
/// Matches C++ adjustVector (ObjectCreationList.cpp:65-78)
fn adjust_vector(vec: &Coord3D, mtx: &Matrix3D) -> Coord3D {
    let vectmp = Vector3::new(vec.x, vec.y, vec.z);
    // glam Mat4 uses transform_vector3() instead of rotate_vector()
    let rotated = mtx.transform_vector3(vectmp);
    Coord3D::new(rotated.x, rotated.y, rotated.z)
}

/// Matrix result of C++ `calcRandomForce`:
/// `Scale(mag) * Rotate_Z(angle) * Rotate_Y(-pitch)` then `Get_X_Vector()`.
pub fn calc_random_force_xyz(angle: Real, pitch: Real, mag: Real) -> Coord3D {
    let horiz = mag * pitch.cos();
    let vert = mag * pitch.sin();
    Coord3D::new(horiz * angle.cos(), horiz * angle.sin(), vert)
}

/// Calculate random force with magnitude and pitch
/// Matches C++ calcRandomForce (ObjectCreationList.cpp:575-591)
pub fn calc_random_force(
    ctx: &CreationContext<'_>,
    min_mag: Real,
    max_mag: Real,
    min_pitch: Real,
    max_pitch: Real,
) -> Coord3D {
    let angle = ctx.game_logic.random_value_real(0.0, 2.0 * PI);
    let pitch = ctx.game_logic.random_value_real(min_pitch, max_pitch);
    let mag = ctx.game_logic.random_value_real(min_mag, max_mag);
    calc_random_force_xyz(angle, pitch, mag)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::types::DefaultThingTemplate;
    use crate::common::*;
    use crate::modules::{ContainModuleInterface, PhysicsBehavior};
    use crate::object::behavior::{
        FloatUpdate, FloatUpdateModuleData, LifetimeUpdate, LifetimeUpdateModuleData,
    };
    use crate::object::draw::W3DDebrisDraw;
    use crate::object::drawable::{Drawable, DrawableExt, DrawableType};
    use crate::object_creation_list::store::{
        ObjectCreationList, get_object_creation_list_store, load_object_creation_lists_from_str,
    };
    use crate::object_creation_list::{GameLogicContext, TerrainLogicContext, ThingFactoryContext};
    use crate::player::{Player, PlayerType, ThePlayerList};
    use crate::team::Team;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn calc_random_force_xyz_matches_cpp_identity_and_up_pitch() {
        let flat = calc_random_force_xyz(0.0, 0.0, 10.0);
        assert!((flat.x - 10.0).abs() < 1e-5);
        assert!(flat.y.abs() < 1e-5);
        assert!(flat.z.abs() < 1e-5);

        let up = calc_random_force_xyz(0.0, std::f32::consts::FRAC_PI_2, 10.0);
        assert!(up.x.abs() < 1e-4);
        assert!(up.y.abs() < 1e-4);
        assert!((up.z - 10.0).abs() < 1e-4);
    }

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
    static TEST_TERRAIN: TestTerrain = TestTerrain;
    static TEST_GLOBALS: Mutex<()> = Mutex::new(());
    static NEXT_OBJECT_ID: AtomicU32 = AtomicU32::new(80_000);

    #[derive(Debug)]
    struct RecordingPhysics {
        extra_friction: Arc<Mutex<Real>>,
        ignore_id: Arc<Mutex<ObjectID>>,
        allow_to_fall: Arc<Mutex<bool>>,
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
        fn apply_force(&mut self, _force: &Vec3D) {}
        fn set_extra_friction(&mut self, friction: Real) {
            *self.extra_friction.lock().unwrap() = friction;
        }
        fn set_ignore_collisions_with(&mut self, obj_id: ObjectID) {
            *self.ignore_id.lock().unwrap() = obj_id;
        }
        fn get_ignore_collisions_with(&self) -> ObjectID {
            *self.ignore_id.lock().unwrap()
        }
        fn set_allow_to_fall(&mut self, allow: bool) {
            *self.allow_to_fall.lock().unwrap() = allow;
        }
        fn get_allow_to_fall(&self) -> bool {
            *self.allow_to_fall.lock().unwrap()
        }
    }

    #[derive(Clone, Copy)]
    struct FactoryOptions {
        attach_float: bool,
        attach_lifetime: bool,
        attach_physics: bool,
        attach_drawable: bool,
        attach_debris_draw: bool,
        kind_of_structure: bool,
        register: bool,
        /// Also track in GameLogic.objects so destroyObject sets OBJECT_STATUS_DESTROYED.
        register_logic: bool,
        /// C++ ThingFactory objects always own an ExperienceTracker.
        attach_experience: bool,
        trainable: bool,
    }

    impl Default for FactoryOptions {
        fn default() -> Self {
            Self {
                attach_float: false,
                attach_lifetime: false,
                attach_physics: false,
                attach_drawable: false,
                attach_debris_draw: false,
                kind_of_structure: false,
                register: false,
                register_logic: false,
                attach_experience: false,
                trainable: false,
            }
        }
    }

    struct TestFactory {
        options: FactoryOptions,
        extra_friction: Arc<Mutex<Real>>,
        ignore_id: Arc<Mutex<ObjectID>>,
        allow_to_fall: Arc<Mutex<bool>>,
        created: Mutex<Vec<Arc<RwLock<Object>>>>,
    }

    impl TestFactory {
        fn new(options: FactoryOptions) -> Self {
            Self {
                options,
                extra_friction: Arc::new(Mutex::new(0.0)),
                ignore_id: Arc::new(Mutex::new(INVALID_ID)),
                allow_to_fall: Arc::new(Mutex::new(false)),
                created: Mutex::new(Vec::new()),
            }
        }
    }

    impl Drop for TestFactory {
        fn drop(&mut self) {
            let ids: Vec<ObjectID> = if let Ok(created) = self.created.lock() {
                created
                    .iter()
                    .filter_map(|obj| obj.read().ok().map(|guard| guard.get_id()))
                    .collect()
            } else {
                Vec::new()
            };
            for id in &ids {
                crate::object::registry::OBJECT_REGISTRY.unregister_object(*id);
                if self.options.register_logic {
                    let _ = TheGameLogic::destroy_object_by_id(*id);
                }
            }
            if self.options.register_logic {
                if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
                    let _ = logic.cleanup_dead_objects();
                }
            }
        }
    }

    impl ThingFactoryContext for TestFactory {
        fn find_template(&self, name: &str) -> Option<Arc<dyn crate::common::ThingTemplate>> {
            Some(Arc::new(DefaultThingTemplate::new(name.to_string())))
        }

        fn new_object(
            &self,
            template: Arc<dyn crate::common::ThingTemplate>,
            _team: &Team,
        ) -> Result<Arc<RwLock<Object>>, GameError> {
            let id = NEXT_OBJECT_ID.fetch_add(1, Ordering::SeqCst);
            let mut obj = Object::new_test(id, 100.0);
            if self.options.kind_of_structure {
                let mut tmpl = DefaultThingTemplate::new(template.get_name().to_string());
                tmpl.add_kind_of(KindOf::Structure);
                obj.set_template_for_test(Arc::new(tmpl));
            }
            if self.options.attach_physics {
                obj.set_physics(Some(Arc::new(Mutex::new(RecordingPhysics {
                    extra_friction: Arc::clone(&self.extra_friction),
                    ignore_id: Arc::clone(&self.ignore_id),
                    allow_to_fall: Arc::clone(&self.allow_to_fall),
                    vel: Vec3D::ZERO,
                }))));
            }
            if self.options.attach_drawable || self.options.attach_debris_draw {
                let mut drawable =
                    Drawable::new(id, id, "TestModel".to_string(), DrawableType::Static);
                if self.options.attach_debris_draw {
                    drawable.attach_w3d_debris_draw();
                }
                obj.set_drawable(Some(Arc::new(RwLock::new(drawable))));
            }
            if self.options.attach_experience {
                obj.attach_experience_tracker_for_test(self.options.trainable);
            }
            let arc = Arc::new(RwLock::new(obj));
            if self.options.register {
                crate::object::registry::OBJECT_REGISTRY.register_object(id, &arc);
            }
            if self.options.register_logic {
                let _ = TheGameLogic::register_object(Arc::clone(&arc));
            }
            if self.options.attach_float {
                let data: Arc<dyn crate::common::ModuleData> =
                    Arc::new(FloatUpdateModuleData::default());
                let float_update =
                    FloatUpdate::new(Arc::clone(&arc), data).expect("FloatUpdate::new");
                arc.write()
                    .unwrap()
                    .push_behavior_module_for_test(Arc::new(Mutex::new(float_update)));
            }
            if self.options.attach_lifetime {
                let data: Arc<dyn crate::common::ModuleData> =
                    Arc::new(LifetimeUpdateModuleData::default());
                let lifetime =
                    LifetimeUpdate::new(Arc::clone(&arc), data).expect("LifetimeUpdate::new");
                arc.write()
                    .unwrap()
                    .push_behavior_module_for_test(Arc::new(Mutex::new(lifetime)));
            }
            self.created.lock().unwrap().push(Arc::clone(&arc));
            Ok(arc)
        }
    }

    fn test_ctx<'a>(factory: &'a TestFactory) -> CreationContext<'a> {
        CreationContext {
            game_logic: &TEST_LOGIC,
            thing_factory: factory,
            terrain_logic: &TEST_TERRAIN,
        }
    }

    fn test_ctx_with_terrain<'a>(
        factory: &'a TestFactory,
        terrain: &'a dyn TerrainLogicContext,
    ) -> CreationContext<'a> {
        CreationContext {
            game_logic: &TEST_LOGIC,
            thing_factory: factory,
            terrain_logic: terrain,
        }
    }

    struct LayeredTerrain {
        layer: PathfindLayerEnum,
        height: Real,
    }

    impl TerrainLogicContext for LayeredTerrain {
        fn get_ground_height(&self, _x: Real, _y: Real) -> Real {
            self.height
        }
        fn get_layer_height(&self, _x: Real, _y: Real, _layer: PathfindLayerEnum) -> Real {
            self.height
        }
        fn get_highest_layer_for_destination(&self, _pos: &Coord3D) -> PathfindLayerEnum {
            self.layer
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

    struct SpyFlattenTerrain {
        flatten_count: std::sync::atomic::AtomicUsize,
    }

    impl TerrainLogicContext for SpyFlattenTerrain {
        fn get_ground_height(&self, _x: Real, _y: Real) -> Real {
            12.0
        }
        fn get_layer_height(&self, _x: Real, _y: Real, _layer: PathfindLayerEnum) -> Real {
            12.0
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
        fn flatten_terrain(&self, _object: &Object) {
            self.flatten_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
        fn find_closest_edge_point(&self, pos: &Coord3D) -> Coord3D {
            *pos
        }
    }

    struct UnderwaterTerrain {
        water_z: Real,
    }

    impl TerrainLogicContext for UnderwaterTerrain {
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
            water_z: &mut Real,
            terrain_z: &mut Real,
        ) -> Bool {
            *water_z = self.water_z;
            *terrain_z = 0.0;
            true
        }
        fn flatten_terrain(&self, _object: &Object) {}
        fn find_closest_edge_point(&self, pos: &Coord3D) -> Coord3D {
            *pos
        }
    }

    fn ensure_neutral_player_with_team() -> Arc<RwLock<Team>> {
        {
            let list = ThePlayerList().read().expect("player list");
            if let Some(neutral) = list.get_neutral_player() {
                drop(list);
                if let Ok(mut player) = neutral.write() {
                    if let Some(team) = player.get_default_team() {
                        return team;
                    }
                    let team = Arc::new(RwLock::new(Team::new(
                        AsciiString::from("teamOclNeutral"),
                        9_001,
                    )));
                    player.set_default_team(Some(Arc::clone(&team)));
                    return team;
                }
            }
        }

        let team = Arc::new(RwLock::new(Team::new(
            AsciiString::from("teamOclNeutral"),
            9_001,
        )));
        let mut player = Player::new(0);
        player.set_player_type(PlayerType::Neutral, false);
        player.set_default_team(Some(Arc::clone(&team)));
        ThePlayerList()
            .write()
            .expect("player list")
            .add_player(Arc::new(RwLock::new(player)));
        team
    }

    fn object_nugget(name: &str) -> GenericObjectCreationNugget {
        let mut nugget = GenericObjectCreationNugget::default();
        nugget.names = vec![name.to_string()];
        nugget.name_are_objects = true;
        nugget.debris_to_generate = 1;
        nugget
    }

    fn debris_nugget(model: &str) -> GenericObjectCreationNugget {
        let mut nugget = GenericObjectCreationNugget::default();
        nugget.names = vec![model.to_string()];
        nugget.name_are_objects = false;
        nugget.debris_to_generate = 1;
        nugget
    }

    #[derive(Debug)]
    struct TestOclContain {
        valid: bool,
        contained: Vec<ObjectID>,
    }

    impl ContainModuleInterface for TestOclContain {
        fn can_contain(&self, _object_id: ObjectID) -> bool {
            self.valid
        }
        fn contain_object(&mut self, object_id: ObjectID) -> Result<(), String> {
            if !self.valid {
                return Err("container rejected object".into());
            }
            self.contained.push(object_id);
            Ok(())
        }
        fn release_object(&mut self, object_id: ObjectID) -> Result<(), String> {
            self.contained.retain(|id| *id != object_id);
            Ok(())
        }
        fn get_contained_objects(&self) -> &[ObjectID] {
            &self.contained
        }
        fn get_contained_count(&self) -> usize {
            self.contained.len()
        }
        fn get_max_capacity(&self) -> usize {
            if self.valid { 1 } else { 0 }
        }
        fn is_valid_container_for(&self, _obj: &Object, _check_capacity: bool) -> bool {
            self.valid
        }
    }

    fn source_with_optional_contain(valid: Option<bool>, hidden: bool) -> Object {
        let id = NEXT_OBJECT_ID.fetch_add(1, Ordering::SeqCst);
        let mut source = Object::new_test(id, 100.0);
        if let Some(valid) = valid {
            source.set_contain(Some(Arc::new(Mutex::new(TestOclContain {
                valid,
                contained: Vec::new(),
            }))));
        }
        if hidden {
            let mut drawable = Drawable::new(id, id, "OclSrc".to_string(), DrawableType::Static);
            let _ = drawable.set_drawable_hidden(true);
            source.set_drawable(Some(Arc::new(RwLock::new(drawable))));
        }
        source
    }

    #[test]
    fn create_uses_neutral_player_team_when_source_is_missing() {
        let _guard = TEST_GLOBALS.lock().unwrap();
        ensure_neutral_player_with_team();
        let factory = TestFactory::new(FactoryOptions::default());
        let ctx = test_ctx(&factory);
        let nugget = object_nugget("AmericaInfantryRanger");
        let pos = Coord3D::new(10.0, 20.0, 0.0);
        let created = nugget.create_with_angle(&ctx, None, &pos, &pos, 0.0, 0);
        assert!(
            created.is_some(),
            "C++ still creates via Neutral default team"
        );
        assert_eq!(factory.created.lock().unwrap().len(), 1);
    }

    fn register_dummy_for_team_control() -> ObjectID {
        let dummy_id = NEXT_OBJECT_ID.fetch_add(1, Ordering::SeqCst);
        let dummy = Arc::new(RwLock::new(Object::new_test(dummy_id, 1.0)));
        crate::object::registry::OBJECT_REGISTRY.register_object(dummy_id, &dummy);
        dummy_id
    }

    fn source_with_live_player(defeated: bool) -> Object {
        let list_index = ThePlayerList()
            .read()
            .expect("player list")
            .get_player_count() as i32;
        let mut player = Player::new(list_index);
        player.set_defeated(defeated);
        ThePlayerList()
            .write()
            .expect("player list")
            .add_player(Arc::new(RwLock::new(player)));

        let team = Arc::new(RwLock::new(Team::new(
            AsciiString::from("teamOclRequiresLive"),
            9_200 + list_index as u32,
        )));
        team.write()
            .unwrap()
            .set_controlling_player_id(Some(list_index as u32));

        let id = NEXT_OBJECT_ID.fetch_add(1, Ordering::SeqCst);
        let mut source = Object::new_test(id, 100.0);
        source.set_team(Some(team)).unwrap();
        source
    }

    #[test]
    fn requires_live_player_returns_none_when_source_is_missing() {
        // C++ ObjectCreationList.cpp:1298-1299 — missing sourceObj is NULL.
        let _guard = TEST_GLOBALS.lock().unwrap();
        ensure_neutral_player_with_team();
        let factory = TestFactory::new(FactoryOptions::default());
        let ctx = test_ctx(&factory);
        let mut nugget = object_nugget("UsefulPilot");
        nugget.requires_live_player = true;
        let pos = Coord3D::new(0.0, 0.0, 0.0);
        assert!(
            nugget
                .create_with_angle(&ctx, None, &pos, &pos, 0.0, 0)
                .is_none()
        );
        assert!(factory.created.lock().unwrap().is_empty());
    }

    #[test]
    fn requires_live_player_returns_none_when_controlling_player_is_missing() {
        // C++ ObjectCreationList.cpp:1298-1299 — source without getControllingPlayer().
        let _guard = TEST_GLOBALS.lock().unwrap();
        ensure_neutral_player_with_team();
        let factory = TestFactory::new(FactoryOptions::default());
        let ctx = test_ctx(&factory);
        let mut nugget = object_nugget("UsefulPilot");
        nugget.requires_live_player = true;
        let source = Object::new_test(70_010, 100.0);
        let pos = *source.get_position();
        assert!(
            nugget
                .create_with_angle(&ctx, Some(&source), &pos, &pos, 0.0, 0)
                .is_none()
        );
        assert!(factory.created.lock().unwrap().is_empty());
    }

    #[test]
    fn requires_live_player_returns_none_when_player_is_inactive() {
        // C++ ObjectCreationList.cpp:1298-1299 — !isPlayerActive().
        let _guard = TEST_GLOBALS.lock().unwrap();
        ensure_neutral_player_with_team();
        let dummy = register_dummy_for_team_control();
        let factory = TestFactory::new(FactoryOptions::default());
        let ctx = test_ctx(&factory);
        let mut nugget = object_nugget("UsefulPilot");
        nugget.requires_live_player = true;
        let source = source_with_live_player(true);
        assert!(
            source.get_controlling_player().is_some(),
            "test setup must attach a controlling player"
        );
        let pos = *source.get_position();
        assert!(
            nugget
                .create_with_angle(&ctx, Some(&source), &pos, &pos, 0.0, 0)
                .is_none()
        );
        assert!(factory.created.lock().unwrap().is_empty());
        crate::object::registry::OBJECT_REGISTRY.unregister_object(dummy);
    }

    #[test]
    fn requires_live_player_creates_when_player_is_active() {
        let _guard = TEST_GLOBALS.lock().unwrap();
        ensure_neutral_player_with_team();
        let dummy = register_dummy_for_team_control();
        let factory = TestFactory::new(FactoryOptions::default());
        let ctx = test_ctx(&factory);
        let mut nugget = object_nugget("UsefulPilot");
        nugget.requires_live_player = true;
        let source = source_with_live_player(false);
        let pos = *source.get_position();
        assert!(
            nugget
                .create_with_angle(&ctx, Some(&source), &pos, &pos, 0.0, 0)
                .is_some()
        );
        assert_eq!(factory.created.lock().unwrap().len(), 1);
        crate::object::registry::OBJECT_REGISTRY.unregister_object(dummy);
    }

    #[test]
    fn create_internal_still_runs_later_nuggets_when_one_needs_live_player() {
        // C++ ObjectCreationList.cpp:1524-1534 createInternal runs every nugget.
        let _guard = TEST_GLOBALS.lock().unwrap();
        ensure_neutral_player_with_team();
        let factory = TestFactory::new(FactoryOptions::default());
        let ctx = test_ctx(&factory);
        let mut gated = object_nugget("DeadPilot");
        gated.requires_live_player = true;
        let live = object_nugget("FireFieldSmall");
        let mut ocl = ObjectCreationList::new();
        ocl.add_nugget(Arc::new(gated));
        ocl.add_nugget(Arc::new(live));
        let pos = Coord3D::new(1.0, 2.0, 0.0);
        let created = ocl.create_with_angle(&ctx, None, &pos, &pos, 0.0, 0);
        assert!(
            created.is_some(),
            "later CreateObject must still run after RequiresLivePlayer NULL"
        );
        assert_eq!(factory.created.lock().unwrap().len(), 1);
    }

    #[test]
    fn floating_disposition_enables_float_update_if_present() {
        let _guard = TEST_GLOBALS.lock().unwrap();
        ensure_neutral_player_with_team();
        let factory = TestFactory::new(FactoryOptions {
            attach_float: true,
            ..FactoryOptions::default()
        });
        let ctx = test_ctx(&factory);
        let mut nugget = object_nugget("FloatingCrate");
        nugget.disposition = DebrisDisposition::new(DebrisDisposition::FLOATING);
        let pos = Coord3D::new(0.0, 0.0, 0.0);
        let created = nugget
            .create_with_angle(&ctx, None, &pos, &pos, 0.0, 0)
            .expect("created floating object");
        let obj = created.read().unwrap();
        let enabled = obj
            .with_update_behavior_downcast::<FloatUpdate, _, _>("FloatUpdate", |fu| fu.is_enabled())
            .expect("FloatUpdate attached");
        assert!(enabled);
    }

    #[test]
    fn fade_in_path_does_not_panic_and_fades_drawable() {
        let _guard = TEST_GLOBALS.lock().unwrap();
        ensure_neutral_player_with_team();
        let factory = TestFactory::new(FactoryOptions {
            attach_drawable: true,
            ..FactoryOptions::default()
        });
        let ctx = test_ctx(&factory);
        let mut nugget = object_nugget("FadeUnit");
        nugget.fade_in = true;
        nugget.fade_frames = 15;
        nugget.fade_sound_name = "TestFadeSound".to_string();
        let pos = Coord3D::new(1.0, 2.0, 3.0);
        let created = nugget
            .create_with_angle(&ctx, None, &pos, &pos, 0.0, 0)
            .expect("created fade object");
        let obj = created.read().unwrap();
        let drawable = obj.get_drawable().expect("drawable attached");
        let draw = drawable.read().unwrap();
        assert_eq!(draw.fading_mode(), Drawable::FADING_IN);
        assert_eq!(draw.time_to_fade(), 15);
    }

    #[test]
    fn generic_debris_is_skipped_when_lod_is_debris_skipped() {
        let _guard = TEST_GLOBALS.lock().unwrap();
        ensure_neutral_player_with_team();
        crate::helpers::TheGameLODManager::set_dynamic_debris_skip_mask(7);
        let factory = TestFactory::new(FactoryOptions::default());
        let ctx = test_ctx(&factory);
        let nugget = debris_nugget("wreckage.w3d");
        let pos = Coord3D::new(0.0, 0.0, 0.0);
        let created = nugget.create_with_angle(&ctx, None, &pos, &pos, 0.0, 0);
        crate::helpers::TheGameLODManager::set_dynamic_debris_skip_mask(0);
        assert!(created.is_none());
        assert!(factory.created.lock().unwrap().is_empty());
    }

    #[test]
    fn extra_friction_is_applied_on_send_it_out() {
        let _guard = TEST_GLOBALS.lock().unwrap();
        ensure_neutral_player_with_team();
        let factory = TestFactory::new(FactoryOptions {
            attach_physics: true,
            ..FactoryOptions::default()
        });
        let ctx = test_ctx(&factory);
        let mut nugget = object_nugget("FrictionChunk");
        nugget.disposition = DebrisDisposition::new(DebrisDisposition::SEND_IT_OUT);
        nugget.extra_friction = -0.25;
        nugget.disposition_intensity = 1.0;
        let pos = Coord3D::new(0.0, 0.0, 0.0);
        assert!(
            nugget
                .create_with_angle(&ctx, None, &pos, &pos, 0.0, 0)
                .is_some()
        );
        assert!((*factory.extra_friction.lock().unwrap() + 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn lifetime_override_is_applied_when_module_exists() {
        let _guard = TEST_GLOBALS.lock().unwrap();
        ensure_neutral_player_with_team();
        let factory = TestFactory::new(FactoryOptions {
            attach_lifetime: true,
            ..FactoryOptions::default()
        });
        let ctx = test_ctx(&factory);
        let nugget = object_nugget("TimedCrate");
        let pos = Coord3D::new(0.0, 0.0, 0.0);
        let created = nugget
            .create_with_angle(&ctx, None, &pos, &pos, 0.0, 45)
            .expect("created lifetime object");
        let obj = created.read().unwrap();
        let die_frame = obj
            .with_update_behavior_downcast::<LifetimeUpdate, _, _>("LifetimeUpdate", |lup| {
                lup.get_die_frame()
            })
            .expect("LifetimeUpdate attached");
        let current = crate::helpers::TheGameLogic::get_frame();
        assert_eq!(die_frame, current + 45);
    }

    #[test]
    fn preserve_layer_copies_source_layer_when_not_contained() {
        let _guard = TEST_GLOBALS.lock().unwrap();
        ensure_neutral_player_with_team();
        let factory = TestFactory::new(FactoryOptions::default());
        let ctx = test_ctx(&factory);
        let mut nugget = object_nugget("BridgeChunk");
        nugget.preserve_layer = true;
        // LIKE_EXISTING avoids ON_GROUND_ALIGNED overwriting the copied layer
        // (C++ doStuffToObj also calls setLayer from terrain when that flag is set).
        nugget.disposition = DebrisDisposition::new(DebrisDisposition::LIKE_EXISTING);
        let mut source = Object::new_test(70_001, 100.0);
        source.set_layer(PathfindLayerEnum::Bridge1);
        let pos = *source.get_position();
        let created = nugget
            .create_with_angle(&ctx, Some(&source), &pos, &pos, 0.0, 0)
            .expect("created layered object");
        assert_eq!(
            created.read().unwrap().get_layer(),
            PathfindLayerEnum::Bridge1
        );
    }

    #[test]
    fn particle_sys_name_dummy_template_does_not_panic() {
        let _guard = TEST_GLOBALS.lock().unwrap();
        ensure_neutral_player_with_team();
        let factory = TestFactory::new(FactoryOptions::default());
        let ctx = test_ctx(&factory);
        let mut nugget = object_nugget("ParticleChunk");
        nugget.particle_sys_name = "OclDummyNoSuchParticleTemplate".to_string();
        let pos = Coord3D::new(0.0, 0.0, 0.0);
        let created = nugget.create_with_angle(&ctx, None, &pos, &pos, 0.0, 0);
        assert!(
            created.is_some(),
            "object create must succeed without particle template"
        );
    }

    #[test]
    fn particle_sys_name_registered_template_records_object_id() {
        let _guard = TEST_GLOBALS.lock().unwrap();
        ensure_neutral_player_with_team();
        if !crate::helpers::register_test_particle_template("OclNuggetSmoke") {
            return;
        }
        let factory = TestFactory::new(FactoryOptions::default());
        let ctx = test_ctx(&factory);
        let mut nugget = object_nugget("ParticleChunk");
        nugget.particle_sys_name = "OclNuggetSmoke".to_string();
        let pos = Coord3D::new(0.0, 0.0, 0.0);
        let before = crate::helpers::test_particle_attach_count();
        let created = nugget
            .create_with_angle(&ctx, None, &pos, &pos, 0.0, 0)
            .expect("created object");
        let object_id = created.read().unwrap().get_id();
        assert_eq!(crate::helpers::test_particle_attach_count(), before + 1);
        assert_eq!(
            crate::helpers::test_last_attached_object_id(),
            Some(object_id)
        );
    }

    #[test]
    fn parse_ini_create_object_drives_fade_and_friction_fields() {
        let _guard = TEST_GLOBALS.lock().unwrap();
        ensure_neutral_player_with_team();
        let ini = r#"
ObjectCreationList OCL_NuggetCreateGaps
  CreateObject
    ObjectNames = AmericaInfantryRanger
    Count = 1
    Disposition = FLOATING SEND_IT_OUT
    ExtraFriction = -0.3
    FadeIn = Yes
    FadeTime = 1000
  End
End
"#;
        load_object_creation_lists_from_str(ini).expect("parse OCL");
        let store = get_object_creation_list_store();
        let ocl = store
            .as_ref()
            .and_then(|s| s.find_object_creation_list("OCL_NuggetCreateGaps"))
            .expect("parsed OCL");
        let nugget = ObjectCreationNugget::as_any(ocl.nuggets()[0].as_ref())
            .downcast_ref::<GenericObjectCreationNugget>()
            .expect("CreateObject nugget");
        assert!(nugget.disposition.has(DebrisDisposition::FLOATING));
        assert!(nugget.disposition.has(DebrisDisposition::SEND_IT_OUT));
        assert!(nugget.fade_in);
        assert_eq!(nugget.fade_frames, 30);
        // C++ parseFrictionPerSec stores ExtraFriction * SECONDS_PER_LOGICFRAME_REAL (1/30).
        assert!((nugget.extra_friction + 0.01).abs() < 1e-5);

        let factory = TestFactory::new(FactoryOptions {
            attach_float: true,
            attach_physics: true,
            attach_drawable: true,
            ..FactoryOptions::default()
        });
        let ctx = test_ctx(&factory);
        let pos = Coord3D::new(5.0, 5.0, 0.0);
        let created = nugget
            .create_with_angle(&ctx, None, &pos, &pos, 0.0, 0)
            .expect("INI-driven create");
        let obj = created.read().unwrap();
        assert!(
            obj.with_update_behavior_downcast::<FloatUpdate, _, _>("FloatUpdate", |fu| fu
                .is_enabled())
                .unwrap()
        );
        assert!((*factory.extra_friction.lock().unwrap() + 0.01).abs() < 1e-5);
        let drawable = obj.get_drawable().unwrap();
        assert_eq!(drawable.read().unwrap().fading_mode(), Drawable::FADING_IN);
    }

    #[test]
    fn on_ground_aligned_non_ground_layer_adds_one_z() {
        let _guard = TEST_GLOBALS.lock().unwrap();
        ensure_neutral_player_with_team();
        let factory = TestFactory::new(FactoryOptions::default());
        let terrain = LayeredTerrain {
            layer: PathfindLayerEnum::Bridge1,
            height: 40.0,
        };
        let ctx = test_ctx_with_terrain(&factory, &terrain);
        let mut nugget = object_nugget("BridgeCrate");
        nugget.disposition = DebrisDisposition::new(DebrisDisposition::ON_GROUND_ALIGNED);
        let pos = Coord3D::new(8.0, 9.0, 0.0);
        let created = nugget
            .create_with_angle(&ctx, None, &pos, &pos, 0.0, 0)
            .expect("created layered object");
        let z = created.read().unwrap().get_position().z;
        assert!(
            (z - 41.0).abs() < 1e-4,
            "C++ ON_GROUND_ALIGNED adds +1.0 on non-ground layers, got {z}"
        );
        assert_eq!(
            created.read().unwrap().get_layer(),
            PathfindLayerEnum::Bridge1
        );
    }

    #[test]
    fn dies_on_bad_land_water_uses_death_flooded_not_generic_kill() {
        let _guard = TEST_GLOBALS.lock().unwrap();
        ensure_neutral_player_with_team();
        let factory = TestFactory::new(FactoryOptions {
            register: true,
            ..FactoryOptions::default()
        });
        let terrain = UnderwaterTerrain { water_z: 20.0 };
        let ctx = test_ctx_with_terrain(&factory, &terrain);
        let mut nugget = object_nugget("DrowningScout");
        nugget.dies_on_bad_land = true;
        nugget.disposition = DebrisDisposition::new(DebrisDisposition::LIKE_EXISTING);
        let pos = Coord3D::new(15.0, 15.0, 5.0);
        let created = nugget
            .create_with_angle(&ctx, None, &pos, &pos, 0.0, 0)
            .expect("created drowning object");
        let obj = created.read().unwrap();
        let last = obj
            .get_last_damage_info()
            .expect("water path must attempt_damage, not skip");
        assert_eq!(last.input.damage_type, crate::damage::DamageType::Water);
        assert_eq!(last.input.death_type, crate::damage::DeathType::Flooded);
        assert_eq!(
            obj.get_last_death_type(),
            Some(crate::damage::DeathType::Flooded)
        );
    }

    #[test]
    fn contain_inside_without_contain_module_destroys_object() {
        let _guard = TEST_GLOBALS.lock().unwrap();
        ensure_neutral_player_with_team();
        let factory = TestFactory::new(FactoryOptions {
            register_logic: true,
            ..FactoryOptions::default()
        });
        let ctx = test_ctx(&factory);
        let mut nugget = object_nugget("StillbornPassenger");
        nugget.contain_inside_source_object = true;
        nugget.disposition = DebrisDisposition::new(DebrisDisposition::LIKE_EXISTING);
        let source = source_with_optional_contain(None, false);
        let pos = *source.get_position();
        let created = nugget
            .create_with_angle(&ctx, Some(&source), &pos, &pos, 0.0, 0)
            .expect("C++ still returns the object after destroyObject");
        assert!(
            created.read().unwrap().is_destroyed(),
            "no contain module must stillborn the created object"
        );
    }

    #[test]
    fn contain_inside_valid_container_contains_and_hides_when_source_hidden() {
        let _guard = TEST_GLOBALS.lock().unwrap();
        ensure_neutral_player_with_team();
        let factory = TestFactory::new(FactoryOptions {
            attach_drawable: true,
            register_logic: true,
            ..FactoryOptions::default()
        });
        let ctx = test_ctx(&factory);
        let mut nugget = object_nugget("HiddenPassenger");
        nugget.contain_inside_source_object = true;
        nugget.disposition = DebrisDisposition::new(DebrisDisposition::LIKE_EXISTING);
        let source = source_with_optional_contain(Some(true), true);
        let pos = *source.get_position();
        let created = nugget
            .create_with_angle(&ctx, Some(&source), &pos, &pos, 0.0, 0)
            .expect("created contained object");
        let obj = created.read().unwrap();
        assert!(
            !obj.is_destroyed(),
            "valid container must not destroy the created object"
        );
        let contain = source.get_contain().expect("test contain attached");
        assert_eq!(contain.get_contained_count(), 1);
        assert_eq!(contain.get_contained_objects(), vec![obj.get_id()]);
        let drawable = obj.get_drawable().expect("created drawable");
        assert!(
            drawable.is_drawable_effectively_hidden(),
            "C++ hides the new drawable when the source drawable is effectively hidden"
        );
    }

    #[test]
    fn contain_inside_invalid_capacity_destroys_object() {
        let _guard = TEST_GLOBALS.lock().unwrap();
        ensure_neutral_player_with_team();
        let factory = TestFactory::new(FactoryOptions {
            register_logic: true,
            ..FactoryOptions::default()
        });
        let ctx = test_ctx(&factory);
        let mut nugget = object_nugget("OverflowPassenger");
        nugget.contain_inside_source_object = true;
        nugget.disposition = DebrisDisposition::new(DebrisDisposition::LIKE_EXISTING);
        let source = source_with_optional_contain(Some(false), false);
        let pos = *source.get_position();
        let created = nugget
            .create_with_angle(&ctx, Some(&source), &pos, &pos, 0.0, 0)
            .expect("C++ still returns the object after destroyObject");
        assert!(
            created.read().unwrap().is_destroyed(),
            "invalid isValidContainerFor must stillborn the created object"
        );
        let contain = source.get_contain().expect("test contain attached");
        assert_eq!(contain.get_contained_count(), 0);
    }

    #[test]
    fn dies_on_bad_land_missing_pathfind_cell_treated_as_impassable_kills() {
        let _guard = TEST_GLOBALS.lock().unwrap();
        ensure_neutral_player_with_team();
        use crate::ai::pathfind_astar::PathfindCellType;
        // Default THE_AI pathfinder is a 1000x1000 Clear grid; negative cells are absent.
        let pos = Coord3D::new(-50.0, -50.0, 5.0);
        assert_eq!(
            super::pathfind_cell_type_at(&pos, PathfindLayerEnum::Ground),
            PathfindCellType::Impassable,
            "C++ missing PathfindCell is CELL_IMPASSABLE"
        );
        let factory = TestFactory::new(FactoryOptions {
            register: true,
            ..FactoryOptions::default()
        });
        let ctx = test_ctx(&factory);
        let mut nugget = object_nugget("ImpassableScout");
        nugget.dies_on_bad_land = true;
        nugget.disposition = DebrisDisposition::new(DebrisDisposition::LIKE_EXISTING);
        let created = nugget
            .create_with_angle(&ctx, None, &pos, &pos, 0.0, 0)
            .expect("created object on missing pathfind cell");
        let obj = created.read().unwrap();
        assert!(
            obj.is_effectively_dead() || obj.is_destroyed(),
            "C++ missing PathfindCell is CELL_IMPASSABLE and kill()s the object"
        );
    }

    #[test]
    fn like_existing_structure_calls_flatten() {
        let _guard = TEST_GLOBALS.lock().unwrap();
        ensure_neutral_player_with_team();
        let factory = TestFactory::new(FactoryOptions {
            kind_of_structure: true,
            ..FactoryOptions::default()
        });
        let terrain = SpyFlattenTerrain {
            flatten_count: std::sync::atomic::AtomicUsize::new(0),
        };
        let ctx = test_ctx_with_terrain(&factory, &terrain);
        let mut nugget = object_nugget("SneakAttackBunker");
        nugget.disposition = DebrisDisposition::new(DebrisDisposition::LIKE_EXISTING);
        let pos = Coord3D::new(30.0, 40.0, 7.0);
        let created = nugget
            .create_with_angle(&ctx, None, &pos, &pos, 0.0, 0)
            .expect("created structure");
        assert_eq!(
            terrain
                .flatten_count
                .load(std::sync::atomic::Ordering::SeqCst),
            1,
            "LIKE_EXISTING KINDOF_STRUCTURE must flatten terrain"
        );
        let z = created.read().unwrap().get_position().z;
        assert!(
            (z - 12.0).abs() < 1e-4,
            "flatten path restamps z to ground height, got {z}"
        );
        assert!(created.read().unwrap().is_kind_of(KindOf::Structure));
    }

    #[test]
    fn particle_name_attach_does_not_panic() {
        let _guard = TEST_GLOBALS.lock().unwrap();
        ensure_neutral_player_with_team();
        let factory = TestFactory::new(FactoryOptions::default());
        let ctx = test_ctx(&factory);
        let mut nugget = object_nugget("SparkCrate");
        nugget.particle_sys_name = "OclMissingParticleSystem".to_string();
        nugget.disposition = DebrisDisposition::new(DebrisDisposition::LIKE_EXISTING);
        let pos = Coord3D::new(1.0, 1.0, 1.0);
        assert!(
            nugget
                .create_with_angle(&ctx, None, &pos, &pos, 0.0, 0)
                .is_some()
        );
    }

    #[test]
    fn debris_set_model_name_called_when_not_name_are_objects() {
        let _guard = TEST_GLOBALS.lock().unwrap();
        ensure_neutral_player_with_team();
        let factory = TestFactory::new(FactoryOptions {
            attach_debris_draw: true,
            ..FactoryOptions::default()
        });
        let ctx = test_ctx(&factory);
        let mut nugget = debris_nugget("EXRockChunk");
        nugget.disposition = DebrisDisposition::new(DebrisDisposition::LIKE_EXISTING);
        nugget.anim_sets.push(AnimSet {
            anim_initial: "AnimInit".to_string(),
            anim_flying: "AnimFly".to_string(),
            anim_final: "AnimLand".to_string(),
        });
        nugget.shadow_type = crate::common::SHADOW_DECAL;
        let pos = Coord3D::new(2.0, 3.0, 4.0);
        let created = nugget
            .create_with_angle(&ctx, None, &pos, &pos, 0.0, 0)
            .expect("created debris");
        let obj = created.read().unwrap();
        let drawable = obj.get_drawable().expect("drawable");
        let draw = drawable.read().unwrap();
        draw.module_by_name(&AsciiString::from("W3DDebrisDraw"))
            .expect("W3DDebrisDraw attached")
            .with_module_downcast::<W3DDebrisDraw, _, _>(|module| {
                assert_eq!(module.model_name().as_str(), "EXRockChunk");
                assert_eq!(module.anim_initial().as_str(), "AnimInit");
                assert_eq!(module.anim_flying().as_str(), "AnimFly");
                assert_eq!(module.anim_final().as_str(), "AnimLand");
            })
            .expect("downcast W3DDebrisDraw");
    }

    #[test]
    fn inherit_veterancy_skips_when_created_tracker_is_not_trainable() {
        // C++ ObjectCreationList.cpp:996 isTrainable gate.
        let _guard = TEST_GLOBALS.lock().unwrap();
        ensure_neutral_player_with_team();
        let factory = TestFactory::new(FactoryOptions {
            attach_experience: true,
            trainable: false,
            register: true,
            ..FactoryOptions::default()
        });
        let ctx = test_ctx(&factory);
        let mut nugget = object_nugget("EjectedPilot");
        nugget.inherit_veterancy = true;
        let mut source = Object::new_test(70_100, 100.0);
        source.attach_experience_tracker_for_test(true);
        source
            .get_experience_tracker()
            .unwrap()
            .lock()
            .unwrap()
            .set_veterancy_level(VeterancyLevel::Elite);
        source.set_name(AsciiString::from("NamedPilot"));
        let pos = *source.get_position();
        let created = nugget
            .create_with_angle(&ctx, Some(&source), &pos, &pos, 0.0, 0)
            .expect("created");
        let obj = created.read().unwrap();
        let level = obj
            .get_experience_tracker()
            .unwrap()
            .lock()
            .unwrap()
            .get_veterancy_level();
        assert_eq!(level, VeterancyLevel::Regular);
        assert!(obj.get_name().is_empty());
    }

    #[test]
    fn inherit_veterancy_sets_rank_and_transfers_script_name() {
        // C++ ObjectCreationList.cpp:996-1005 isTrainable + transferObjectName.
        let _guard = TEST_GLOBALS.lock().unwrap();
        ensure_neutral_player_with_team();
        let factory = TestFactory::new(FactoryOptions {
            attach_experience: true,
            trainable: true,
            register: true,
            ..FactoryOptions::default()
        });
        let ctx = test_ctx(&factory);
        let mut nugget = object_nugget("EjectedPilot");
        nugget.inherit_veterancy = true;
        let mut source = Object::new_test(70_101, 100.0);
        source.attach_experience_tracker_for_test(true);
        source
            .get_experience_tracker()
            .unwrap()
            .lock()
            .unwrap()
            .set_veterancy_level(VeterancyLevel::Elite);
        source.set_name(AsciiString::from("NamedPilot"));
        let pos = *source.get_position();
        let created = nugget
            .create_with_angle(&ctx, Some(&source), &pos, &pos, 0.0, 0)
            .expect("created");
        let obj = created.read().unwrap();
        let level = obj
            .get_experience_tracker()
            .unwrap()
            .lock()
            .unwrap()
            .get_veterancy_level();
        assert_eq!(level, VeterancyLevel::Elite);
        assert_eq!(obj.get_name().as_str(), "NamedPilot");
    }
}
