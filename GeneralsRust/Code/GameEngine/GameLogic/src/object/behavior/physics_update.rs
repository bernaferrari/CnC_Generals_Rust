//! PhysicsBehavior - Rust conversion of C++ PhysicsBehavior/PhysicsUpdate
//!
//! Provides a lightweight rigid body update hook with snapshot parity.

use crate::common::audio::AudioEventRts;
use crate::common::xfer::XferExt;
use crate::common::{
    AsciiString, Coord3D, DisabledType, KindOf, ModuleData, ObjectID, ObjectStatusTypes, Real,
    UnsignedInt, XferVersion, LOGICFRAMES_PER_SECOND, MODELCONDITION_FREEFALL,
    MODELCONDITION_SPLATTED, MODELCONDITION_STUNNED, MODELCONDITION_STUNNED_FLAILING,
    SECONDS_PER_LOGICFRAME_REAL,
};
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::helpers::{TheGameLogic, TheTerrainLogic};
use crate::modules::{
    BehaviorModuleInterface, CollideModuleInterface, PhysicsBehavior as PhysicsBehaviorTrait,
    SleepyUpdatePhase, UpdateModuleInterface, UpdateSleepTime, UPDATE_SLEEP_FOREVER,
    UPDATE_SLEEP_NONE,
};
use crate::object::behavior::behavior_module::BehaviorModuleData;
use crate::object::Object as GameObject;
use game_engine::common::global_data;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use glam::{Mat4, Quat, Vec3};
use std::sync::{Arc, Mutex, RwLock, Weak};

const DEFAULT_MASS: Real = 1.0;
const DEFAULT_SHOCK_YAW: Real = 0.05;
const DEFAULT_SHOCK_PITCH: Real = 0.025;
const DEFAULT_SHOCK_ROLL: Real = 0.025;
const DEFAULT_FORWARD_FRICTION: Real = 0.15;
const DEFAULT_LATERAL_FRICTION: Real = 0.15;
const DEFAULT_Z_FRICTION: Real = 0.8;
const DEFAULT_AERO_FRICTION: Real = 0.0;
const MIN_AERO_FRICTION: Real = 0.0;
const MIN_NON_AERO_FRICTION: Real = 0.01;
const MAX_FRICTION: Real = 0.99;

const STUN_RELIEF_EPSILON: Real = 0.5;
const INVALID_VEL_MAG: Real = -1.0;

const MIN_ANGLE_TAN: Real = 3.0;
const TINY_DELTA: Real = 0.01;

const FLAG_IS_IN_UPDATE: i32 = 1 << 0;
const FLAG_UPDATE_EVER_RUN: i32 = 1 << 1;
const FLAG_WAS_AIRBORNE_LAST_FRAME: i32 = 1 << 2;
const FLAG_HAS_PITCHROLLYAW: i32 = 1 << 3;
const FLAG_IS_IN_FREEFALL: i32 = 1 << 4;
const FLAG_ALLOW_TO_FALL: i32 = 1 << 5;
const FLAG_STICK_TO_GROUND: i32 = 1 << 6;
const FLAG_APPLY_FRICTION2D_WHEN_AIRBORNE: i32 = 1 << 7;
const FLAG_ALLOW_BOUNCE: i32 = 1 << 8;
const FLAG_IMMUNE_TO_FALLING_DAMAGE: i32 = 1 << 9;
const FLAG_IS_STUNNED: i32 = 1 << 10;

#[derive(Clone, Debug)]
pub struct PhysicsBehaviorModuleData {
    pub base: BehaviorModuleData,
    pub mass: Real,
    pub shock_resistance: Real,
    pub shock_max_yaw: Real,
    pub shock_max_pitch: Real,
    pub shock_max_roll: Real,
    pub forward_friction: Real,
    pub lateral_friction: Real,
    pub z_friction: Real,
    pub aerodynamic_friction: Real,
    pub center_of_mass_offset: Real,
    pub kill_when_resting_on_ground: bool,
    pub allow_bouncing: bool,
    pub allow_collide_force: bool,
    pub min_fall_speed_for_damage: Real,
    pub fall_height_damage_factor: Real,
    pub pitch_roll_yaw_factor: Real,
    pub vehicle_crashes_into_building_weapon_template: AsciiString,
    pub vehicle_crashes_into_non_building_weapon_template: AsciiString,
}

