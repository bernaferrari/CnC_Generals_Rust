//! Weapon Target Acquisition System
//!
//! This module implements functional weapon target acquisition with:
//! - Target search within weapon range
//! - Target validation (can this unit attack that target?)
//! - Lead prediction for moving targets
//! - Lock-on mechanics for missiles
//! - Priority-based targeting
//! - Line-of-sight checking
//! - Integration with spatial partitioning

use crate::common::{Coord3D, KindOf};
use crate::weapon::{
    BallisticsCalculator, TargetPrediction, WeaponAntiMask, WeaponBonus, WeaponTemplate,
};
use crate::{GameLogicError, GameLogicResult};

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub type ObjectId = u32;
#[allow(dead_code)]
pub const INVALID_OBJECT_ID: ObjectId = 0;

/// Target priority classification (per C++ design: structure > siege > infantry > armor)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TargetPriorityClass {
    /// Structures/Buildings (highest priority)
    Structure = 0,
    /// Siege weapons and artillery
    Siege = 1,
    /// Infantry units
    Infantry = 2,
    /// Armored vehicles
    Armor = 3,
    /// Aircraft
    Aircraft = 4,
    /// Other/default
    Other = 5,
}

/// Lock-on state for guided weapons (missiles)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockOnState {
    /// No lock-on, searching for target
    Searching,
    /// Acquiring lock on target
    Acquiring { target_id: ObjectId, progress: u8 },
    /// Locked on target
    Locked { target_id: ObjectId },
    /// Lost lock on target
    Lost { last_target_id: ObjectId },
}

/// Target acquisition result
#[derive(Debug, Clone)]
pub struct TargetAcquisitionResult {
    /// Selected target object ID
    pub target_id: ObjectId,
    /// Target position
    pub position: Coord3D,
    /// Predicted intercept position (for moving targets)
    pub predicted_position: Option<Coord3D>,
    /// Distance to target
    pub distance: f32,
    /// Priority class of target
    pub priority_class: TargetPriorityClass,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f32,
    /// Whether line of sight is clear
    pub has_line_of_sight: bool,
}

/// Target search parameters
#[derive(Debug, Clone)]
pub struct TargetSearchParams {
    /// Shooter position
    pub shooter_pos: Coord3D,
    /// Shooter object ID
    pub shooter_id: ObjectId,
    /// Maximum search range
    pub max_range: f32,
    /// Minimum range (for artillery)
    pub min_range: f32,
    /// Weapon anti-mask (what can be targeted)
    pub anti_mask: WeaponAntiMask,
    /// Preferred target priority classes
    pub preferred_priorities: Vec<TargetPriorityClass>,
    /// Whether to require line of sight
    pub require_line_of_sight: bool,
    /// Weapon bonus (affects range)
    pub weapon_bonus: WeaponBonus,
    /// Projectile speed for lead calculation
    pub projectile_speed: f32,
}

/// Weapon target acquisition system
pub struct WeaponTargetAcquisition {
    /// Lock-on tracking for guided weapons
    lock_on_states: RwLock<HashMap<ObjectId, LockOnState>>,
    /// Lock-on acquisition time (frames)
    lock_on_time: u32,
    /// Line-of-sight cache (shooter, target) -> (has_los, frame)
    los_cache: RwLock<HashMap<(ObjectId, ObjectId), (bool, u32)>>,
    /// Cache duration in frames
    los_cache_duration: u32,
}

impl WeaponTargetAcquisition {
    /// Create a new weapon target acquisition system
    pub fn new() -> Self {
        Self {
            lock_on_states: RwLock::new(HashMap::new()),
            lock_on_time: 30, // 1 second at 30 FPS
            los_cache: RwLock::new(HashMap::new()),
            los_cache_duration: 15, // 0.5 seconds
        }
    }

    /// Find the best target for a weapon
    pub fn find_best_target(
        &self,
        params: &TargetSearchParams,
        current_frame: u32,
    ) -> GameLogicResult<Option<TargetAcquisitionResult>> {
        Ok(self
            .find_best_targets(params, current_frame, 1)?
            .into_iter()
            .next())
    }

