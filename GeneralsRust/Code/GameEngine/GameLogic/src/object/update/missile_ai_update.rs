//! Missile AI Update Module
//!
//! Port of C++ MissileAIUpdate from:
//! GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate/MissileAIUpdate.cpp
//!
//! Implements smart missile behavior including tracking, homing, fuel management,
//! countermeasure resistance, and multi-stage flight patterns.

use crate::common::{
    Bool, Coord3D, Matrix3D, ModuleData, ObjectID, ObjectStatusMaskType, Real, UnsignedInt,
    INVALID_ID, MODELCONDITION_JAMMED,
};
use crate::damage::{DamageInfo, DamageInfoInput, DamageType, DeathType};
use crate::effects::FXList;
use crate::helpers::{get_game_logic_random_value_real, TheGameLogic, TheTerrainLogic};
use crate::modules::{
    AIUpdateInterfaceExt, BehaviorModuleInterface, PhysicsBehaviorExt, ProjectileUpdateInterface,
    UpdateModuleInterface, UpdateSleepTime, UPDATE_SLEEP_NONE,
};
use crate::object::behavior::behavior_module::BehaviorModuleData;
use crate::object::Object;
use crate::player::CMD_FROM_AI;
use crate::weapon::{WeaponSlotType, WeaponTemplate};
use crate::GameLogicResult;
use game_engine::common::ini::ini_particle_sys::ParticleSystemTemplate;
use game_engine::common::system::{Snapshotable, Xfer};
use glam::Vec4;
use std::sync::{Arc, Weak};

const BIGNUM: Real = 99999.0;
const APPROACH_HEIGHT: Real = 10.0;

/// Missile state machine states
/// Matches C++ MissileStateType from MissileAIUpdate.h
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissileState {
    /// Pre-launch state (waiting to be fired)
    PreLaunch,
    /// Launch delay before ignition
    Launch,
    /// Ignition moment (engines start, trail begins)
    Ignition,
    /// Attack mode without turning (initial straight flight)
    AttackNoTurn,
    /// Attack mode with full turning capability
    Attack,
    /// Final approach mode (precise terminal guidance)
    Kill,
    /// Self-destruct delay before removal
    KillSelf,
    /// Dead state (no longer active)
    Dead,
}

/// Missile AI Update module data (from INI)
/// Matches C++ MissileAIUpdateModuleData from MissileAIUpdate.cpp lines 42-59
#[derive(Debug, Clone)]
pub struct MissileAIUpdateModuleData {
    pub base: BehaviorModuleData,
    /// Whether missile attempts to follow moving targets
    pub try_to_follow_target: bool,

    /// Fuel lifetime in frames (0 = unlimited)
    pub fuel_lifetime: UnsignedInt,

    /// Delay before ignition in frames
    pub ignition_delay: UnsignedInt,

    /// Initial velocity when fired
    pub initial_velocity: Real,

    /// Distance to travel before turning is allowed
    pub initial_distance: Real,

    /// Distance to target before diving/final approach
    pub dive_distance: Real,

    /// Distance to target for lock-on behavior change
    pub lock_distance: Real,

    /// Scatter distance when jammed by ECM
    pub distance_scatter_when_jammed: Real,

    /// Particle effects on ignition
    pub ignition_fx: Option<Arc<FXList>>,

    /// Use weapon speed instead of initial velocity
    pub use_weapon_speed: bool,

    /// Detonate when fuel runs out
    pub detonate_on_no_fuel: bool,

    /// Kill garrison count for special warheads
    pub garrison_hit_kill_count: UnsignedInt,

    /// FX for garrison kills
    pub garrison_hit_kill_fx: Option<Arc<FXList>>,

    /// Whether detonation calls kill() instead of destroy()
    pub detonate_calls_kill: bool,

    /// Frames to delay before self-destruction after detonation
    pub kill_self_delay: UnsignedInt,
}

impl Default for MissileAIUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            try_to_follow_target: true,
            fuel_lifetime: 0,
            ignition_delay: 0,
            initial_velocity: 0.0,
            initial_distance: 0.0,
            dive_distance: 0.0,
            lock_distance: 75.0,
            distance_scatter_when_jammed: 75.0,
            ignition_fx: None,
            use_weapon_speed: false,
            detonate_on_no_fuel: false,
            garrison_hit_kill_count: 0,
            garrison_hit_kill_fx: None,
            detonate_calls_kill: false,
            kill_self_delay: 3, // Long enough for contrail to catch up
        }
    }
}

crate::impl_behavior_module_data_via_base!(MissileAIUpdateModuleData, base);

/// Missile AI Update Module
/// Matches C++ MissileAIUpdate class from MissileAIUpdate.cpp lines 97-127
#[derive(Debug)]
pub struct MissileAIUpdate {
    /// Module configuration data
    data: Arc<MissileAIUpdateModuleData>,

    /// Owning projectile object id (for detonation effects)
    object_id: ObjectID,

    /// Current state in missile state machine
    state: MissileState,

    /// Frame when state was entered
    state_timestamp: UnsignedInt,

    /// Next frame to recalculate target position
    next_target_track_time: UnsignedInt,

    /// ID of object that launched this missile
    launcher_id: ObjectID,

    /// ID of target object (if targeting object)
    victim_id: ObjectID,

    /// Whether warhead is armed (can detonate)
    is_armed: bool,

    /// Frame when fuel expires
    fuel_expiration_date: UnsignedInt,

    /// Distance remaining before turning is allowed
    no_turn_dist_left: Real,

    /// Previous position (for distance tracking)
    prev_pos: Coord3D,

    /// Maximum acceleration
    max_accel: Real,

    /// Weapon template for detonation
    detonation_weapon_tmpl: Option<Weak<WeaponTemplate>>,