impl Default for PhysicsBehaviorModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            mass: DEFAULT_MASS,
            shock_resistance: 0.0,
            shock_max_yaw: DEFAULT_SHOCK_YAW,
            shock_max_pitch: DEFAULT_SHOCK_PITCH,
            shock_max_roll: DEFAULT_SHOCK_ROLL,
            forward_friction: DEFAULT_FORWARD_FRICTION,
            lateral_friction: DEFAULT_LATERAL_FRICTION,
            z_friction: DEFAULT_Z_FRICTION,
            aerodynamic_friction: DEFAULT_AERO_FRICTION,
            center_of_mass_offset: 0.0,
            kill_when_resting_on_ground: false,
            allow_bouncing: false,
            allow_collide_force: true,
            min_fall_speed_for_damage: height_to_speed(40.0),
            fall_height_damage_factor: 1.0,
            pitch_roll_yaw_factor: 2.0,
            vehicle_crashes_into_building_weapon_template: AsciiString::from(
                "VehicleCrashesIntoBuildingWeapon",
            ),
            vehicle_crashes_into_non_building_weapon_template: AsciiString::from(
                "VehicleCrashesIntoNonBuildingWeapon",
            ),
        }
    }
}

crate::impl_behavior_module_data_via_base!(PhysicsBehaviorModuleData, base);

impl PhysicsBehaviorModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, PHYSICS_BEHAVIOR_FIELDS)
    }
}

#[derive(Debug, Clone)]
struct PhysicsBehaviorState {
    accel: Coord3D,
    prev_accel: Coord3D,
    vel: Coord3D,
    vel_mag: Real,
    yaw_rate: Real,
    roll_rate: Real,
    pitch_rate: Real,
    yaw_angle: Real,
    pitch_angle: Real,
    roll_angle: Real,
    turning: i32,
    ignore_collisions_with: ObjectID,
    flags: i32,
    mass: Real,
    current_overlap: ObjectID,
    previous_overlap: ObjectID,
    motive_force_expires: UnsignedInt,
    extra_bounciness: Real,
    extra_friction: Real,
    last_collidee: ObjectID,
    original_allow_bounce: bool,
}

impl PhysicsBehaviorState {
    fn new(mass: Real) -> Self {
        Self {
            accel: Coord3D::ZERO,
            prev_accel: Coord3D::ZERO,
            vel: Coord3D::ZERO,
            vel_mag: 0.0,
            yaw_rate: 0.0,
            roll_rate: 0.0,
            pitch_rate: 0.0,
            yaw_angle: 0.0,
            pitch_angle: 0.0,
            roll_angle: 0.0,
            turning: 0,
            ignore_collisions_with: crate::common::INVALID_ID,
            flags: 0,
            mass,
            current_overlap: crate::common::INVALID_ID,
            previous_overlap: crate::common::INVALID_ID,
            motive_force_expires: 0,
            extra_bounciness: 0.0,
            extra_friction: 0.0,
            last_collidee: crate::common::INVALID_ID,
            original_allow_bounce: false,
        }
    }

    fn has_flag(&self, flag: i32) -> bool {
        (self.flags & flag) != 0
    }

    fn set_flag(&mut self, flag: i32, enabled: bool) {
        if enabled {
            self.flags |= flag;
        } else {
            self.flags &= !flag;
        }
    }
}

#[derive(Debug)]
struct PhysicsBehaviorHandle {
    state: PhysicsBehaviorState,
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<PhysicsBehaviorModuleData>,
    bounce_sound: Option<AudioEventRts>,
}

impl PhysicsBehaviorHandle {
    fn new(object: Weak<RwLock<GameObject>>, module_data: Arc<PhysicsBehaviorModuleData>) -> Self {
        Self {
            state: PhysicsBehaviorState::new(module_data.mass),
            object,
            module_data,
            bounce_sound: None,
        }
    }

    fn is_motive(&self) -> bool {
        self.state.motive_force_expires > TheGameLogic::get_frame()
    }

    fn update_pitch_roll_yaw_flag(&mut self) {
        let has = self.state.pitch_rate != 0.0
            || self.state.roll_rate != 0.0
            || self.state.yaw_rate != 0.0;
        self.state.set_flag(FLAG_HAS_PITCHROLLYAW, has);
    }
}

impl PhysicsBehaviorTrait for PhysicsBehaviorHandle {
    fn update(&mut self, _dt: f32) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn get_velocity(&self) -> Vec3 {
        self.state.vel
    }

    fn set_velocity(&mut self, velocity: &Vec3) {
        self.state.vel = *velocity;
        self.state.vel_mag = INVALID_VEL_MAG;
    }

    fn add_velocity_to(&mut self, velocity: &Vec3) {
        self.state.vel += *velocity;
        self.state.vel_mag = INVALID_VEL_MAG;
    }

    fn is_on_ground(&self) -> bool {
        !self.state.has_flag(FLAG_IS_IN_FREEFALL)
    }