    /// Find the best targets for a weapon, ordered by the same priority and
    /// confidence rules as single-target acquisition.
    pub fn find_best_targets(
        &self,
        params: &TargetSearchParams,
        current_frame: u32,
        max_targets: usize,
    ) -> GameLogicResult<Vec<TargetAcquisitionResult>> {
        if max_targets == 0 {
            return Ok(Vec::new());
        }

        // 1. Get all objects within range using spatial partitioning
        let potential_targets = self.get_objects_in_range(&params.shooter_pos, params.max_range)?;

        if potential_targets.is_empty() {
            return Ok(Vec::new());
        }

        // 2. Filter and validate targets
        let mut valid_targets = Vec::new();

        for target_id in potential_targets {
            // Skip self
            if target_id == params.shooter_id {
                continue;
            }

            // Validate target
            if let Some(acquisition_result) =
                self.validate_and_score_target(target_id, params, current_frame)?
            {
                valid_targets.push(acquisition_result);
            }
        }

        if valid_targets.is_empty() {
            return Ok(Vec::new());
        }

        // 3. Sort by priority class (primary) and then by score (secondary)
        valid_targets.sort_by(|a, b| {
            // First compare by priority class (lower value = higher priority)
            match a.priority_class.cmp(&b.priority_class) {
                std::cmp::Ordering::Equal => {
                    // If same priority, compare by confidence (higher = better)
                    b.confidence
                        .partial_cmp(&a.confidence)
                        .unwrap_or(std::cmp::Ordering::Equal)
                }
                other => other,
            }
        });

        valid_targets.truncate(max_targets);
        Ok(valid_targets)
    }

    /// Evaluate a specific target using the same validation/scoring pipeline
    /// as full target search.
    pub fn evaluate_target(
        &self,
        target_id: ObjectId,
        params: &TargetSearchParams,
        current_frame: u32,
    ) -> GameLogicResult<Option<TargetAcquisitionResult>> {
        if target_id == params.shooter_id {
            return Ok(None);
        }
        self.validate_and_score_target(target_id, params, current_frame)
    }

    /// Validate and score a potential target
    fn validate_and_score_target(
        &self,
        target_id: ObjectId,
        params: &TargetSearchParams,
        current_frame: u32,
    ) -> GameLogicResult<Option<TargetAcquisitionResult>> {
        // Get target info
        let target_pos = self.get_object_position(target_id)?;
        let distance = params.shooter_pos.distance(target_pos);

        // Check range constraints
        if distance > params.max_range || distance < params.min_range {
            return Ok(None);
        }

        // Check if target can be attacked based on anti-mask
        if !self.can_weapon_attack_target(&params.anti_mask, target_id)? {
            return Ok(None);
        }

        // Check line of sight if required
        let has_los = if params.require_line_of_sight {
            self.check_line_of_sight(params.shooter_id, target_id, current_frame)?
        } else {
            true
        };

        if params.require_line_of_sight && !has_los {
            return Ok(None);
        }

        // Get target priority class
        let priority_class = self.get_target_priority_class(target_id)?;

        // Calculate target lead for moving targets
        let (predicted_position, confidence) = if params.projectile_speed > 0.0 {
            self.calculate_target_lead(target_id, &params.shooter_pos, params.projectile_speed)?
        } else {
            (None, 1.0) // Instant hit weapons don't need prediction
        };

        // Calculate confidence score
        let mut score = confidence;

        // Bonus for preferred priority classes
        if params.preferred_priorities.contains(&priority_class) {
            score *= 1.5;
        }

        // Penalty for distance (closer is better)
        let distance_factor = 1.0 - (distance / params.max_range).min(1.0);
        score *= 0.5 + distance_factor * 0.5;

        // Bonus for having line of sight
        if has_los {
            score *= 1.2;
        }

        // Check if target is already being engaged heavily
        let engagement_penalty = self.get_engagement_penalty(target_id)?;
        score *= engagement_penalty;

        Ok(Some(TargetAcquisitionResult {
            target_id,
            position: target_pos,
            predicted_position,
            distance,
            priority_class,
            confidence: score.min(1.0),
            has_line_of_sight: has_los,
        }))
    }