    /// Exhaust particle system template
    exhaust_sys_tmpl: Option<Arc<ParticleSystemTemplate>>,

    /// Whether missile is tracking a moving target
    is_tracking_target: bool,

    /// Exhaust particle system ID
    exhaust_id: UnsignedInt, // Would be ParticleSystemID

    /// Extra weapon bonus flags from launcher
    extra_bonus_flags: crate::common::types::WeaponBonusConditionFlags,

    /// Original target position (for fallback)
    original_target_pos: Coord3D,

    /// Frames until countermeasure diversion occurs
    frames_till_decoyed: UnsignedInt,

    /// Whether missile does no damage (decoy mode)
    no_damage: bool,

    /// Whether missile has been jammed
    is_jammed: bool,
}

impl MissileAIUpdate {
    /// Create a new missile AI update module
    /// Matches C++ MissileAIUpdate::MissileAIUpdate from MissileAIUpdate.cpp lines 97-120
    pub fn new(data: Arc<MissileAIUpdateModuleData>, current_frame: UnsignedInt) -> Self {
        Self {
            data: data.clone(),
            object_id: INVALID_ID,
            state: MissileState::PreLaunch,
            state_timestamp: current_frame,
            next_target_track_time: UnsignedInt::MAX, // Never recalc by default
            launcher_id: INVALID_ID,
            victim_id: INVALID_ID,
            is_armed: false,
            fuel_expiration_date: 0,
            no_turn_dist_left: data.initial_distance,
            prev_pos: Coord3D::new(0.0, 0.0, 0.0),
            max_accel: BIGNUM,
            detonation_weapon_tmpl: None,
            exhaust_sys_tmpl: None,
            is_tracking_target: false,
            exhaust_id: INVALID_ID,
            extra_bonus_flags: crate::common::types::WeaponBonusConditionFlags::none(),
            original_target_pos: Coord3D::new(0.0, 0.0, 0.0),
            frames_till_decoyed: 0,
            no_damage: false,
            is_jammed: false,
        }
    }

    /// Switch to a new state
    /// Matches C++ MissileAIUpdate::switchToState from MissileAIUpdate.cpp lines 152-159
    fn switch_to_state(&mut self, new_state: MissileState, current_frame: UnsignedInt) {
        if self.state != new_state {
            self.state = new_state;
            self.state_timestamp = current_frame;
        }
    }

    /// Launch missile at object or position
    /// Matches C++ MissileAIUpdate::projectileLaunchAtObjectOrPosition
    /// from MissileAIUpdate.cpp lines 164-183
    pub fn projectile_launch_at_object_or_position(
        &mut self,
        victim: Option<ObjectID>,
        victim_pos: &Coord3D,
        launch_pos: &Coord3D,
        launcher: Option<ObjectID>,
        detonation_weapon: Option<Weak<WeaponTemplate>>,
        exhaust_sys_override: Option<Arc<ParticleSystemTemplate>>,
    ) {
        self.launcher_id = launcher.unwrap_or(INVALID_ID);
        self.detonation_weapon_tmpl = detonation_weapon;

        // Position projectile for launch would happen here via Weapon::positionProjectileForLaunch

        self.projectile_fire_at_object_or_position(
            victim,
            victim_pos,
            launch_pos,
            exhaust_sys_override,
        );
    }

    /// Fire the missile (actual launch)
    /// Matches C++ MissileAIUpdate::projectileFireAtObjectOrPosition
    /// from MissileAIUpdate.cpp lines 191-275
    fn projectile_fire_at_object_or_position(
        &mut self,
        victim: Option<ObjectID>,
        victim_pos: &Coord3D,
        launch_pos: &Coord3D,
        exhaust_sys_override: Option<Arc<ParticleSystemTemplate>>,
    ) {
        self.exhaust_sys_tmpl = exhaust_sys_override;

        // Determine initial velocity (C++ uses detWeap->getWeaponSpeed when enabled).
        let mut initial_vel_to_use = self.data.initial_velocity;
        if self.data.use_weapon_speed {
            if let Some(weapon) = self
                .detonation_weapon_tmpl
                .as_ref()
                .and_then(|weak| weak.upgrade())
            {
                initial_vel_to_use = weapon.get_projectile_speed();
            }
        }
        if initial_vel_to_use > 0.0 {
            self.max_accel = initial_vel_to_use;
        }

        // Calculate launch direction with Z boost for upward trajectory
        // Matches C++ lines 213-227
        let delta_z = victim_pos.z - launch_pos.z;
        let dx = victim_pos.x - launch_pos.x;
        let dy = victim_pos.y - launch_pos.y;
        let xy_dist = (dx * dx + dy * dy).sqrt().max(1.0);
        let _z_factor = if delta_z > 0.0 {
            delta_z / xy_dist
        } else {
            0.0
        };

        // Initial physics application would happen here
        // Force = mass * velocity, applied along direction vector with Z boost

        self.switch_to_state(MissileState::Launch, TheGameLogic::get_frame());
        self.is_tracking_target = false;

        // Set up target tracking
        if let Some(victim_id) = victim {
            if self.data.try_to_follow_target {
                // aiMoveToObject would be called here
                self.original_target_pos = *victim_pos;
                self.is_tracking_target = true;
                self.victim_id = victim_id;
            }
        } else {
            // Position-only target
            self.original_target_pos = *victim_pos;
            // aiMoveToPosition would be called here
            self.victim_id = INVALID_ID;
        }

        self.prev_pos = *launch_pos;
    }