    fn apply_force(&mut self, force: &Vec3) {
        if !force.x.is_finite() || !force.y.is_finite() || !force.z.is_finite() {
            return;
        }

        let mut mod_force = *force;
        if self.is_motive() {
            if let Some(obj) = self.object.upgrade() {
                if let Ok(obj) = obj.read() {
                    let (dir_x, dir_y) = obj.get_unit_direction_vector_2d();
                    let lateral_dot = force.x * -dir_y + force.y * dir_x;
                    mod_force.x = lateral_dot * -dir_y;
                    mod_force.y = lateral_dot * dir_x;
                }
            }
        }

        let mass = if self.state.mass.abs() < 0.0001 {
            0.0001
        } else {
            self.state.mass
        };
        let mass_inv = 1.0 / mass;
        self.state.accel.x += mod_force.x * mass_inv;
        self.state.accel.y += mod_force.y * mass_inv;
        self.state.accel.z += mod_force.z * mass_inv;

        if !self.state.has_flag(FLAG_IS_IN_UPDATE) {
            if let Some(obj) = self.object.upgrade() {
                if let Ok(obj) = obj.read() {
                    TheGameLogic::set_wake_frame(obj.get_id(), UPDATE_SLEEP_NONE);
                }
            }
        }
    }

    fn set_yaw_rate(&mut self, rate: Real) {
        self.state.yaw_rate = rate;
        self.update_pitch_roll_yaw_flag();
    }

    fn set_roll_rate(&mut self, rate: Real) {
        self.state.roll_rate = rate;
        self.update_pitch_roll_yaw_flag();
    }

    fn set_pitch_rate(&mut self, rate: Real) {
        self.state.pitch_rate = rate;
        self.update_pitch_roll_yaw_flag();
    }

    fn set_turning(&mut self, turning: i32) {
        self.state.turning = turning;
    }

    fn set_mass(&mut self, mass: Real) {
        self.state.mass = mass;
    }

    fn set_extra_friction(&mut self, friction: Real) {
        self.state.extra_friction = friction;
    }

    fn set_extra_bounciness(&mut self, bounciness: Real) {
        self.state.extra_bounciness = bounciness;
    }

    fn set_allow_bouncing(&mut self, allow: bool) {
        self.state.set_flag(FLAG_ALLOW_BOUNCE, allow);
    }

    fn set_allow_airborne_friction(&mut self, allow: bool) {
        self.state
            .set_flag(FLAG_APPLY_FRICTION2D_WHEN_AIRBORNE, allow);
    }

    fn set_bounce_sound(&mut self, sound: Option<AudioEventRts>) {
        self.bounce_sound = sound;
    }

    fn get_bounce_sound(&self) -> Option<AudioEventRts> {
        self.bounce_sound.clone()
    }

    fn set_ignore_collisions_with(&mut self, obj_id: ObjectID) {
        self.state.ignore_collisions_with = obj_id;
    }

    fn set_angles(&mut self, yaw: Real, pitch: Real, roll: Real) {
        self.state.yaw_angle = yaw;
        self.state.pitch_angle = pitch;
        self.state.roll_angle = roll;
    }

    fn get_mass(&self) -> Real {
        self.state.mass
    }

    fn apply_angular_velocity(&mut self, _angular_velocity: &Vec3) {}

    fn apply_motive_force(&mut self, force: &Vec3) {
        let prev = self.state.motive_force_expires;
        self.state.motive_force_expires = 0;
        self.apply_force(force);
        let now = TheGameLogic::get_frame();
        self.state.motive_force_expires = now.saturating_add(MOTIVE_FRAMES);
        if prev == 0 {
            self.state.motive_force_expires = now.saturating_add(MOTIVE_FRAMES);
        }
    }

    fn get_turning(&self) -> Real {
        self.state.turning as Real
    }

    fn get_last_collidee(&self) -> ObjectID {
        self.state.last_collidee
    }

    fn get_ignore_collisions_with(&self) -> ObjectID {
        self.state.ignore_collisions_with
    }

    fn reset_dynamic_physics(&mut self) {
        self.state.accel = Coord3D::ZERO;
        self.state.prev_accel = Coord3D::ZERO;
        self.state.vel = Coord3D::ZERO;
        self.state.vel_mag = 0.0;
        self.state.turning = 0;
        self.state.yaw_rate = 0.0;
        self.state.roll_rate = 0.0;
        self.state.pitch_rate = 0.0;
        self.update_pitch_roll_yaw_flag();
    }

    fn apply_shock(&mut self, force: &Coord3D) {
        let resistance = self.module_data.shock_resistance.clamp(0.0, 1.0);
        let resisted = *force * (1.0 - resistance);
        self.apply_force(&resisted);
    }

    fn apply_random_rotation(&mut self) {
        if self.state.has_flag(FLAG_STICK_TO_GROUND) {
            return;
        }
        self.set_allow_bouncing(true);

        let random_yaw = crate::helpers::get_game_logic_random_value_real(-1.0, 1.0);
        let random_pitch = crate::helpers::get_game_logic_random_value_real(-1.0, 1.0);
        let random_roll = crate::helpers::get_game_logic_random_value_real(-1.0, 1.0);

        self.state.yaw_rate += self.module_data.shock_max_yaw * random_yaw;
        self.state.pitch_rate += self.module_data.shock_max_pitch * random_pitch;
        self.state.roll_rate += self.module_data.shock_max_roll * random_roll;
        self.update_pitch_roll_yaw_flag();

        if !self.state.has_flag(FLAG_IS_IN_UPDATE) {
            if let Some(obj) = self.object.upgrade() {
                if let Ok(obj) = obj.read() {
                    TheGameLogic::set_wake_frame(obj.get_id(), UPDATE_SLEEP_NONE);
                }
            }
        }
    }