    /// Calculate target lead prediction for moving targets
    fn calculate_target_lead(
        &self,
        target_id: ObjectId,
        shooter_pos: &Coord3D,
        projectile_speed: f32,
    ) -> GameLogicResult<(Option<Coord3D>, f32)> {
        // Get target position and velocity
        let target_pos = self.get_object_position(target_id)?;
        let target_velocity = self.get_object_velocity(target_id)?;

        // Check if target is moving
        let target_speed = target_velocity.distance(Coord3D::new(0.0, 0.0, 0.0));
        if target_speed < 1.0 {
            // Target is stationary or moving very slowly
            return Ok((None, 1.0));
        }

        // Use ballistics calculator to predict intercept
        let prediction = BallisticsCalculator::predict_target_intercept(
            shooter_pos,
            &target_pos,
            &target_velocity,
            projectile_speed,
        )?;

        Ok((Some(prediction.predicted_position), prediction.confidence))
    }

    /// Check if weapon can attack target based on anti-mask
    fn can_weapon_attack_target(
        &self,
        anti_mask: &WeaponAntiMask,
        target_id: ObjectId,
    ) -> GameLogicResult<bool> {
        let target_kind = self.get_object_kind(target_id)?;

        // Check anti-mask flags
        match target_kind {
            ObjectKind::AirborneVehicle => Ok(anti_mask.contains(WeaponAntiMask::AIRBORNE_VEHICLE)),
            ObjectKind::Ground => Ok(anti_mask.contains(WeaponAntiMask::GROUND)),
            ObjectKind::Projectile => Ok(anti_mask.contains(WeaponAntiMask::PROJECTILE)),
            ObjectKind::SmallMissile => Ok(anti_mask.contains(WeaponAntiMask::SMALL_MISSILE)),
            ObjectKind::Mine => {
                Ok(anti_mask.contains(WeaponAntiMask::MINE | WeaponAntiMask::GROUND))
            }
            ObjectKind::AirborneInfantry => {
                Ok(anti_mask.contains(WeaponAntiMask::AIRBORNE_INFANTRY))
            }
            ObjectKind::BallisticMissile => {
                Ok(anti_mask.contains(WeaponAntiMask::BALLISTIC_MISSILE))
            }
            ObjectKind::Parachute => Ok(anti_mask.contains(WeaponAntiMask::PARACHUTE)),
            ObjectKind::Unknown => Ok(false),
        }
    }

    /// Get target priority class
    fn get_target_priority_class(
        &self,
        target_id: ObjectId,
    ) -> GameLogicResult<TargetPriorityClass> {
        let target_type = self.get_object_type(target_id)?;

        // Map object type to priority class
        Ok(match target_type.as_str() {
            // Structures (highest priority)
            t if t.contains("Building") || t.contains("Structure") => {
                TargetPriorityClass::Structure
            }
            // Siege weapons
            t if t.contains("Artillery")
                || t.contains("Tomahawk")
                || t.contains("Inferno")
                || t.contains("Scud") =>
            {
                TargetPriorityClass::Siege
            }
            // Infantry
            t if t.contains("Infantry") || t.contains("Soldier") => TargetPriorityClass::Infantry,
            // Armor
            t if t.contains("Tank") || t.contains("Vehicle") => TargetPriorityClass::Armor,
            // Aircraft
            t if t.contains("Aircraft") || t.contains("Plane") || t.contains("Helicopter") => {
                TargetPriorityClass::Aircraft
            }
            // Default
            _ => TargetPriorityClass::Other,
        })
    }

    /// Check line of sight between shooter and target
    fn check_line_of_sight(
        &self,
        shooter_id: ObjectId,
        target_id: ObjectId,
        current_frame: u32,
    ) -> GameLogicResult<bool> {
        // Check cache first
        {
            let cache = self.los_cache.read().map_err(|e| {
                GameLogicError::Threading(format!("Failed to read LOS cache: {}", e))
            })?;

            if let Some(&(has_los, cache_frame)) = cache.get(&(shooter_id, target_id)) {
                if current_frame - cache_frame < self.los_cache_duration {
                    return Ok(has_los);
                }
            }
        }

        // Perform actual line of sight check
        let shooter_pos = self.get_object_position(shooter_id)?;
        let target_pos = self.get_object_position(target_id)?;

        let has_los = self.raycast_line_of_sight(&shooter_pos, &target_pos)?;

        // Update cache
        {
            let mut cache = self.los_cache.write().map_err(|e| {
                GameLogicError::Threading(format!("Failed to write LOS cache: {}", e))
            })?;

            cache.insert((shooter_id, target_id), (has_los, current_frame));
        }

        Ok(has_los)
    }