    /// Handle collision with object or terrain
    /// Matches C++ MissileAIUpdate::projectileHandleCollision
    /// from MissileAIUpdate.cpp lines 279-361
    pub fn projectile_handle_collision(&mut self, other: Option<ObjectID>) -> bool {
        // Check if warhead is armed
        if !self.is_armed {
            return true; // Inert, no collision response
        }

        // Check if hit ground unexpectedly
        if other.is_none() {
            // Ground collision logic
            // Would check if significantly above target
            // Matches C++ lines 288-303
        }

        // Check if should collide with this specific object
        if let Some(_other_id) = other {
            // Would call: m_detonationWeaponTmpl->shouldProjectileCollideWith(...)
            // Matches C++ lines 305-314

            // Special garrison kill logic
            // Matches C++ lines 316-352
            if self.data.garrison_hit_kill_count > 0 {
                // Would check if target is garrisonable building
                // Kill units inside if criteria met
            }
        }

        // Detonate on collision
        self.detonate();

        true
    }

    /// Detonate the missile
    /// Matches C++ MissileAIUpdate::detonate from MissileAIUpdate.cpp lines 364-400
    fn detonate(&mut self) {
        let Some(obj_arc) = TheGameLogic::find_object_by_id(self.object_id) else {
            self.switch_to_state(MissileState::KillSelf, TheGameLogic::get_frame());
            return;
        };
        let Ok(mut obj_guard) = obj_arc.write() else {
            self.switch_to_state(MissileState::KillSelf, TheGameLogic::get_frame());
            return;
        };

        let obj_pos = *obj_guard.get_position();

        if let Some(weapon) = self
            .detonation_weapon_tmpl
            .as_ref()
            .and_then(|weak| weak.upgrade())
        {
            let _ = crate::weapon::with_weapon_store(|store| {
                let _ = store.handle_projectile_detonation(
                    &weapon,
                    self.object_id,
                    &obj_pos,
                    self.extra_bonus_flags,
                    !self.no_damage,
                );
            });

            if weapon.die_on_detonate {
                let max_health = obj_guard.get_max_health();
                let mut damage_info = DamageInfo {
                    input: DamageInfoInput {
                        damage_type: DamageType::Unresistable,
                        death_type: DeathType::Detonated,
                        source_id: INVALID_ID,
                        amount: max_health,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                let _ = obj_guard.attempt_damage(&mut damage_info);
            }
        } else if !self.no_damage {
            let max_health = obj_guard.get_max_health();
            let mut damage_info = DamageInfo {
                input: DamageInfoInput {
                    damage_type: DamageType::Unresistable,
                    death_type: DeathType::Detonated,
                    source_id: INVALID_ID,
                    amount: max_health,
                    ..Default::default()
                },
                ..Default::default()
            };
            let _ = obj_guard.attempt_damage(&mut damage_info);
        }

        if let Some(drawable) = obj_guard.get_drawable() {
            if let Ok(mut draw_guard) = drawable.write() {
                let _ = draw_guard.set_drawable_hidden(true);
            }
        }

        obj_guard.set_status(ObjectStatusMaskType::MISSILE_KILLING_SELF, true);

        self.switch_to_state(MissileState::KillSelf, TheGameLogic::get_frame());
    }

    /// Update missile state machine (per-frame update)
    /// Matches C++ MissileAIUpdate::update from MissileAIUpdate.cpp lines 628-745
    pub fn update(
        &mut self,
        current_frame: UnsignedInt,
        current_pos: Coord3D,
    ) -> GameLogicResult<()> {
        // Update no-turn distance tracking
        if self.no_turn_dist_left > 0.0
            && matches!(
                self.state,
                MissileState::Ignition | MissileState::AttackNoTurn | MissileState::Attack
            )
        {
            let dist_this_turn = ((current_pos.x - self.prev_pos.x).powi(2)
                + (current_pos.y - self.prev_pos.y).powi(2)
                + (current_pos.z - self.prev_pos.z).powi(2))
            .sqrt();
            self.no_turn_dist_left -= dist_this_turn;
            self.prev_pos = current_pos;
        }

        // Handle countermeasure diversion
        if self.frames_till_decoyed > 0 && self.frames_till_decoyed <= current_frame {
            self.handle_countermeasure_diversion();
        }

        // Check if missile fell through world
        if current_pos.z < 0.0 {
            let _ = TheGameLogic::destroy_object_by_id(self.object_id);
            return Ok(());
        }

        // State machine
        match self.state {
            MissileState::PreLaunch => self.do_prelaunch_state(),
            MissileState::Launch => {
                self.do_launch_state(current_frame);
                // Special case: may transition to Ignition immediately
                if self.state == MissileState::Ignition {
                    self.do_ignition_state(current_frame);
                }
            }
            MissileState::Ignition => self.do_ignition_state(current_frame),
            MissileState::AttackNoTurn => self.do_attack_state(false, current_frame),
            MissileState::Attack => self.do_attack_state(true, current_frame),
            MissileState::Kill => self.do_kill_state(current_frame),
            MissileState::KillSelf => self.do_kill_self_state(current_frame),
            MissileState::Dead => self.do_dead_state(),
        }

        // Bridge collision detection
        // Matches C++ lines 714-740
        // Would check layer transitions for bridge hits

        Ok(())
    }

    fn handle_countermeasure_diversion(&mut self) {
        self.frames_till_decoyed = 0;
        self.no_damage = true;

        let Some(victim_arc) = TheGameLogic::find_object_by_id(self.victim_id) else {
            return;
        };
        let Some(missile_arc) = TheGameLogic::find_object_by_id(self.object_id) else {
            return;
        };

        let target_id = {
            let Ok(missile_guard) = missile_arc.read() else {
                return;
            };
            let Ok(victim_guard) = victim_arc.read() else {
                return;
            };
            missile_guard.calculate_countermeasure_to_divert_to(&victim_guard)
        };
        if target_id == INVALID_ID {
            return;
        }

        let Some(target_arc) = TheGameLogic::find_object_by_id(target_id) else {
            return;
        };
        let Ok(target_guard) = target_arc.read() else {
            return;
        };
        let target_pos = *target_guard.get_position();
        let target_id = target_guard.get_id();
        drop(target_guard);

        let ai = missile_arc
            .read()
            .ok()
            .and_then(|missile_guard| missile_guard.get_ai_update_interface());
        if let Some(ai) = ai {
            ai.ai_move_to_object(target_id, CMD_FROM_AI);
        }

        self.original_target_pos = target_pos;
        self.is_tracking_target = true;
        self.victim_id = target_id;
    }

    /// Pre-launch state: disable movement
    /// Matches C++ MissileAIUpdate::doPrelaunchState from MissileAIUpdate.cpp lines 403-411
    fn do_prelaunch_state(&mut self) {
        // Set max acceleration and turn rate to 0
        // curLoco->setMaxAcceleration(0);
        // curLoco->setMaxTurnRate(0);
    }

    /// Launch state: wait for ignition delay
    /// Matches C++ MissileAIUpdate::doLaunchState from MissileAIUpdate.cpp lines 434-448
    fn do_launch_state(&mut self, current_frame: UnsignedInt) {
        // Disable turning during launch
        // curLoco->setMaxAcceleration(0);
        // curLoco->setMaxTurnRate(0);

        let delay = self.data.ignition_delay;
        if current_frame >= self.state_timestamp + delay {
            self.switch_to_state(MissileState::Ignition, current_frame);
        }
    }

    /// Ignition state: arm warhead, start exhaust, enable movement
    /// Matches C++ MissileAIUpdate::doIgnitionState from MissileAIUpdate.cpp lines 451-474
    fn do_ignition_state(&mut self, current_frame: UnsignedInt) {
        // Enable acceleration but no turning yet
        // curLoco->setMaxAcceleration(m_maxAccel);
        // curLoco->setMaxTurnRate(0);

        // Play ignition FX
        // FXList::doFXObj(d->m_ignitionFX, getObject());

        // Create exhaust particle system
        // if (m_exhaustSysTmpl != NULL) {
        //     m_exhaustID = TheParticleSystemManager->createAttachedParticleSystemID(...)
        // }

        // Arm the warhead
        self.is_armed = true;

        // Set fuel expiration
        let now = current_frame;
        self.fuel_expiration_date = if self.data.fuel_lifetime > 0 {
            now + self.data.fuel_lifetime
        } else {
            UnsignedInt::MAX
        };

        self.switch_to_state(MissileState::AttackNoTurn, current_frame);
    }

    /// Attack state: fly toward target with optional turning
    /// Matches C++ MissileAIUpdate::doAttackState from MissileAIUpdate.cpp lines 477-554
    fn do_attack_state(&mut self, _turn_ok: bool, current_frame: UnsignedInt) {
        // Check fuel expiration
        if current_frame >= self.fuel_expiration_date {
            if self.data.detonate_on_no_fuel {
                self.detonate();
                return;
            }

            // Disable propulsion
            // curLoco->setMaxAcceleration(0);
            // curLoco->setMaxTurnRate(0);
            // Toss exhaust
        } else {
            // Enable propulsion with optional turning
            // curLoco->setMaxAcceleration(m_maxAccel);
            // curLoco->setMaxTurnRate(turnOK ? BIGNUM : 0);
        }

        // Check lock distance for terminal guidance
        if self.data.lock_distance > 0.0 {
            // Calculate distance to target
            // If within lock distance, switch to KILL state
            // Matches C++ lines 506-530
        }

        // Check dive distance
        if self.data.dive_distance > 0.0 {
            // If close enough, disable preferred height
            // Matches C++ lines 532-543
        }

        // Check if traveled far enough to enable turning
        if self.no_turn_dist_left <= 0.0 {
            self.switch_to_state(MissileState::Attack, current_frame);
        }

        // Handle lost airborne target
        // Matches C++ lines 550-553
    }

    /// Kill state: precise terminal guidance to target
    /// Matches C++ MissileAIUpdate::doKillState from MissileAIUpdate.cpp lines 557-611
    fn do_kill_state(&mut self, current_frame: UnsignedInt) {
        // Check fuel
        if current_frame >= self.fuel_expiration_date {
            if self.data.detonate_on_no_fuel {
                self.detonate();
                return;
            }

            // Lost target, fall back
            self.airborne_target_gone(current_frame);
            return;
        }

        // Enable braking mode for precise positioning
        // obj->setStatus(MAKE_OBJECT_STATUS_MASK(OBJECT_STATUS_BRAKING));

        // Enable full turning
        // curLoco->setMaxAcceleration(m_maxAccel);
        // curLoco->setMaxTurnRate(BIGNUM);

        // Check if reached target
        // if (isIdle()) { ... detonate if close enough ... }
        // Matches C++ lines 585-605
    }

    /// Kill self state: delay before final destruction
    /// Matches C++ MissileAIUpdate::doKillSelfState from MissileAIUpdate.cpp lines 413-431
    fn do_kill_self_state(&mut self, current_frame: UnsignedInt) {
        // Hold in this state for delay frames
        if current_frame < self.state_timestamp + self.data.kill_self_delay {
            return;
        }

        if self.detonation_weapon_tmpl.is_some() {
            if self.data.detonate_calls_kill {
                if let Some(obj_arc) = TheGameLogic::find_object_by_id(self.object_id) {
                    if let Ok(mut obj_guard) = obj_arc.write() {
                        obj_guard.kill(None, None);
                    }
                }
            } else {
                let _ = TheGameLogic::destroy_object_by_id(self.object_id);
            }
        }

        self.switch_to_state(MissileState::Dead, current_frame);
    }

    /// Dead state: no longer active
    /// Matches C++ MissileAIUpdate::doDeadState from MissileAIUpdate.cpp lines 614-622
    fn do_dead_state(&mut self) {
        // Disable all propulsion
        // curLoco->setMaxAcceleration(0);
        // curLoco->setMaxTurnRate(0);
    }

    /// Handle lost airborne target
    /// Matches C++ MissileAIUpdate::airborneTargetGone from MissileAIUpdate.cpp lines 759-765
    fn airborne_target_gone(&mut self, current_frame: UnsignedInt) {
        // Run out of fuel immediately
        self.fuel_expiration_date = current_frame;
        self.switch_to_state(MissileState::KillSelf, current_frame);
    }

    /// Set frames until countermeasure diversion
    /// Matches C++ MissileAIUpdate::setFramesTillCountermeasureDiversionOccurs
    /// from MissileAIUpdate.cpp lines 770-774
    pub fn set_frames_till_countermeasure_diversion_occurs(
        &mut self,
        frames: UnsignedInt,
        current_frame: UnsignedInt,
    ) {
        self.frames_till_decoyed = current_frame + frames;
    }

    /// Mark missile as jammed by ECM
    /// Matches C++ MissileAIUpdate::projectileNowJammed from MissileAIUpdate.cpp lines 777-809
    pub fn projectile_now_jammed(&mut self) {
        if self.is_jammed {
            return; // Already jammed
        }

        // Set jammed model condition
        // getObject()->setModelConditionState(MODELCONDITION_JAMMED);

        // Scatter target position
        let scatter = self.data.distance_scatter_when_jammed;
        let mut target_position = self.original_target_pos;

        target_position.x += get_game_logic_random_value_real(-scatter, scatter);
        target_position.y += get_game_logic_random_value_real(-scatter, scatter);
        if let Some(terrain) = TheTerrainLogic::get() {
            let layer = terrain.get_highest_layer_for_destination(&target_position);
            target_position.z =
                terrain.get_layer_height(target_position.x, target_position.y, layer);
        }

        // Retarget to scattered position
        // aiMoveToPosition(&targetPosition, CMD_FROM_AI);

        self.is_tracking_target = false;
        self.original_target_pos = target_position;
        self.victim_id = INVALID_ID;
        self.is_jammed = true;
    }

    /// Check if missile is armed
    pub fn projectile_is_armed(&self) -> bool {
        self.is_armed
    }

    /// Get launcher ID
    pub fn projectile_get_launcher_id(&self) -> ObjectID {
        self.launcher_id
    }
}

/// Behavior module wrapper so MissileAIUpdate participates in the update scheduler.
pub struct MissileAIUpdateBehavior {
    object: Weak<std::sync::RwLock<Object>>,
    module_data: Arc<MissileAIUpdateModuleData>,
    update: MissileAIUpdate,
}

impl MissileAIUpdateBehavior {
    pub fn new(
        object: Arc<std::sync::RwLock<Object>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let data = module_data
            .as_ref()
            .downcast_ref::<MissileAIUpdateModuleData>()
            .ok_or("Invalid MissileAIUpdate module data")?;
        let module_data = Arc::new(data.clone());
        let current_frame = TheGameLogic::get_frame();
        let mut update = MissileAIUpdate::new(module_data.clone(), current_frame);
        if let Ok(guard) = object.read() {
            update.object_id = guard.get_id();
        }
        Ok(Self {
            object: Arc::downgrade(&object),
            module_data,
            update,
        })
    }

    pub fn update_mut(&mut self) -> &mut MissileAIUpdate {
        &mut self.update
    }

    pub fn set_frames_till_countermeasure_diversion_occurs(
        &mut self,
        frames: UnsignedInt,
        current_frame: UnsignedInt,
    ) {
        self.update
            .set_frames_till_countermeasure_diversion_occurs(frames, current_frame);
    }

    pub fn projectile_launch_at_object_or_position(
        &mut self,
        victim: Option<ObjectID>,
        victim_pos: &Coord3D,
        launcher: Option<ObjectID>,
        weapon_slot: WeaponSlotType,
        specific_barrel_to_use: i32,
        detonation_weapon: Option<Weak<WeaponTemplate>>,
        exhaust_sys_override: Option<Arc<ParticleSystemTemplate>>,
    ) {
        let launch_pos = if let Some(projectile_arc) = self.object.upgrade() {
            let _ = WeaponTemplate::position_projectile_for_launch(
                &projectile_arc,
                launcher.unwrap_or(INVALID_ID),
                weapon_slot,
                specific_barrel_to_use,
            );
            let launch_pos = projectile_arc
                .read()
                .map(|obj| obj.get_position().clone())
                .unwrap_or_else(|_| Coord3D::new(0.0, 0.0, 0.0));

            if let Ok(obj_guard) = projectile_arc.read() {
                if let Some(ai) = obj_guard.get_ai_update_interface() {
                    if let Some(victim_id) = victim {
                        if self.module_data.try_to_follow_target {
                            ai.ai_move_to_object(victim_id, CMD_FROM_AI);
                        }
                    } else {
                        let mut initial_pos = *victim_pos;
                        if self.module_data.lock_distance > 0.0 {
                            initial_pos.z += APPROACH_HEIGHT;
                        }
                        ai.ai_move_to_position(&initial_pos, false, CMD_FROM_AI);
                    }
                }
            }

            if let Ok(mut obj_guard) = projectile_arc.write() {
                let mut initial_vel = self.module_data.initial_velocity;
                if self.module_data.use_weapon_speed {
                    if let Some(weapon) = detonation_weapon.as_ref().and_then(|weak| weak.upgrade())
                    {
                        initial_vel = weapon.get_projectile_speed();
                    }
                }
                if let Some(ai) = obj_guard.get_ai_update_interface() {
                    if let Ok(ai_guard) = ai.try_lock() {
                        if let Some(loco) = ai_guard.get_cur_locomotor() {
                            if let Ok(mut loco_guard) = loco.lock() {
                                loco_guard.set_max_speed(initial_vel);
                                loco_guard.set_max_acceleration(initial_vel);
                            }
                        }
                    }
                }

                let dx = victim_pos.x - launch_pos.x;
                let dy = victim_pos.y - launch_pos.y;
                let delta_z = victim_pos.z - launch_pos.z;
                let mut xy_dist = (dx * dx + dy * dy).sqrt();
                if xy_dist < 1.0 {
                    xy_dist = 1.0;
                }
                let z_factor = if delta_z > 0.0 {
                    delta_z / xy_dist
                } else {
                    0.0
                };

                let mut dir = obj_guard.get_transform_matrix().x_axis.truncate();
                if dir.length_squared() < 1e-6 {
                    dir = Coord3D::new(dx, dy, delta_z);
                }
                if dir.length_squared() > 1e-6 {
                    dir = dir.normalize();
                } else {
                    dir = Coord3D::new(1.0, 0.0, 0.0);
                }
                dir.z += 2.0 * z_factor;
                if dir.length_squared() > 1e-6 {
                    dir = dir.normalize();
                }

                if let Some(physics) = obj_guard.get_physics() {
                    let force_mag = physics.get_mass() * initial_vel;
                    let force = dir * force_mag;
                    physics.apply_motive_force(&force);
                }

                let obj_pos = *obj_guard.get_position();
                let up = if dir.y.abs() < 0.999 {
                    Coord3D::new(0.0, 1.0, 0.0)
                } else {
                    Coord3D::new(0.0, 0.0, 1.0)
                };
                let z_axis = dir.cross(up).normalize_or_zero();
                let y_axis = z_axis.cross(dir).normalize_or_zero();
                let transform = Matrix3D::from_cols(
                    Vec4::new(dir.x, dir.y, dir.z, 0.0),
                    Vec4::new(y_axis.x, y_axis.y, y_axis.z, 0.0),
                    Vec4::new(z_axis.x, z_axis.y, z_axis.z, 0.0),
                    Vec4::new(obj_pos.x, obj_pos.y, obj_pos.z, 1.0),
                );
                obj_guard.set_transform_matrix(&transform);
            }

            launch_pos
        } else {
            Coord3D::new(0.0, 0.0, 0.0)
        };

        self.update.projectile_launch_at_object_or_position(
            victim,
            victim_pos,
            &launch_pos,
            launcher,
            detonation_weapon,
            exhaust_sys_override,
        );
    }
}

impl UpdateModuleInterface for MissileAIUpdateBehavior {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        let Some(object) = self.object.upgrade() else {
            return Ok(UPDATE_SLEEP_NONE);
        };
        let current_frame = TheGameLogic::get_frame();
        let position = object
            .read()
            .map_err(|_| "MissileAIUpdateBehavior object lock poisoned")?
            .get_position()
            .clone();
        self.update
            .update(current_frame, position)
            .map_err(|err| Box::new(err) as Box<dyn std::error::Error + Send + Sync>)?;
        Ok(UPDATE_SLEEP_NONE)
    }
}

impl ProjectileUpdateInterface for MissileAIUpdateBehavior {
    fn projectile_update(&mut self, _object_id: ObjectID, _delta_time: Real) {
        let _ = UpdateModuleInterface::update(self);
    }