    fn set_stunned(&mut self, stunned: bool) {
        self.state.set_flag(FLAG_IS_STUNNED, stunned);
        if let Some(obj) = self.object.upgrade() {
            if let Ok(mut obj) = obj.write() {
                if stunned {
                    obj.set_model_condition_state(MODELCONDITION_STUNNED_FLAILING);
                } else {
                    obj.clear_model_condition_state(MODELCONDITION_STUNNED);
                    obj.clear_model_condition_state(MODELCONDITION_STUNNED_FLAILING);
                }
            }
        }
    }

    fn set_allow_to_fall(&mut self, allow: bool) {
        self.state.set_flag(FLAG_ALLOW_TO_FALL, allow);
    }

    fn clear_acceleration(&mut self) {
        self.state.accel = Coord3D::ZERO;
        self.state.prev_accel = Coord3D::ZERO;
    }
}

const MOTIVE_FRAMES: UnsignedInt = (LOGICFRAMES_PER_SECOND / 3) as UnsignedInt;

pub struct PhysicsBehaviorUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<PhysicsBehaviorModuleData>,
    physics_handle: Arc<Mutex<PhysicsBehaviorHandle>>,
}

impl PhysicsBehaviorUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let data = module_data
            .as_ref()
            .as_any()
            .downcast_ref::<PhysicsBehaviorModuleData>()
            .ok_or("Invalid module data for PhysicsBehavior")?;

        let module_data = Arc::new(data.clone());
        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: module_data.clone(),
            physics_handle: Arc::new(Mutex::new(PhysicsBehaviorHandle::new(
                Arc::downgrade(&object),
                module_data,
            ))),
        })
    }

    fn apply_gravity(&self, state: &mut PhysicsBehaviorState) {
        let gravity = global_data::read_safe()
            .map(|data| data.gravity)
            .unwrap_or(-1.0);
        state.accel.z += gravity;
    }

    fn clamp_friction(value: Real, min: Real) -> Real {
        value.clamp(min, MAX_FRICTION)
    }

    fn get_aerodynamic_friction(&self, state: &PhysicsBehaviorState) -> Real {
        Self::clamp_friction(
            self.module_data.aerodynamic_friction + state.extra_friction,
            MIN_AERO_FRICTION,
        )
    }

    fn get_forward_friction(&self, state: &PhysicsBehaviorState) -> Real {
        Self::clamp_friction(
            self.module_data.forward_friction + state.extra_friction,
            MIN_NON_AERO_FRICTION,
        )
    }

    fn get_lateral_friction(&self, state: &PhysicsBehaviorState) -> Real {
        Self::clamp_friction(
            self.module_data.lateral_friction + state.extra_friction,
            MIN_NON_AERO_FRICTION,
        )
    }

    #[allow(dead_code)]
    fn get_z_friction(&self, state: &PhysicsBehaviorState) -> Real {
        Self::clamp_friction(
            self.module_data.z_friction + state.extra_friction,
            MIN_NON_AERO_FRICTION,
        )
    }

    fn apply_ypr_damping(state: &mut PhysicsBehaviorState, factor: Real) {
        state.pitch_rate *= factor;
        state.roll_rate *= factor;
        state.yaw_rate *= factor;
        let has = state.pitch_rate != 0.0 || state.roll_rate != 0.0 || state.yaw_rate != 0.0;
        state.set_flag(FLAG_HAS_PITCHROLLYAW, has);
    }

    fn apply_frictional_forces(&self, obj: &GameObject, state: &mut PhysicsBehaviorState) {
        let apply_ground = state.has_flag(FLAG_APPLY_FRICTION2D_WHEN_AIRBORNE)
            || !obj.is_significantly_above_terrain();
        if apply_ground {
            Self::apply_ypr_damping(state, 1.0 - DEFAULT_LATERAL_FRICTION);

            if state.vel.x != 0.0 || state.vel.y != 0.0 {
                let (dir_x, dir_y) = obj.get_unit_direction_vector_2d();
                let mass = state.mass;

                let lateral_dot = state.vel.x * -dir_y + state.vel.y * dir_x;
                let lateral_vel_x = lateral_dot * -dir_y;
                let lateral_vel_y = lateral_dot * dir_x;

                let lf = mass * self.get_lateral_friction(state);
                let mut force = Coord3D::new(-(lf * lateral_vel_x), -(lf * lateral_vel_y), 0.0);

                if state.motive_force_expires <= TheGameLogic::get_frame() {
                    let forward_dot = state.vel.x * dir_x + state.vel.y * dir_y;
                    let forward_vel_x = forward_dot * dir_x;
                    let forward_vel_y = forward_dot * dir_y;
                    let ff = mass * self.get_forward_friction(state);
                    force.x += -(ff * forward_vel_x);
                    force.y += -(ff * forward_vel_y);
                }
                let mass_inv = if mass.abs() < 0.0001 { 0.0 } else { 1.0 / mass };
                state.accel += force * mass_inv;
            }
        } else {
            let aero = -self.get_aerodynamic_friction(state);
            state.accel.x += state.vel.x * aero;
            state.accel.y += state.vel.y * aero;
            state.accel.z += state.vel.z * aero;
            Self::apply_ypr_damping(state, 1.0 + aero);
        }
    }

    fn handle_bounce(
        &self,
        state: &mut PhysicsBehaviorState,
        old_z: Real,
        new_z: Real,
        ground_z: Real,
    ) -> Option<Coord3D> {
        if state.has_flag(FLAG_ALLOW_BOUNCE) && new_z <= ground_z {
            let stiffness = global_data::read_safe()
                .map(|data| data.ground_stiffness)
                .unwrap_or(0.8)
                .clamp(0.01, 0.99);

            let mut desired_accel_z = 0.0;
            let vz = state.vel.z;
            if old_z > ground_z && vz < 0.0 {
                desired_accel_z = vz.abs() * stiffness;
            }

            let bounce_force = Coord3D::new(0.0, 0.0, state.mass * desired_accel_z);
            Self::apply_ypr_damping(state, 0.7);

            if bounce_force.z > 0.0 {
                return Some(bounce_force);
            }

            state.set_flag(FLAG_ALLOW_BOUNCE, state.original_allow_bounce);
            return None;
        }
        None
    }

    fn is_very_small3d(vec: Coord3D) -> bool {
        let thresh = 0.01;
        vec.x.abs() < thresh && vec.y.abs() < thresh && vec.z.abs() < thresh
    }

    fn is_zero3d(vec: Coord3D) -> bool {
        vec.x == 0.0 && vec.y == 0.0 && vec.z == 0.0
    }

    fn calc_sleep_time(&self, state: &PhysicsBehaviorState, obj: &GameObject) -> UpdateSleepTime {
        if Self::is_zero3d(state.vel)
            && Self::is_zero3d(state.accel)
            && !state.has_flag(FLAG_HAS_PITCHROLLYAW)
            && state.motive_force_expires <= TheGameLogic::get_frame()
            && obj.get_layer() == crate::common::PathfindLayerEnum::Ground
            && !obj.is_above_terrain()
            && state.current_overlap == crate::common::INVALID_ID
            && state.previous_overlap == crate::common::INVALID_ID
            && state.has_flag(FLAG_UPDATE_EVER_RUN)
        {
            UPDATE_SLEEP_FOREVER
        } else {
            UPDATE_SLEEP_NONE
        }
    }
}