    /// Update lock-on state for guided weapons
    pub fn update_lock_on_state(
        &self,
        shooter_id: ObjectId,
        target_id: Option<ObjectId>,
    ) -> GameLogicResult<LockOnState> {
        let mut states = self.lock_on_states.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire lock-on states: {}", e))
        })?;

        let current_state = states
            .get(&shooter_id)
            .copied()
            .unwrap_or(LockOnState::Searching);

        let new_state = match (current_state, target_id) {
            // No target - reset to searching
            (_, None) => LockOnState::Searching,

            // Starting acquisition
            (LockOnState::Searching, Some(id)) => LockOnState::Acquiring {
                target_id: id,
                progress: 0,
            },

            // Continue acquisition
            (
                LockOnState::Acquiring {
                    target_id: current,
                    progress,
                },
                Some(id),
            ) if current == id => {
                if progress >= 100 {
                    // Lock acquired
                    LockOnState::Locked { target_id: id }
                } else {
                    // Increment progress
                    let increment = (100.0 / self.lock_on_time as f32).ceil() as u8;
                    LockOnState::Acquiring {
                        target_id: id,
                        progress: (progress + increment).min(100),
                    }
                }
            }

            // Target changed during acquisition - restart
            (LockOnState::Acquiring { .. }, Some(id)) => LockOnState::Acquiring {
                target_id: id,
                progress: 0,
            },

            // Maintain or restart lock while locked
            (LockOnState::Locked { target_id: current }, Some(id)) => {
                if current == id {
                    LockOnState::Locked { target_id: current }
                } else {
                    LockOnState::Acquiring {
                        target_id: id,
                        progress: 0,
                    }
                }
            }

            // Lost lock - trying to reacquire
            (LockOnState::Lost { .. }, Some(id)) => LockOnState::Acquiring {
                target_id: id,
                progress: 0,
            },
        };

        states.insert(shooter_id, new_state);
        Ok(new_state)
    }

    /// Get lock-on state for a shooter
    pub fn get_lock_on_state(&self, shooter_id: ObjectId) -> GameLogicResult<LockOnState> {
        let states = self.lock_on_states.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to read lock-on states: {}", e))
        })?;

        Ok(states
            .get(&shooter_id)
            .copied()
            .unwrap_or(LockOnState::Searching))
    }

    /// Clear lock-on state for a shooter
    pub fn clear_lock_on_state(&self, shooter_id: ObjectId) -> GameLogicResult<()> {
        let mut states = self.lock_on_states.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire lock-on states: {}", e))
        })?;

        states.remove(&shooter_id);
        Ok(())
    }

    // ===== INTEGRATION METHODS (game system integration) =====

    /// Get objects within range using spatial partitioning
    fn get_objects_in_range(
        &self,
        position: &Coord3D,
        range: f32,
    ) -> GameLogicResult<Vec<ObjectId>> {
        let Some(partition) = crate::helpers::ThePartitionManager::get() else {
            return Ok(Vec::new());
        };

        Ok(partition.get_objects_in_range(position, range))
    }

    /// Get object position
    fn get_object_position(&self, object_id: ObjectId) -> GameLogicResult<Coord3D> {
        let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(object_id) else {
            return Err(GameLogicError::InvalidObject(object_id));
        };
        let obj = obj_arc.read().map_err(|_| {
            GameLogicError::Threading("Failed to lock object for position".to_string())
        })?;
        Ok(*obj.get_position())
    }

    /// Get object velocity
    fn get_object_velocity(&self, object_id: ObjectId) -> GameLogicResult<Coord3D> {
        let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(object_id) else {
            return Err(GameLogicError::InvalidObject(object_id));
        };
        let obj = obj_arc.read().map_err(|_| {
            GameLogicError::Threading("Failed to lock object for velocity".to_string())
        })?;

        let Some(physics) = obj.get_physics() else {
            return Ok(Coord3D::new(0.0, 0.0, 0.0));
        };

        let Ok(physics_guard) = physics.lock() else {
            return Ok(Coord3D::new(0.0, 0.0, 0.0));
        };
        let vel = physics_guard.get_velocity();
        Ok(Coord3D::new(vel.x, vel.y, vel.z))
    }

    /// Get object kind for anti-mask checking
    fn get_object_kind(&self, object_id: ObjectId) -> GameLogicResult<ObjectKind> {
        let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(object_id) else {
            return Ok(ObjectKind::Unknown);
        };
        let Ok(obj) = obj_arc.read() else {
            return Ok(ObjectKind::Unknown);
        };

        if obj.is_kind_of(KindOf::SmallMissile) {
            return Ok(ObjectKind::SmallMissile);
        }
        if obj.is_kind_of(KindOf::BallisticMissile) {
            return Ok(ObjectKind::BallisticMissile);
        }
        if obj.is_kind_of(KindOf::Projectile) {
            return Ok(ObjectKind::Projectile);
        }
        if obj.is_kind_of(KindOf::Mine) || obj.is_kind_of(KindOf::Demotrap) {
            return Ok(ObjectKind::Mine);
        }
        if obj.is_airborne_target() {
            if obj.is_kind_of(KindOf::Vehicle) {
                return Ok(ObjectKind::AirborneVehicle);
            }
            if obj.is_kind_of(KindOf::Infantry) {
                return Ok(ObjectKind::AirborneInfantry);
            }
            if obj.is_kind_of(KindOf::Parachute) {
                return Ok(ObjectKind::Parachute);
            }
            return Ok(ObjectKind::Unknown);
        }
        if obj.is_kind_of(KindOf::Aircraft) {
            return Ok(ObjectKind::AirborneVehicle);
        }

        Ok(ObjectKind::Ground)
    }

    /// Get object type name
    fn get_object_type(&self, object_id: ObjectId) -> GameLogicResult<String> {
        let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(object_id) else {
            return Err(GameLogicError::InvalidObject(object_id));
        };
        let obj = obj_arc
            .read()
            .map_err(|_| GameLogicError::Threading("Failed to lock object for type".to_string()))?;
        Ok(obj.get_template_name().to_string())
    }

    /// Perform raycast for line of sight
    fn raycast_line_of_sight(&self, from: &Coord3D, to: &Coord3D) -> GameLogicResult<bool> {
        let terrain = crate::terrain::get_terrain_logic();
        let Ok(guard) = terrain.read() else {
            return Ok(true);
        };
        Ok(guard.is_clear_line_of_sight(from, to))
    }

    /// Get engagement penalty (how many units are already attacking this target)
    fn get_engagement_penalty(&self, target_id: ObjectId) -> GameLogicResult<f32> {
        // This would check how many units are already attacking this target
        // Return 1.0 = no penalty, lower values = already engaged
        Ok(1.0)
    }
}

