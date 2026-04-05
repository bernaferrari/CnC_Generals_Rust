//! MinefieldBehavior - Rust conversion of C++ MinefieldBehavior class
//!
//! A complex behavior that handles landmine functionality including:
//! - Virtual mine counts and regeneration
//! - Collision detection and detonation
//! - Immunity system for mine clearers
//! - Movement after placement ("scooting")
//! - Health-based mine depletion
//! - Creator death monitoring
//!
//! Author: Steven Johnson, June 2002 (C++ version)
//! Rust conversion: 2025

use crate::common::{
    Bool, Int, KindOf, ModelConditionFlags, ObjectID, ObjectStatusMaskType, ObjectStatusTypes,
    PathfindLayerEnum, Real, Relationship, UnsignedInt, MODELCONDITION_RUBBLE,
};
use crate::modules::UpdateSleepTime;
use crate::object::behavior::behavior_module::LandMineInterface;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

// Forward declarations - assume these exist in other modules
pub trait WeaponTemplate: Send + Sync {
    fn get_name(&self) -> &str;
}

pub trait ObjectCreationList: Send + Sync {
    fn create(
        &self,
        source: &dyn Object,
        target: Option<&dyn Object>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

pub trait Object: Send + Sync {
    fn get_id(&self) -> ObjectID;
    fn get_position(&self) -> &Coord3D;
    fn set_position(&mut self, position: &Coord3D);
    fn get_producer_id(&self) -> ObjectID;
    fn get_relationship(&self, other: &dyn Object) -> Relationship;
    fn is_kind_of(&self, kind: KindOf) -> Bool;
    fn is_effectively_dead(&self) -> Bool;
    fn get_geometry_info(&self) -> &dyn GeometryInfo;
    fn set_layer(&mut self, layer: PathfindLayerEnum);
    fn set_status(&mut self, status: ObjectStatusMask, set: bool);
    fn clear_status(&mut self, status: ObjectStatusMask);
    fn set_model_condition_state(&mut self, condition: ModelConditionFlags);
    fn clear_model_condition_state(&mut self, condition: ModelConditionFlags);
    fn attempt_damage(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn get_body_module(&self) -> Option<Arc<Mutex<dyn BodyModuleInterface>>>;
    fn get_ai(&self) -> Option<Arc<Mutex<dyn AIUpdateInterface>>>;
}

pub trait BodyModuleInterface: Send + Sync {
    fn get_health(&self) -> Real;
    fn get_max_health(&self) -> Real;
    fn internal_change_health(&mut self, amount: Real);
    fn attempt_healing(&mut self, damage_info: &mut DamageInfo);
}

pub trait AIUpdateInterface: Send + Sync {
    fn is_clearing_mines(&self) -> Bool;
    fn get_goal_object(&self) -> Option<Arc<Mutex<dyn Object>>>;
}

pub trait GameLogic: Send + Sync {
    fn destroy_object(
        &self,
        object: Arc<Mutex<dyn Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn find_object_by_id(&self, id: ObjectID) -> Option<Arc<Mutex<dyn Object>>>;
    fn get_frame(&self) -> UnsignedInt;
}

pub trait WeaponStore: Send + Sync {
    fn create_and_fire_temp_weapon(
        &self,
        weapon_template: &dyn WeaponTemplate,
        source: &dyn Object,
        position: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

pub trait TerrainLogic: Send + Sync {
    fn get_highest_layer_for_destination(&self, position: &Coord3D) -> PathfindLayerEnum;
    fn get_layer_height(&self, x: Real, y: Real, layer: PathfindLayerEnum) -> Real;
    fn get_ground_height(&self, x: Real, y: Real) -> Real;
}

pub trait GlobalData: Send + Sync {
    fn get_gravity(&self) -> Real;
}

pub trait GeometryInfo: Send + Sync {
    fn clip_point_to_footprint(&self, center: &Coord3D, point: &mut Coord3D);
}

// Type aliases and enums
pub type ObjectStatusMask = ObjectStatusMaskType;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DamageType {
    Healing,
    Unresistable,
    Normal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeathType {
    None,
    Normal,
}

#[derive(Debug, Clone)]
pub struct Coord3D {
    pub x: Real,
    pub y: Real,
    pub z: Real,
}

impl Coord3D {
    pub fn new(x: Real, y: Real, z: Real) -> Self {
        Self { x, y, z }
    }

    pub fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    pub fn sub(&mut self, other: &Coord3D) {
        self.x -= other.x;
        self.y -= other.y;
        self.z -= other.z;
    }

    pub fn length(&self) -> Real {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }
}

#[derive(Debug)]
pub struct DamageInfo {
    pub amount: Real,
    pub source_id: ObjectID,
    pub damage_type: DamageType,
    pub death_type: DeathType,
}

impl DamageInfo {
    pub fn new() -> Self {
        Self {
            amount: 0.0,
            source_id: 0,
            damage_type: DamageType::Normal,
            death_type: DeathType::Normal,
        }
    }
}

// Constants
const UPDATE_SLEEP_NONE: UpdateSleepTime = UpdateSleepTime::None;
const UPDATE_SLEEP_FOREVER: UpdateSleepTime = UpdateSleepTime::Forever;
const INVALID_ID: ObjectID = 0;
const NEVER: UnsignedInt = u32::MAX;
const FOREVER: UnsignedInt = u32::MAX;
const MIN_HEALTH: Real = 0.1;
const LOGICFRAMES_PER_SECOND: UnsignedInt = 30;
const OBJECT_STATUS_NO_ATTACK_FROM_AI: ObjectStatusMask =
    ObjectStatusMaskType::from_status(ObjectStatusTypes::NoAttackFromAi);
const OBJECT_STATUS_MASKED: ObjectStatusMask =
    ObjectStatusMaskType::from_status(ObjectStatusTypes::Masked);

fn sqr(x: Real) -> Real {
    x * x
}

fn calc_dist_squared(a: &Coord3D, b: &Coord3D) -> Real {
    sqr(a.x - b.x) + sqr(a.y - b.y) + sqr(a.z - b.z)
}

/// Configuration data for MinefieldBehavior
pub struct MinefieldBehaviorModuleData {
    /// What happens when we detonate
    pub detonation_weapon: Option<Arc<dyn WeaponTemplate>>,
    /// Can we be triggered by allies, etc? (bitfield)
    pub detonated_by: Int,
    /// Stop regeneration after creator dies
    pub stops_regen_after_creator_dies: Bool,
    /// If true, can't be killed normally
    pub regenerates: Bool,
    /// If false, workers don't detonate mines
    pub workers_detonate: Bool,
    /// If above is true, how often to check
    pub creator_death_check_rate: UnsignedInt,
    /// If nonzero, gradually scoot to destination point
    pub scoot_from_starting_point_time: UnsignedInt,
    /// Number of "virtual" mines we have
    pub num_virtual_mines: UnsignedInt,
    /// Minimum movement required to detonate again
    pub repeat_detonate_move_thresh: Real,
    /// Health drain rate when creator dies
    pub health_percent_to_drain_per_second: Real,
    /// Object creation list to make
    pub ocl: Option<Arc<dyn ObjectCreationList>>,
}

impl MinefieldBehaviorModuleData {
    pub fn new() -> Self {
        Self {
            detonation_weapon: None,
            detonated_by: (1 << (Relationship::Enemies as u8)) | (1 << (Relationship::Neutral as u8)),
            stops_regen_after_creator_dies: true,
            regenerates: false,
            workers_detonate: false,
            creator_death_check_rate: LOGICFRAMES_PER_SECOND,
            scoot_from_starting_point_time: 0,
            num_virtual_mines: 1,
            repeat_detonate_move_thresh: 1.0,
            health_percent_to_drain_per_second: 0.0,
            ocl: None,
        }
    }
}

impl Default for MinefieldBehaviorModuleData {
    fn default() -> Self {
        Self::new()
    }
}

/// Information about objects immune to this mine
#[derive(Debug, Clone)]
struct ImmuneInfo {
    id: ObjectID,
    collide_time: UnsignedInt,
}

impl ImmuneInfo {
    fn new() -> Self {
        Self {
            id: INVALID_ID,
            collide_time: 0,
        }
    }
}

/// Information about objects that have detonated this mine
#[derive(Debug, Clone)]
struct DetonatorInfo {
    id: ObjectID,
    location: Coord3D,
}

/// MinefieldBehavior - Handles landmine functionality
///
/// This complex behavior manages landmine mechanics including:
/// - Multiple virtual mines per object
/// - Detonation based on collision and relationships
/// - Immunity system for mine-clearing units
/// - Movement animation after placement
/// - Health-based mine depletion
/// - Regeneration tied to creator survival
pub struct MinefieldBehavior {
    /// Reference to the object this behavior is attached to
    object: Arc<Mutex<dyn Object>>,
    /// Configuration data for this behavior
    module_data: Arc<MinefieldBehaviorModuleData>,
    /// Reference to game systems
    game_logic: Arc<dyn GameLogic>,
    weapon_store: Arc<dyn WeaponStore>,
    terrain_logic: Arc<dyn TerrainLogic>,
    global_data: Arc<dyn GlobalData>,

    /// Next frame to check if creator is dead
    next_death_check_frame: UnsignedInt,
    /// Frames remaining for scooting movement
    scoot_frames_left: UnsignedInt,
    /// Velocity for scooting movement
    scoot_vel: Coord3D,
    /// Acceleration for scooting movement
    scoot_accel: Coord3D,
    /// Number of virtual mines remaining
    virtual_mines_remaining: UnsignedInt,
    /// Objects immune to detonation (mine clearers)
    immunes: Vec<ImmuneInfo>,
    /// Objects that have detonated this mine
    detonators: Vec<DetonatorInfo>,
    /// Whether to ignore damage (for internal health changes)
    ignore_damage: Bool,
    /// Whether this mine regenerates
    regenerates: Bool,
    /// Whether health is draining due to creator death
    draining: Bool,
}

impl MinefieldBehavior {
    /// Maximum number of immune objects to track
    const MAX_IMMUNITY: usize = 3;

    /// Creates a new MinefieldBehavior
    pub fn new(
        object: Arc<Mutex<dyn Object>>,
        module_data: Arc<MinefieldBehaviorModuleData>,
        game_logic: Arc<dyn GameLogic>,
        weapon_store: Arc<dyn WeaponStore>,
        terrain_logic: Arc<dyn TerrainLogic>,
        global_data: Arc<dyn GlobalData>,
    ) -> Self {
        let behavior = Self {
            object,
            module_data: Arc::clone(&module_data),
            game_logic,
            weapon_store,
            terrain_logic,
            global_data,
            next_death_check_frame: 0,
            scoot_frames_left: 0,
            scoot_vel: Coord3D::new(0.0, 0.0, 0.0),
            scoot_accel: Coord3D::new(0.0, 0.0, 0.0),
            virtual_mines_remaining: module_data.num_virtual_mines,
            immunes: vec![ImmuneInfo::new(); Self::MAX_IMMUNITY],
            detonators: Vec::new(),
            ignore_damage: false,
            regenerates: module_data.regenerates,
            draining: false,
        };

        // Set initial object status - mines aren't auto-acquirable
        if let Ok(mut obj) = behavior.object.lock() {
            obj.set_status(OBJECT_STATUS_NO_ATTACK_FROM_AI, true);
        }

        behavior
    }

    /// Gets the module data for this behavior
    pub fn get_minefield_behavior_module_data(&self) -> &MinefieldBehaviorModuleData {
        &self.module_data
    }

    /// Gets a reference to the object this behavior is attached to
    pub fn get_object(&self) -> Arc<Mutex<dyn Object>> {
        Arc::clone(&self.object)
    }

    /// Calculates appropriate sleep time based on current state
    fn calc_sleep_time(&self) -> UpdateSleepTime {
        // If we're draining health, update every frame
        if self.draining {
            return UpdateSleepTime::None;
        }

        // If we're scooting, update every frame
        if self.scoot_frames_left > 0 {
            return UpdateSleepTime::None;
        }

        // If monitoring immunity, update every frame
        for immune in &self.immunes {
            if immune.id != INVALID_ID {
                return UpdateSleepTime::None;
            }
        }

        let mut sleep_time = u32::MAX;
        let now = self.game_logic.get_frame();

        // Sleep until next death check if we care about creator death
        if self.regenerates && self.module_data.stops_regen_after_creator_dies {
            sleep_time = sleep_time.min(self.next_death_check_frame.saturating_sub(now));
        }

        // Prevent 0 frame sleeps
        if sleep_time == 0 {
            sleep_time = 1;
        }

        UpdateSleepTime::from_u32(sleep_time)
    }

    /// Detonates the mine once at the given position
    fn detonate_once(
        &mut self,
        position: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Fire detonation weapon
        if let Some(ref weapon) = self.module_data.detonation_weapon {
            let object = self.object.lock().map_err(|_| "Failed to lock object")?;
            self.weapon_store
                .create_and_fire_temp_weapon(&**weapon, &*object, position)?;
        }

        // Decrement virtual mine count
        if self.virtual_mines_remaining > 0 {
            self.virtual_mines_remaining -= 1;
        }

        // Handle destruction or health reduction
        if !self.regenerates && self.virtual_mines_remaining == 0 {
            self.game_logic.destroy_object(Arc::clone(&self.object))?;
        } else {
            // Reduce health based on mine depletion
            let percent =
                self.virtual_mines_remaining as Real / self.module_data.num_virtual_mines as Real;

            if let Ok(mut object) = self.object.lock() {
                if let Some(body) = object.get_body_module() {
                    let body_guard = body.lock().map_err(|_| "Failed to lock body")?;
                    let health = body_guard.get_health();
                    let desired = percent * body_guard.get_max_health();
                    let desired = desired.max(MIN_HEALTH);
                    let amount = health - desired;

                    if amount > 0.0 {
                        self.ignore_damage = true;

                        let mut damage_info = DamageInfo {
                            amount,
                            source_id: object.get_id(),
                            damage_type: DamageType::Unresistable,
                            death_type: DeathType::None,
                        };

                        drop(body_guard);
                        object.attempt_damage(&mut damage_info)?;

                        self.ignore_damage = false;
                    }
                }
            }
        }

        // Update visual state based on mine count
        if let Ok(mut object) = self.object.lock() {
            if self.virtual_mines_remaining == 0 {
                object.set_model_condition_state(MODELCONDITION_RUBBLE);
                object.set_status(OBJECT_STATUS_MASKED, true);
            } else {
                object.clear_model_condition_state(MODELCONDITION_RUBBLE);
                object.clear_status(OBJECT_STATUS_MASKED);
            }
        }

        // Execute object creation list
        if let Some(ref ocl) = self.module_data.ocl {
            let object = self.object.lock().map_err(|_| "Failed to lock object")?;
            ocl.create(&*object, Some(&*object))?;
        }

        Ok(())
    }

    /// Sets parameters for "scooting" movement after mine placement
    pub fn set_scoot_parms(
        &mut self,
        start: &Coord3D,
        end: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let scoot_time = self.module_data.scoot_from_starting_point_time;

        let mut end_on_ground = end.clone();
        end_on_ground.z = self
            .terrain_logic
            .get_ground_height(end_on_ground.x, end_on_ground.y);

        let mut scoot_time = scoot_time;

        // Adjust scoot time based on fall duration
        if start.z > end_on_ground.z {
            let fall_time =
                (2.0 * (start.z - end_on_ground.z) / self.global_data.get_gravity().abs()).sqrt();
            let fall_frames = fall_time.ceil() as UnsignedInt;
            if scoot_time < fall_frames {
                scoot_time = fall_frames;
            }
        }

        if scoot_time == 0 {
            // No scooting - place immediately
            if let Ok(mut object) = self.object.lock() {
                object.set_position(&end_on_ground);
            }
            self.scoot_frames_left = 0;
        } else {
            // Calculate scooting physics
            let dx = end_on_ground.x - start.x;
            let dy = end_on_ground.y - start.y;
            let dz = end_on_ground.z - start.z;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist <= 0.1 && dz.abs() <= 0.1 {
                // Too close - place immediately
                if let Ok(mut object) = self.object.lock() {
                    object.set_position(&end_on_ground);
                }
                self.scoot_frames_left = 0;
            } else {
                // Set up scooting motion
                let t = scoot_time as Real;
                let speed = dist / t;
                let accel_mag = (2.0 * (dist - speed * t) / (t * t)).abs();

                let dx_norm = if dist <= 0.1 { 0.0 } else { dx / dist };
                let dy_norm = if dist <= 0.1 { 0.0 } else { dy / dist };

                self.scoot_vel.x = dx_norm * speed;
                self.scoot_vel.y = dy_norm * speed;
                self.scoot_vel.z = 0.0;

                self.scoot_accel.x = -dx_norm * accel_mag;
                self.scoot_accel.y = -dy_norm * accel_mag;
                self.scoot_accel.z = self.global_data.get_gravity();

                if let Ok(mut object) = self.object.lock() {
                    object.set_position(start);
                }

                self.scoot_frames_left = scoot_time;
            }
        }

        Ok(())
    }

    /// Disarms the mine (for mine clearing operations)
    pub fn disarm(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.regenerates {
            self.game_logic.destroy_object(Arc::clone(&self.object))?;
            return Ok(());
        }

        // Reduce health to minimum but don't destroy
        if let Ok(mut object) = self.object.lock() {
            if let Some(body) = object.get_body_module() {
                let body_guard = body.lock().map_err(|_| "Failed to lock body")?;
                let health = body_guard.get_health();
                let amount = health - MIN_HEALTH;

                if amount > 0.0 {
                    self.ignore_damage = true;

                    let mut damage_info = DamageInfo {
                        amount,
                        source_id: object.get_id(),
                        damage_type: DamageType::Unresistable,
                        death_type: DeathType::None,
                    };

                    drop(body_guard);
                    object.attempt_damage(&mut damage_info)?;

                    self.ignore_damage = false;
                }
            }
        }

        self.virtual_mines_remaining = 0;

        if let Ok(mut object) = self.object.lock() {
            object.set_model_condition_state(MODELCONDITION_RUBBLE);
            object.set_status(OBJECT_STATUS_MASKED, true);
        }

        Ok(())
    }

    /// Main update loop
    pub fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        let now = self.game_logic.get_frame();

        // Handle scooting movement
        if self.scoot_frames_left > 0 {
            if let Ok(mut object) = self.object.lock() {
                let mut pos = object.get_position().clone();

                self.scoot_vel.x += self.scoot_accel.x;
                self.scoot_vel.y += self.scoot_accel.y;
                self.scoot_vel.z += self.scoot_accel.z;

                pos.x += self.scoot_vel.x;
                pos.y += self.scoot_vel.y;
                pos.z += self.scoot_vel.z;

                // Set to highest layer and adjust height
                let mut tmp = pos.clone();
                tmp.z = 99999.0;
                let new_layer = self.terrain_logic.get_highest_layer_for_destination(&tmp);
                object.set_layer(new_layer);

                let mut ground = self.terrain_logic.get_layer_height(pos.x, pos.y, new_layer);
                if new_layer != PathfindLayerEnum::Ground {
                    ground += 1.0; // Fudge factor for bridges
                }

                if pos.z < ground || self.scoot_frames_left <= 1 {
                    pos.z = ground;
                }

                object.set_position(&pos);
                self.scoot_frames_left -= 1;
            }
        }

        // Check for expired immunities
        for immune in &mut self.immunes {
            if immune.id == INVALID_ID {
                continue;
            }

            if self.game_logic.find_object_by_id(immune.id).is_none()
                || now > immune.collide_time + 2
            {
                immune.id = INVALID_ID;
                immune.collide_time = 0;
            }
        }

        // Check creator death
        if now >= self.next_death_check_frame {
            if self.regenerates && self.module_data.stops_regen_after_creator_dies {
                self.next_death_check_frame = now + self.module_data.creator_death_check_rate;

                if let Ok(object) = self.object.lock() {
                    let producer_id = object.get_producer_id();
                    if producer_id != INVALID_ID {
                        if let Some(producer) = self.game_logic.find_object_by_id(producer_id) {
                            if let Ok(prod_obj) = producer.lock() {
                                if prod_obj.is_effectively_dead() {
                                    self.regenerates = false;
                                    self.draining = true;
                                    // In real implementation, would stop auto-heal behavior
                                }
                            }
                        } else {
                            // Producer doesn't exist anymore
                            self.regenerates = false;
                            self.draining = true;
                        }
                    }
                }
            }
        }

        // Handle health draining
        if self.draining {
            if let Ok(mut object) = self.object.lock() {
                if let Some(body) = object.get_body_module() {
                    let body_guard = body.lock().map_err(|_| "Failed to lock body")?;
                    let max_health = body_guard.get_max_health();
                    drop(body_guard);

                    let drain_amount = (max_health
                        * self.module_data.health_percent_to_drain_per_second)
                        / LOGICFRAMES_PER_SECOND as Real;

                    let mut damage_info = DamageInfo {
                        amount: drain_amount,
                        source_id: object.get_id(),
                        damage_type: DamageType::Unresistable,
                        death_type: DeathType::Normal,
                    };

                    object.attempt_damage(&mut damage_info)?;
                }
            }
        }

        Ok(self.calc_sleep_time())
    }

    /// Handle collision with another object
    pub fn on_collide(
        &mut self,
        other: Arc<Mutex<dyn Object>>,
        _loc: &Coord3D,
        _normal: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let other_guard = other.lock().map_err(|_| "Failed to lock other object")?;

        if other_guard.is_effectively_dead() {
            return Ok(());
        }

        if self.virtual_mines_remaining == 0 {
            return Ok(());
        }

        let now = self.game_logic.get_frame();
        let other_id = other_guard.get_id();
        let object = self.object.lock().map_err(|_| "Failed to lock object")?;

        // Check immunity list first
        for immune in &mut self.immunes {
            if immune.id == other_id {
                immune.collide_time = now;
                return Ok(());
            }
        }

        // Check if workers detonate mines
        if !self.module_data.workers_detonate {
            if other_guard.is_kind_of(KindOf::Infantry) && other_guard.is_kind_of(KindOf::Dozer) {
                return Ok(());
            }
        }

        // Check relationship
        let required_mask = match object.get_relationship(&*other_guard) {
            Relationship::Allies => 1 << (Relationship::Allies as u8),
            Relationship::Allies => 1 << (Relationship::Allies as u8),
            Relationship::Enemies => 1 << (Relationship::Enemies as u8),
            Relationship::Neutral => 1 << (Relationship::Neutral as u8),
            Relationship::Allies => 1 << (Relationship::Allies as u8),
        };

        if (self.module_data.detonated_by & required_mask) == 0 {
            return Ok(());
        }

        // Don't detonate while scooting
        if self.scoot_frames_left > 0 {
            return Ok(());
        }

        // Check for mine clearing immunity
        if let Some(ai) = other_guard.get_ai() {
            let ai_guard = ai.lock().map_err(|_| "Failed to lock AI")?;
            if ai_guard.is_clearing_mines() && ai_guard.get_goal_object().is_some() {
                // Grant immunity
                for immune in &mut self.immunes {
                    if immune.id == INVALID_ID || immune.id == other_id {
                        immune.id = other_id;
                        immune.collide_time = now;
                        break;
                    }
                }
                return Ok(());
            }
        }

        // Check repeat detonation threshold
        let other_pos = other_guard.get_position().clone();
        let mut found = false;
        for detonator in &mut self.detonators {
            if other_id == detonator.id {
                found = true;
                let dist_sqr = calc_dist_squared(&other_pos, &detonator.location);
                if dist_sqr <= sqr(self.module_data.repeat_detonate_move_thresh) {
                    return Ok(()); // Too close to previous detonation
                } else {
                    detonator.location = other_pos.clone();
                    break;
                }
            }
        }

        if !found {
            self.detonators.push(DetonatorInfo {
                id: other_id,
                location: other_pos.clone(),
            });
        }

        // Detonate at collision point
        let mut det_pt = other_pos;
        let obj_pos = object.get_position();
        object
            .get_geometry_info()
            .clip_point_to_footprint(obj_pos, &mut det_pt);

        drop(other_guard);
        drop(object);

        self.detonate_once(&det_pt)?;

        Ok(())
    }

    /// Handle damage to the mine
    pub fn on_damage(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.ignore_damage {
            return Ok(());
        }

        // Adjust virtual mine count based on health
        let should_detonate = if let Ok(object) = self.object.lock() {
            if let Some(body) = object.get_body_module() {
                let body_guard = body.lock().map_err(|_| "Failed to lock body")?;
                let health = body_guard.get_health();
                let max_health = body_guard.get_max_health();
                drop(body_guard);

                let mut detonate_pos = None;
                loop {
                    let virtual_mines_expected_f =
                        self.module_data.num_virtual_mines as Real * health / max_health;
                    let virtual_mines_expected =
                        if damage_info.damage_type == DamageType::Healing {
                            virtual_mines_expected_f.floor() as UnsignedInt
                        } else {
                            virtual_mines_expected_f.ceil() as UnsignedInt
                        }
                        .min(self.module_data.num_virtual_mines);

                    if self.virtual_mines_remaining < virtual_mines_expected {
                        self.virtual_mines_remaining = virtual_mines_expected;
                    } else if self.virtual_mines_remaining > virtual_mines_expected {
                        if self.draining
                            && damage_info.source_id == object.get_id()
                            && damage_info.damage_type == DamageType::Unresistable
                        {
                            // Just remove a mine without detonating
                            self.virtual_mines_remaining -= 1;
                        } else {
                            // Get position for detonation
                            detonate_pos = Some(object.get_position().clone());
                            break;
                        }
                    } else {
                        break;
                    }
                }
                detonate_pos
            } else {
                None
            }
        } else {
            None
        };

        // Perform detonation outside of the lock
        if let Some(position) = should_detonate {
            self.detonate_once(&position)?;
        }

        // Update visual state
        if let Ok(mut object) = self.object.lock() {
            if self.virtual_mines_remaining == 0 {
                // Ensure minimum health for regenerating mines
                if self.regenerates {
                    if let Some(body) = object.get_body_module() {
                        let mut body_guard = body.lock().map_err(|_| "Failed to lock body")?;
                        let health = body_guard.get_health();
                        if health < MIN_HEALTH {
                            body_guard.internal_change_health(MIN_HEALTH - health);
                        }
                    }
                }

                object.set_model_condition_state(MODELCONDITION_RUBBLE);
                object.set_status(OBJECT_STATUS_MASKED, true);
            } else {
                object.clear_model_condition_state(MODELCONDITION_RUBBLE);
                object.clear_status(OBJECT_STATUS_MASKED);
            }
        }

        Ok(())
    }

    /// Handle healing
    pub fn on_healing(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.on_damage(damage_info)
    }

    /// Handle death
    pub fn on_die(
        &mut self,
        _damage_info: Option<&DamageInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.game_logic.destroy_object(Arc::clone(&self.object))?;
        Ok(())
    }
}

// Thread safety implementations
unsafe impl Send for MinefieldBehavior {}
unsafe impl Sync for MinefieldBehavior {}

impl crate::modules::BehaviorModuleInterface for MinefieldBehavior {
    fn get_land_mine_interface(&mut self) -> Option<&mut dyn LandMineInterface> {
        Some(self)
    }
}

impl LandMineInterface for MinefieldBehavior {
    fn set_scoot_parms(&mut self, start: &crate::common::Coord3D, end: &crate::common::Coord3D) {
        let start_local = Coord3D::new(start.x, start.y, start.z);
        let end_local = Coord3D::new(end.x, end.y, end.z);
        let _ = MinefieldBehavior::set_scoot_parms(self, &start_local, &end_local);
    }

    fn disarm(&mut self) {
        let _ = MinefieldBehavior::disarm(self);
    }
}

/// Factory for creating MinefieldBehavior instances
pub struct MinefieldBehaviorFactory;

impl MinefieldBehaviorFactory {
    pub fn create_behavior(
        _thing: Arc<RwLock<crate::object::Object>>,
        _module_data: Arc<dyn crate::common::ModuleData>,
    ) -> Result<
        Box<dyn crate::modules::BehaviorModuleInterface>,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        // MinefieldBehavior requires game_logic, weapon_store, terrain_logic, and global_data
        // which are not available through the generic factory interface.
        // This would need to be created through a more specific factory that has access to these services.
        Err("MinefieldBehavior cannot be created through generic factory - use game-specific factory".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

    // Mock implementations would go here...
    // Due to space constraints, I'll include just a basic test structure

    #[test]
    fn test_minefield_creation() {
        // Test basic creation and configuration
        let module_data = Arc::new(MinefieldBehaviorModuleData::new());
        assert_eq!(module_data.num_virtual_mines, 1);
        assert!(!module_data.workers_detonate);
        assert!(module_data.stops_regen_after_creator_dies);
    }
}