impl UpdateModuleInterface for PhysicsBehaviorUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        let Some(obj_arc) = self.object.upgrade() else {
            return UpdateSleepTime::None;
        };
        let Ok(mut obj) = obj_arc.write() else {
            return UpdateSleepTime::None;
        };
        let Ok(mut handle) = self.physics_handle.lock() else {
            return UpdateSleepTime::None;
        };
        let state = &mut handle.state;
        let airborne_at_start = obj.is_above_terrain();

        if !state.has_flag(FLAG_UPDATE_EVER_RUN) {
            state.set_flag(FLAG_WAS_AIRBORNE_LAST_FRAME, airborne_at_start);
        }

        state.set_flag(FLAG_IS_IN_UPDATE, true);
        state.prev_accel = state.accel;
        let prev_pos = *obj.get_position();

        let mut active_vel_z = 0.0;
        let mut ground_z = 0.0;
        let mut got_ground = false;
        let mut bounce_force: Option<Coord3D> = None;

        if !obj.is_disabled_by_type(DisabledType::Held) {
            self.apply_gravity(state);
            self.apply_frictional_forces(&obj, state);

            state.vel += state.accel;

            let thresh = 0.001;
            if state.vel.x.abs() < thresh {
                state.vel.x = 0.0;
            }
            if state.vel.y.abs() < thresh {
                state.vel.y = 0.0;
            }
            if state.vel.z.abs() < thresh {
                state.vel.z = 0.0;
            }

            state.vel_mag = INVALID_VEL_MAG;

            let mut pos = prev_pos;
            let old_pos_z = pos.z;

            if obj.test_status(ObjectStatusTypes::Braking) && !obj.is_kind_of(KindOf::Projectile) {
                pos.z += state.vel.z;
            } else {
                pos += state.vel;
            }

            if let Some(terrain) = TheTerrainLogic::get() {
                ground_z = terrain.get_layer_height(pos.x, pos.y, obj.get_layer());
                if obj.test_status(ObjectStatusTypes::DeckHeightOffset) {
                    ground_z += obj.get_carrier_deck_height();
                }
                got_ground = true;
            }

            bounce_force = self.handle_bounce(state, old_pos_z, pos.z, ground_z);
            active_vel_z = state.vel.z;

            if state.has_flag(FLAG_IS_STUNNED) {
                if (state.vel.x.abs() < STUN_RELIEF_EPSILON
                    && state.vel.y.abs() < STUN_RELIEF_EPSILON
                    && state.vel.z.abs() < STUN_RELIEF_EPSILON)
                    || !obj.is_significantly_above_terrain()
                {
                    state.set_flag(FLAG_IS_STUNNED, false);
                    obj.clear_model_condition_state(MODELCONDITION_STUNNED);
                }
            }

            if pos.z <= ground_z {
                let dz = ground_z - pos.z;
                state.vel.z += dz;
                if state.vel.z > 0.0 {
                    state.vel.z = 0.0;
                }
                state.vel_mag = INVALID_VEL_MAG;
                pos.z = ground_z;
                state.set_flag(FLAG_ALLOW_TO_FALL, false);
                if state.has_flag(FLAG_IS_STUNNED) {
                    obj.clear_model_condition_state(MODELCONDITION_STUNNED_FLAILING);
                    obj.set_model_condition_state(MODELCONDITION_STUNNED);
                }
            } else if pos.z > ground_z {
                if state.has_flag(FLAG_IS_IN_FREEFALL) {
                    obj.set_disabled(DisabledType::DisabledFreefall);
                    obj.set_model_condition_state(MODELCONDITION_FREEFALL);
                } else if state.has_flag(FLAG_STICK_TO_GROUND)
                    && !state.has_flag(FLAG_ALLOW_TO_FALL)
                {
                    pos.z = ground_z;
                }
            }

            if state.has_flag(FLAG_HAS_PITCHROLLYAW) {
                let yaw_rate = state.yaw_rate * self.module_data.pitch_roll_yaw_factor;
                let mut pitch_rate = state.pitch_rate * self.module_data.pitch_roll_yaw_factor;
                let roll_rate = state.roll_rate * self.module_data.pitch_roll_yaw_factor;

                let offset = self.module_data.center_of_mass_offset;
                if offset != 0.0 {
                    let remaining_angle = if offset > 0.0 {
                        (crate::common::PI / 2.0) - state.pitch_angle
                    } else {
                        (-crate::common::PI / 2.0) + state.pitch_angle
                    };
                    pitch_rate *= remaining_angle.sin();
                }

                state.yaw_angle += yaw_rate;
                state.pitch_angle += pitch_rate;
                state.roll_angle += roll_rate;
            } else {
                state.yaw_angle = obj.get_orientation();
            }

            if bounce_force.is_some() {
                state.pitch_angle = 0.0;
                state.roll_angle = 0.0;
            }

            let rotation = Quat::from_euler(
                glam::EulerRot::XYZ,
                state.roll_angle,
                state.pitch_angle,
                state.yaw_angle,
            );
            let matrix = Mat4::from_translation(pos) * Mat4::from_quat(rotation);
            obj.set_transform_matrix(&matrix);
        }

        state.accel = Coord3D::ZERO;
        state.previous_overlap = state.current_overlap;
        state.current_overlap = crate::common::INVALID_ID;
        let allow_bounce = state.has_flag(FLAG_ALLOW_BOUNCE);

        if let Some(force) = bounce_force {
            if allow_bounce {
                handle.apply_force(&force);
            }
        }
        let state = &mut handle.state;

        let airborne_at_end = obj.is_above_terrain();

        if state.has_flag(FLAG_WAS_AIRBORNE_LAST_FRAME)
            && !airborne_at_end
            && !state.has_flag(FLAG_IMMUNE_TO_FALLING_DAMAGE)
        {
            let normal = Coord3D::new(0.0, 0.0, -1.0);
            let collision_pos = *obj.get_position();
            obj.on_collide(None, &collision_pos, &normal);

            let net_speed = -active_vel_z - self.module_data.min_fall_speed_for_damage;
            if net_speed > 0.0 && !obj.is_kind_of(KindOf::Projectile) {
                if (state.vel.x.abs() <= TINY_DELTA
                    || (active_vel_z / state.vel.x).abs() >= MIN_ANGLE_TAN)
                    && (state.vel.y.abs() <= TINY_DELTA
                        || (active_vel_z / state.vel.y).abs() >= MIN_ANGLE_TAN)
                {
                    let damage_amount =
                        net_speed * state.mass * self.module_data.fall_height_damage_factor;
                    let mut damage = DamageInfo::with_simple(
                        damage_amount,
                        obj.get_id(),
                        DamageType::Falling,
                        DeathType::Splatted,
                    );
                    damage.input.shock_wave_amount = 0.0;
                    damage.input.shock_wave_radius = 0.0;
                    damage.sync_from_input();
                    let _ = obj.attempt_damage(&mut damage);
                    if obj.is_effectively_dead() {
                        obj.set_model_condition_state(MODELCONDITION_SPLATTED);
                    }
                }
            }
        }

        if !airborne_at_end {
            state.set_flag(FLAG_IS_IN_FREEFALL, false);
            obj.clear_disabled(DisabledType::DisabledFreefall);
            obj.clear_model_condition_state(MODELCONDITION_FREEFALL);
        }

        if self.module_data.kill_when_resting_on_ground
            && !airborne_at_end
            && Self::is_very_small3d(state.vel)
        {
            if !obj.is_kind_of(KindOf::Drone)
                || obj.is_effectively_dead()
                || obj.is_disabled_by_type(DisabledType::DisabledUnmanned)
            {
                obj.kill(None, None);
            }
        }

        if got_ground {
            let height_above = obj.get_position().z - ground_z;
            obj.set_height_above_terrain(height_above);
        }

        state.set_flag(FLAG_UPDATE_EVER_RUN, true);
        state.set_flag(FLAG_WAS_AIRBORNE_LAST_FRAME, airborne_at_end);
        state.set_flag(FLAG_IS_IN_UPDATE, false);

        self.calc_sleep_time(state, &obj)
    }

    fn get_update_phase(&self) -> SleepyUpdatePhase {
        SleepyUpdatePhase::Physics
    }

    fn get_disabled_types_to_process(&self) -> crate::common::DisabledMaskType {
        crate::common::DisabledMaskType::all()
    }
}