impl Default for WeaponTargetAcquisition {
    fn default() -> Self {
        Self::new()
    }
}

/// Object kind for anti-mask classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    AirborneVehicle,
    Ground,
    Projectile,
    SmallMissile,
    Mine,
    AirborneInfantry,
    BallisticMissile,
    Parachute,
    Unknown,
}

/// Global weapon target acquisition system
static WEAPON_TARGET_ACQUISITION: RwLock<Option<Arc<WeaponTargetAcquisition>>> = RwLock::new(None);

/// Initialize the global weapon target acquisition system
pub fn initialize_weapon_target_acquisition() -> GameLogicResult<()> {
    let mut system = WEAPON_TARGET_ACQUISITION.write().map_err(|e| {
        GameLogicError::Threading(format!(
            "Failed to acquire weapon target acquisition lock: {}",
            e
        ))
    })?;

    if system.is_none() {
        *system = Some(Arc::new(WeaponTargetAcquisition::new()));
    }

    Ok(())
}

/// Get reference to the global weapon target acquisition system
pub fn with_weapon_target_acquisition<F, R>(f: F) -> GameLogicResult<R>
where
    F: FnOnce(&WeaponTargetAcquisition) -> R,
{
    let system = WEAPON_TARGET_ACQUISITION.read().map_err(|e| {
        GameLogicError::Threading(format!(
            "Failed to acquire weapon target acquisition lock: {}",
            e
        ))
    })?;

    match system.as_ref() {
        Some(acquisition) => Ok(f(acquisition)),
        None => Err(GameLogicError::SystemNotInitialized(
            "Weapon target acquisition not initialized".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registered_target_object(
        id: ObjectId,
        kind_of: &str,
        airborne: bool,
    ) -> Arc<RwLock<crate::object::Object>> {
        let mut template =
            crate::common::DefaultThingTemplate::new(format!("TargetAcquisitionObject{}", id));
        let properties =
            std::collections::HashMap::from([("KindOf".to_string(), kind_of.to_string())]);
        template.parse_object_fields_from_ini(&properties);

        let object = crate::object::Object::new_with_id(
            Arc::new(template),
            id,
            crate::common::ObjectStatusMaskType::none(),
            None,
        )
        .expect("create target acquisition object");
        if airborne {
            object
                .write()
                .expect("target acquisition object write lock")
                .set_status(crate::common::ObjectStatusMaskType::AIRBORNE_TARGET, true);
        }
        crate::system::game_logic::get_game_logic()
            .lock()
            .unwrap()
            .register_object(object.clone())
            .expect("register target acquisition object");
        object
    }

    fn reset_target_objects() {
        crate::object::registry::OBJECT_REGISTRY.clear();
        crate::system::game_logic::get_game_logic()
            .lock()
            .unwrap()
            .clear_all_objects();
    }

    #[test]
    fn test_priority_class_ordering() {
        // Verify priority ordering (structure > siege > infantry > armor)
        assert!(TargetPriorityClass::Structure < TargetPriorityClass::Siege);
        assert!(TargetPriorityClass::Siege < TargetPriorityClass::Infantry);
        assert!(TargetPriorityClass::Infantry < TargetPriorityClass::Armor);
        assert!(TargetPriorityClass::Armor < TargetPriorityClass::Aircraft);
    }

    #[test]
    fn test_lock_on_state_progression() {
        let acquisition = WeaponTargetAcquisition::new();
        let shooter_id = 1;
        let target_id = 2;

        // Initial state should be Searching
        let state = acquisition.get_lock_on_state(shooter_id).unwrap();
        assert_eq!(state, LockOnState::Searching);

        // Start acquiring
        let state = acquisition
            .update_lock_on_state(shooter_id, Some(target_id))
            .unwrap();
        assert!(matches!(state, LockOnState::Acquiring { .. }));

        // Progress acquisition
        for _ in 0..35 {
            acquisition
                .update_lock_on_state(shooter_id, Some(target_id))
                .unwrap();
        }

        // Should be locked now
        let state = acquisition.get_lock_on_state(shooter_id).unwrap();
        assert!(matches!(state, LockOnState::Locked { .. }));
    }

    #[test]
    fn test_lock_on_target_change() {
        let acquisition = WeaponTargetAcquisition::new();
        let shooter_id = 1;
        let target_id_1 = 2;
        let target_id_2 = 3;

        // Start acquiring first target
        acquisition
            .update_lock_on_state(shooter_id, Some(target_id_1))
            .unwrap();

        // Switch to second target
        let state = acquisition
            .update_lock_on_state(shooter_id, Some(target_id_2))
            .unwrap();

        // Should restart acquisition
        assert!(matches!(
            state,
            LockOnState::Acquiring {
                target_id,
                progress: 0
            } if target_id == target_id_2
        ));
    }

    #[test]
    fn test_target_search_params() {
        let params = TargetSearchParams {
            shooter_pos: Coord3D::new(0.0, 0.0, 0.0),
            shooter_id: 1,
            max_range: 500.0,
            min_range: 0.0,
            anti_mask: WeaponAntiMask::new(WeaponAntiMask::GROUND),
            preferred_priorities: vec![TargetPriorityClass::Structure],
            require_line_of_sight: true,
            weapon_bonus: WeaponBonus::new(),
            projectile_speed: 100.0,
        };

        assert_eq!(params.max_range, 500.0);
        assert_eq!(
            params.preferred_priorities[0],
            TargetPriorityClass::Structure
        );
    }

    #[test]
    fn test_find_best_targets_zero_limit_skips_search() {
        let acquisition = WeaponTargetAcquisition::new();
        let params = TargetSearchParams {
            shooter_pos: Coord3D::new(0.0, 0.0, 0.0),
            shooter_id: 1,
            max_range: 500.0,
            min_range: 0.0,
            anti_mask: WeaponAntiMask::new(WeaponAntiMask::GROUND),
            preferred_priorities: vec![TargetPriorityClass::Structure],
            require_line_of_sight: true,
            weapon_bonus: WeaponBonus::new(),
            projectile_speed: 100.0,
        };

        let targets = acquisition.find_best_targets(&params, 100, 0).unwrap();

        assert!(targets.is_empty());
    }

    #[test]
    fn target_kind_matches_cpp_projectile_priority_order() {
        reset_target_objects();
        let acquisition = WeaponTargetAcquisition::new();

        registered_target_object(97_001, "PROJECTILE SMALL_MISSILE", false);
        registered_target_object(97_002, "PROJECTILE BALLISTIC_MISSILE", false);
        registered_target_object(97_003, "PROJECTILE", false);

        assert_eq!(
            acquisition.get_object_kind(97_001).unwrap(),
            ObjectKind::SmallMissile
        );
        assert_eq!(
            acquisition.get_object_kind(97_002).unwrap(),
            ObjectKind::BallisticMissile
        );
        assert_eq!(
            acquisition.get_object_kind(97_003).unwrap(),
            ObjectKind::Projectile
        );

        reset_target_objects();
    }

    #[test]
    fn target_kind_matches_cpp_mine_and_airborne_branches() {
        reset_target_objects();
        let acquisition = WeaponTargetAcquisition::new();

        registered_target_object(97_011, "DEMOTRAP", false);
        registered_target_object(97_012, "PARACHUTE", true);
        registered_target_object(97_013, "VEHICLE", true);
        registered_target_object(97_014, "INFANTRY", true);

        assert_eq!(
            acquisition.get_object_kind(97_011).unwrap(),
            ObjectKind::Mine
        );
        assert_eq!(
            acquisition.get_object_kind(97_012).unwrap(),
            ObjectKind::Parachute
        );
        assert_eq!(
            acquisition.get_object_kind(97_013).unwrap(),
            ObjectKind::AirborneVehicle
        );
        assert_eq!(
            acquisition.get_object_kind(97_014).unwrap(),
            ObjectKind::AirborneInfantry
        );

        reset_target_objects();
    }

    #[test]
    fn target_anti_mask_matches_cpp_mine_and_missile_semantics() {
        reset_target_objects();
        let acquisition = WeaponTargetAcquisition::new();

        registered_target_object(97_021, "DEMOTRAP", false);
        registered_target_object(97_022, "PROJECTILE SMALL_MISSILE", false);

        assert!(
            acquisition
                .can_weapon_attack_target(&WeaponAntiMask::new(WeaponAntiMask::GROUND), 97_021,)
                .unwrap(),
            "C++ treats mines and demo traps as mine|ground victims"
        );
        assert!(acquisition
            .can_weapon_attack_target(&WeaponAntiMask::new(WeaponAntiMask::MINE), 97_021)
            .unwrap());
        assert!(
            !acquisition
                .can_weapon_attack_target(&WeaponAntiMask::new(WeaponAntiMask::PROJECTILE), 97_022,)
                .unwrap(),
            "C++ small-missile victims require AntiSmallMissile, not generic AntiProjectile"
        );
        assert!(acquisition
            .can_weapon_attack_target(&WeaponAntiMask::new(WeaponAntiMask::SMALL_MISSILE), 97_022,)
            .unwrap());

        reset_target_objects();
    }
}