    fn projectile_now_jammed(&mut self) {
        if let Some(object) = self.object.upgrade() {
            if let Ok(mut guard) = object.write() {
                guard.set_model_condition_state(MODELCONDITION_JAMMED);
            }
        }
        self.update.projectile_now_jammed();
    }
}

impl BehaviorModuleInterface for MissileAIUpdateBehavior {
    fn get_module_name(&self) -> &'static str {
        "MissileAIUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_projectile_update_interface(&mut self) -> Option<&mut dyn ProjectileUpdateInterface> {
        Some(self)
    }
}

pub struct MissileAIUpdateFactory;
impl MissileAIUpdateFactory {
    pub fn create_behavior(
        thing: Arc<std::sync::RwLock<Object>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(MissileAIUpdateBehavior::new(thing, module_data)?))
    }
}

impl Snapshotable for MissileAIUpdate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        xfer.xfer_object_id(&mut self.object_id)
            .map_err(|e| format!("MissileAIUpdate xfer object_id: {:?}", e))?;
        let mut state: i32 = self.state as i32;
        xfer.xfer_int(&mut state)
            .map_err(|e| format!("MissileAIUpdate xfer state: {:?}", e))?;
        self.state = match state {
            0 => MissileState::PreLaunch,
            1 => MissileState::Launch,
            2 => MissileState::Ignition,
            3 => MissileState::AttackNoTurn,
            4 => MissileState::Attack,
            5 => MissileState::Kill,
            6 => MissileState::KillSelf,
            _ => MissileState::Dead,
        };
        xfer.xfer_unsigned_int(&mut self.state_timestamp)
            .map_err(|e| format!("MissileAIUpdate xfer state_timestamp: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.next_target_track_time)
            .map_err(|e| format!("MissileAIUpdate xfer next_target_track_time: {:?}", e))?;
        xfer.xfer_object_id(&mut self.launcher_id)
            .map_err(|e| format!("MissileAIUpdate xfer launcher_id: {:?}", e))?;
        xfer.xfer_object_id(&mut self.victim_id)
            .map_err(|e| format!("MissileAIUpdate xfer victim_id: {:?}", e))?;
        xfer.xfer_bool(&mut self.is_armed)
            .map_err(|e| format!("MissileAIUpdate xfer is_armed: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.fuel_expiration_date)
            .map_err(|e| format!("MissileAIUpdate xfer fuel_expiration_date: {:?}", e))?;
        xfer.xfer_real(&mut self.no_turn_dist_left)
            .map_err(|e| format!("MissileAIUpdate xfer no_turn_dist_left: {:?}", e))?;
        xfer.xfer_real(&mut self.prev_pos.x)
            .map_err(|e| format!("MissileAIUpdate xfer prev_pos.x: {:?}", e))?;
        xfer.xfer_real(&mut self.prev_pos.y)
            .map_err(|e| format!("MissileAIUpdate xfer prev_pos.y: {:?}", e))?;
        xfer.xfer_real(&mut self.prev_pos.z)
            .map_err(|e| format!("MissileAIUpdate xfer prev_pos.z: {:?}", e))?;
        xfer.xfer_real(&mut self.max_accel)
            .map_err(|e| format!("MissileAIUpdate xfer max_accel: {:?}", e))?;
        xfer.xfer_bool(&mut self.is_tracking_target)
            .map_err(|e| format!("MissileAIUpdate xfer is_tracking_target: {:?}", e))?;
        let mut extra_bonus_flags = self.extra_bonus_flags.bits();
        xfer.xfer_unsigned_int(&mut extra_bonus_flags)
            .map_err(|e| format!("MissileAIUpdate xfer extra_bonus_flags: {:?}", e))?;
        self.extra_bonus_flags =
            crate::common::types::WeaponBonusConditionFlags::from_bits_truncate(extra_bonus_flags);
        xfer.xfer_unsigned_int(&mut self.exhaust_id)
            .map_err(|e| format!("MissileAIUpdate xfer exhaust_id: {:?}", e))?;
        xfer.xfer_real(&mut self.original_target_pos.x)
            .map_err(|e| format!("MissileAIUpdate xfer original_target_pos.x: {:?}", e))?;
        xfer.xfer_real(&mut self.original_target_pos.y)
            .map_err(|e| format!("MissileAIUpdate xfer original_target_pos.y: {:?}", e))?;
        xfer.xfer_real(&mut self.original_target_pos.z)
            .map_err(|e| format!("MissileAIUpdate xfer original_target_pos.z: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.frames_till_decoyed)
            .map_err(|e| format!("MissileAIUpdate xfer frames_till_decoyed: {:?}", e))?;
        xfer.xfer_bool(&mut self.no_damage)
            .map_err(|e| format!("MissileAIUpdate xfer no_damage: {:?}", e))?;
        xfer.xfer_bool(&mut self.is_jammed)
            .map_err(|e| format!("MissileAIUpdate xfer is_jammed: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::system::{xfer_load::XferLoad, xfer_save::XferSave};
    use std::io::Cursor;
    use std::sync::{Mutex, MutexGuard, OnceLock, RwLock};

    static GAME_LOGIC_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn game_logic_test_guard() -> MutexGuard<'static, ()> {
        GAME_LOGIC_TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap()
    }

    fn reset_game_logic_objects() {
        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
            logic.clear_all_objects();
        }
    }

    fn register_test_object(object_id: ObjectID) -> Arc<RwLock<Object>> {
        let object = Arc::new(RwLock::new(Object::new_test(object_id, 100.0)));
        crate::system::game_logic::get_game_logic()
            .lock()
            .unwrap()
            .register_object(object.clone())
            .unwrap();
        object
    }

    fn test_weapon_template() -> (Arc<WeaponTemplate>, Weak<WeaponTemplate>) {
        let weapon = Arc::new(WeaponTemplate::new(String::from("TestMissileWeapon")));
        let weak = Arc::downgrade(&weapon);
        (weapon, weak)
    }

    #[test]
    fn test_missile_state_machine() {
        let data = Arc::new(MissileAIUpdateModuleData {
            ignition_delay: 1,
            ..Default::default()
        });
        let mut missile = MissileAIUpdate::new(data, 0);

        assert_eq!(missile.state, MissileState::PreLaunch);
        assert!(!missile.is_armed);

        // Launch
        missile.switch_to_state(MissileState::Launch, 0);
        missile.do_launch_state(0);
        assert_eq!(missile.state, MissileState::Launch); // No transition yet

        // After ignition delay
        missile.do_launch_state(1);
        assert_eq!(missile.state, MissileState::Ignition);

        // Ignition arms the missile
        missile.do_ignition_state(1);
        assert!(missile.is_armed);
        assert_eq!(missile.state, MissileState::AttackNoTurn);
    }

    #[test]
    fn test_missile_collision() {
        let data = Arc::new(MissileAIUpdateModuleData::default());
        let mut missile = MissileAIUpdate::new(data, 0);

        // Unarmed missile doesn't respond to collision
        assert!(missile.projectile_handle_collision(Some(1)));
        assert_eq!(missile.state, MissileState::PreLaunch);

        // Armed missile detonates on collision
        missile.is_armed = true;
        missile.projectile_handle_collision(Some(1));
        assert_eq!(missile.state, MissileState::KillSelf);
    }

    #[test]
    fn test_countermeasure_diversion() {
        let data = Arc::new(MissileAIUpdateModuleData::default());
        let mut missile = MissileAIUpdate::new(data, 0);

        missile.set_frames_till_countermeasure_diversion_occurs(10, 0);
        assert_eq!(missile.frames_till_decoyed, 10);
        assert!(!missile.no_damage);
    }

    #[test]
    fn test_jamming() {
        let seed = [0x1234, 0x5678, 0x9abc, 0xdef0, 0x1357, 0x2468];
        let data = Arc::new(MissileAIUpdateModuleData {
            distance_scatter_when_jammed: 100.0,
            ..Default::default()
        });
        let mut missile = MissileAIUpdate::new(data, 0);

        let original = Coord3D::new(100.0, 100.0, 10.0);
        crate::helpers::set_game_logic_random_seed(seed);
        let expected_x = original.x + get_game_logic_random_value_real(-100.0, 100.0);
        let expected_y = original.y + get_game_logic_random_value_real(-100.0, 100.0);

        crate::helpers::set_game_logic_random_seed(seed);
        missile.original_target_pos = original;
        missile.projectile_now_jammed();

        assert!(missile.is_jammed);
        assert!(!missile.is_tracking_target);
        assert_eq!(missile.victim_id, INVALID_ID);
        assert!((missile.original_target_pos.x - expected_x).abs() < 0.001);
        assert!((missile.original_target_pos.y - expected_y).abs() < 0.001);
        assert_eq!(missile.original_target_pos.z, 0.0);
    }

    #[test]
    fn kill_self_without_detonation_template_only_goes_dead() {
        let _guard = game_logic_test_guard();
        reset_game_logic_objects();
        let object = register_test_object(1001);
        let data = Arc::new(MissileAIUpdateModuleData {
            kill_self_delay: 3,
            ..Default::default()
        });
        let mut missile = MissileAIUpdate::new(data, 10);
        missile.object_id = 1001;
        missile.switch_to_state(MissileState::KillSelf, 10);

        missile.do_kill_self_state(13);

        assert_eq!(missile.state, MissileState::Dead);
        assert!(TheGameLogic::find_object_by_id(1001).is_some());
        assert!(!object.read().unwrap().is_effectively_dead());
        reset_game_logic_objects();
    }

    #[test]
    fn kill_self_destroy_path_queues_object_removal() {
        let _guard = game_logic_test_guard();
        reset_game_logic_objects();
        register_test_object(1002);
        let (_weapon, weak_weapon) = test_weapon_template();
        let data = Arc::new(MissileAIUpdateModuleData {
            kill_self_delay: 2,
            detonate_calls_kill: false,
            ..Default::default()
        });
        let mut missile = MissileAIUpdate::new(data, 20);
        missile.object_id = 1002;
        missile.detonation_weapon_tmpl = Some(weak_weapon);
        missile.switch_to_state(MissileState::KillSelf, 20);

        missile.do_kill_self_state(21);
        assert_eq!(missile.state, MissileState::KillSelf);
        assert!(TheGameLogic::find_object_by_id(1002).is_some());

        missile.do_kill_self_state(22);
        assert_eq!(missile.state, MissileState::Dead);
        {
            let mut logic = crate::system::game_logic::get_game_logic().lock().unwrap();
            logic.cleanup_dead_objects().unwrap();
        }
        assert!(TheGameLogic::find_object_by_id(1002).is_none());
        reset_game_logic_objects();
    }

    #[test]
    fn kill_self_kill_path_runs_object_kill() {
        let _guard = game_logic_test_guard();
        reset_game_logic_objects();
        let object = register_test_object(1003);
        let (_weapon, weak_weapon) = test_weapon_template();
        let data = Arc::new(MissileAIUpdateModuleData {
            kill_self_delay: 0,
            detonate_calls_kill: true,
            ..Default::default()
        });
        let mut missile = MissileAIUpdate::new(data, 30);
        missile.object_id = 1003;
        missile.detonation_weapon_tmpl = Some(weak_weapon);
        missile.switch_to_state(MissileState::KillSelf, 30);

        missile.do_kill_self_state(30);

        assert_eq!(missile.state, MissileState::Dead);
        assert!(object.read().unwrap().is_effectively_dead());
        assert!(TheGameLogic::find_object_by_id(1003).is_some());
        reset_game_logic_objects();
    }

    #[test]
    fn update_below_world_queues_projectile_destruction() {
        let _guard = game_logic_test_guard();
        reset_game_logic_objects();
        register_test_object(1004);
        let data = Arc::new(MissileAIUpdateModuleData::default());
        let mut missile = MissileAIUpdate::new(data, 0);
        missile.object_id = 1004;

        missile.update(1, Coord3D::new(0.0, 0.0, -0.01)).unwrap();

        assert!(TheGameLogic::find_object_by_id(1004).is_some());
        {
            let mut logic = crate::system::game_logic::get_game_logic().lock().unwrap();
            logic.cleanup_dead_objects().unwrap();
        }
        assert!(TheGameLogic::find_object_by_id(1004).is_none());
        reset_game_logic_objects();
    }

    #[test]
    fn xfer_preserves_extra_bonus_flags() {
        let data = Arc::new(MissileAIUpdateModuleData::default());
        let mut saved = MissileAIUpdate::new(data.clone(), 7);
        saved.extra_bonus_flags = crate::common::types::WeaponBonusConditionFlags::GARRISONED
            | crate::common::types::WeaponBonusConditionFlags::FRENZY_TWO;
        saved.exhaust_id = 99;
        saved.original_target_pos = Coord3D::new(1.0, 2.0, 3.0);
        saved.frames_till_decoyed = 123;
        saved.no_damage = true;
        saved.is_jammed = true;

        let mut bytes = Cursor::new(Vec::new());
        {
            let mut xfer = XferSave::new(&mut bytes, 6);
            saved.xfer(&mut xfer).unwrap();
        }

        bytes.set_position(0);
        let mut loaded = MissileAIUpdate::new(data, 0);
        {
            let mut xfer = XferLoad::new(&mut bytes, 6);
            loaded.xfer(&mut xfer).unwrap();
        }

        assert_eq!(loaded.extra_bonus_flags, saved.extra_bonus_flags);
        assert_eq!(loaded.exhaust_id, saved.exhaust_id);
        assert_eq!(loaded.frames_till_decoyed, saved.frames_till_decoyed);
        assert_eq!(loaded.no_damage, saved.no_damage);
        assert_eq!(loaded.is_jammed, saved.is_jammed);
    }
}