impl BehaviorModuleInterface for PhysicsBehaviorUpdate {
    fn get_module_name(&self) -> &'static str {
        "PhysicsBehavior"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_collide(&mut self) -> Option<&mut dyn CollideModuleInterface> {
        Some(self)
    }

    fn on_object_created(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(obj_arc) = self.object.upgrade() else {
            return Ok(());
        };
        let Ok(mut obj) = obj_arc.write() else {
            return Ok(());
        };
        if let Ok(mut handle) = self.physics_handle.lock() {
            handle.state.mass = self.module_data.mass;
            handle.state.original_allow_bounce = self.module_data.allow_bouncing;
            handle
                .state
                .set_flag(FLAG_ALLOW_BOUNCE, self.module_data.allow_bouncing);
            handle.state.yaw_angle = obj.get_orientation();
        }
        obj.set_physics(Some(self.physics_handle.clone()));

        let sleep = if self.module_data.mass <= 0.0 {
            UpdateSleepTime::Forever
        } else {
            UpdateSleepTime::None
        };
        TheGameLogic::set_wake_frame(obj.get_id(), sleep);
        Ok(())
    }
}

impl CollideModuleInterface for PhysicsBehaviorUpdate {
    fn on_collision(&mut self, _object_id: ObjectID, other_id: ObjectID) {
        if let Ok(mut handle) = self.physics_handle.lock() {
            handle.state.last_collidee = other_id;
        }
    }
}

impl Snapshotable for PhysicsBehaviorUpdate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|e| format!("PhysicsBehavior xfer version failed: {:?}", e))?;

        let mut handle = self.physics_handle.lock().map_err(|_| "Lock failed")?;
        xfer.xfer_real(&mut handle.state.yaw_rate)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut handle.state.roll_rate)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut handle.state.pitch_rate)
            .map_err(|e| e.to_string())?;
        xfer.xfer_coord3d(&mut handle.state.accel);
        xfer.xfer_coord3d(&mut handle.state.prev_accel);
        xfer.xfer_coord3d(&mut handle.state.vel);

        if version < 2 {
            let mut tmp = Coord3D::ZERO;
            xfer.xfer_coord3d(&mut tmp);
        }

        xfer.xfer_int(&mut handle.state.turning)
            .map_err(|e| e.to_string())?;
        xfer.xfer_object_id(&mut handle.state.ignore_collisions_with)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut handle.state.flags)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut handle.state.mass)
            .map_err(|e| e.to_string())?;
        xfer.xfer_object_id(&mut handle.state.current_overlap)
            .map_err(|e| e.to_string())?;
        xfer.xfer_object_id(&mut handle.state.previous_overlap)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut handle.state.motive_force_expires)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut handle.state.extra_bounciness)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut handle.state.extra_friction)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut handle.state.vel_mag)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct PhysicsBehaviorFactory;

impl PhysicsBehaviorFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(PhysicsBehaviorUpdate::new(thing, module_data)?))
    }
}

fn height_to_speed(height: Real) -> Real {
    let gravity = global_data::read_safe()
        .map(|data| data.gravity)
        .unwrap_or(-1.0);
    (2.0 * gravity.abs() * height).sqrt()
}

fn parse_height_to_speed(
    _ini: &mut INI,
    data: &mut PhysicsBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    let height: Real = tokens[0].parse().map_err(|_| INIError::InvalidData)?;
    data.min_fall_speed_for_damage = height_to_speed(height);
    Ok(())
}

fn parse_friction_per_sec(
    _ini: &mut INI,
    target: &mut Real,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    let fric_per_sec: Real = tokens[0].parse().map_err(|_| INIError::InvalidData)?;
    *target = fric_per_sec * SECONDS_PER_LOGICFRAME_REAL;
    Ok(())
}

fn parse_real_field(_ini: &mut INI, target: &mut Real, tokens: &[&str]) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    *target = tokens[0].parse().map_err(|_| INIError::InvalidData)?;
    Ok(())
}

fn parse_positive_non_zero_real(
    _ini: &mut INI,
    target: &mut Real,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    let value: Real = tokens[0].parse().map_err(|_| INIError::InvalidData)?;
    if value <= 0.0 {
        return Err(INIError::InvalidData);
    }
    *target = value;
    Ok(())
}

fn parse_bool_field(_ini: &mut INI, target: &mut bool, tokens: &[&str]) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    let token = tokens[0].to_ascii_lowercase();
    *target = token == "true" || token == "yes" || token == "1";
    Ok(())
}

fn parse_weapon_template_name(
    _ini: &mut INI,
    target: &mut AsciiString,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    *target = AsciiString::from(tokens[0]);
    Ok(())
}

const PHYSICS_BEHAVIOR_FIELDS: &[FieldParse<PhysicsBehaviorModuleData>] = &[
    FieldParse {
        token: "Mass",
        parse: |ini, data, tokens| parse_positive_non_zero_real(ini, &mut data.mass, tokens),
    },
    FieldParse {
        token: "ShockResistance",
        parse: |ini, data, tokens| {
            parse_positive_non_zero_real(ini, &mut data.shock_resistance, tokens)
        },
    },
    FieldParse {
        token: "ShockMaxYaw",
        parse: |ini, data, tokens| {
            parse_positive_non_zero_real(ini, &mut data.shock_max_yaw, tokens)
        },
    },
    FieldParse {
        token: "ShockMaxPitch",
        parse: |ini, data, tokens| {
            parse_positive_non_zero_real(ini, &mut data.shock_max_pitch, tokens)
        },
    },
    FieldParse {
        token: "ShockMaxRoll",
        parse: |ini, data, tokens| {
            parse_positive_non_zero_real(ini, &mut data.shock_max_roll, tokens)
        },
    },
    FieldParse {
        token: "ForwardFriction",
        parse: |ini, data, tokens| parse_friction_per_sec(ini, &mut data.forward_friction, tokens),
    },
    FieldParse {
        token: "LateralFriction",
        parse: |ini, data, tokens| parse_friction_per_sec(ini, &mut data.lateral_friction, tokens),
    },
    FieldParse {
        token: "ZFriction",
        parse: |ini, data, tokens| parse_friction_per_sec(ini, &mut data.z_friction, tokens),
    },
    FieldParse {
        token: "AerodynamicFriction",
        parse: |ini, data, tokens| {
            parse_friction_per_sec(ini, &mut data.aerodynamic_friction, tokens)
        },
    },
    FieldParse {
        token: "CenterOfMassOffset",
        parse: |ini, data, tokens| parse_real_field(ini, &mut data.center_of_mass_offset, tokens),
    },
    FieldParse {
        token: "AllowBouncing",
        parse: |ini, data, tokens| parse_bool_field(ini, &mut data.allow_bouncing, tokens),
    },
    FieldParse {
        token: "AllowCollideForce",
        parse: |ini, data, tokens| parse_bool_field(ini, &mut data.allow_collide_force, tokens),
    },
    FieldParse {
        token: "KillWhenRestingOnGround",
        parse: |ini, data, tokens| {
            parse_bool_field(ini, &mut data.kill_when_resting_on_ground, tokens)
        },
    },
    FieldParse {
        token: "MinFallHeightForDamage",
        parse: parse_height_to_speed,
    },
    FieldParse {
        token: "FallHeightDamageFactor",
        parse: |ini, data, tokens| {
            parse_real_field(ini, &mut data.fall_height_damage_factor, tokens)
        },
    },
    FieldParse {
        token: "PitchRollYawFactor",
        parse: |ini, data, tokens| parse_real_field(ini, &mut data.pitch_roll_yaw_factor, tokens),
    },
    FieldParse {
        token: "VehicleCrashesIntoBuildingWeaponTemplate",
        parse: |ini, data, tokens| {
            parse_weapon_template_name(
                ini,
                &mut data.vehicle_crashes_into_building_weapon_template,
                tokens,
            )
        },
    },
    FieldParse {
        token: "VehicleCrashesIntoNonBuildingWeaponTemplate",
        parse: |ini, data, tokens| {
            parse_weapon_template_name(
                ini,
                &mut data.vehicle_crashes_into_non_building_weapon_template,
                tokens,
            )
        },
    },
];
