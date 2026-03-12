//! Advanced Targeting System
//!
//! This module provides sophisticated target acquisition, prioritization,
//! and tracking capabilities for weapon systems.

use crate::common::{Coord3D, CoordOrigin, KindOf};
use crate::helpers::ThePartitionManager;
use crate::object::{registry::OBJECT_REGISTRY, ObjectId};
use crate::weapon::{WeaponAntiMask, WeaponTemplate, INVALID_OBJECT_ID};
use crate::{GameLogicError, GameLogicResult};

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

/// Target priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TargetPriority {
    /// Ignore this target
    Ignore = 0,
    /// Low priority target
    Low = 1,
    /// Normal priority target
    Normal = 2,
    /// High priority target
    High = 3,
    /// Critical priority target
    Critical = 4,
    /// Emergency priority target (immediate threat)
    Emergency = 5,
}

/// Target classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetClass {
    /// Infantry units
    Infantry,
    /// Light vehicles
    LightVehicle,
    /// Heavy armor
    HeavyArmor,
    /// Aircraft
    Aircraft,
    /// Naval vessels
    Naval,
    /// Structures/Buildings
    Structure,
    /// Defensive structures
    Defense,
    /// Support units
    Support,
    /// Command units
    Command,
    /// Special weapons
    SpecialWeapon,
}

/// Target information
#[derive(Debug, Clone)]
pub struct TargetInfo {
    /// Target object ID
    pub object_id: ObjectId,
    /// Current position
    pub position: Coord3D,
    /// Velocity vector
    pub velocity: Coord3D,
    /// Target classification
    pub target_class: TargetClass,
    /// Current health percentage
    pub health_percentage: f32,
    /// Threat level to this unit
    pub threat_level: f32,
    /// Distance from targeting unit
    pub distance: f32,
    /// Line of sight availability
    pub line_of_sight: bool,
    /// Time since last update
    pub last_updated: f32,
    /// Whether target is currently engaged by other units
    pub engaged_by_others: HashSet<ObjectId>,
    /// Target priority
    pub priority: TargetPriority,
    /// Predicted future position
    pub predicted_position: Option<Coord3D>,
    /// Confidence in target data (0.0 to 1.0)
    pub confidence: f32,
}

/// Sensor types for target detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensorType {
    /// Visual/optical detection
    Visual,
    /// Radar detection
    Radar,
    /// Infrared/thermal detection
    Infrared,
    /// Motion detection
    Motion,
    /// Sound detection
    Acoustic,
    /// Electronic surveillance
    Electronic,
}

/// Sensor configuration
#[derive(Debug, Clone)]
pub struct SensorConfig {
    /// Sensor type
    pub sensor_type: SensorType,
    /// Detection range
    pub range: f32,
    /// Field of view angle (radians)
    pub fov_angle: f32,
    /// Minimum detection size
    pub min_detection_size: f32,
    /// Weather degradation factor
    pub weather_degradation: f32,
    /// Resolution/accuracy factor
    pub resolution: f32,
    /// Whether sensor can penetrate stealth
    pub stealth_penetration: bool,
}

/// Target acquisition parameters
#[derive(Debug, Clone)]
pub struct AcquisitionParams {
    /// Maximum engagement range
    pub max_range: f32,
    /// Minimum engagement range
    pub min_range: f32,
    /// Field of fire constraints
    pub firing_arc: Option<FiringArc>,
    /// Preferred target classes
    pub preferred_targets: Vec<TargetClass>,
    /// Avoided target classes
    pub avoided_targets: Vec<TargetClass>,
    /// Whether to engage moving targets
    pub engage_moving: bool,
    /// Whether to engage stationary targets
    pub engage_stationary: bool,
    /// Minimum target health to engage
    pub min_target_health: f32,
    /// Maximum simultaneous targets
    pub max_concurrent_targets: u32,
}

/// Firing arc constraints
#[derive(Debug, Clone)]
pub struct FiringArc {
    /// Center direction (radians)
    pub center_direction: f32,
    /// Total arc width (radians)
    pub arc_width: f32,
    /// Minimum elevation (radians)
    pub min_elevation: f32,
    /// Maximum elevation (radians)
    pub max_elevation: f32,
}

/// Advanced targeting system
pub struct TargetingSystem {
    /// Available sensors
    sensors: Vec<SensorConfig>,
    /// Known targets
    known_targets: RwLock<HashMap<ObjectId, TargetInfo>>,
    /// Current primary target
    primary_target: RwLock<Option<ObjectId>>,
    /// Target acquisition parameters
    acquisition_params: AcquisitionParams,
    /// Target priority weights
    priority_weights: TargetPriorityWeights,
}

/// Weights for target priority calculation
#[derive(Debug, Clone)]
pub struct TargetPriorityWeights {
    /// Weight for threat level
    pub threat_weight: f32,
    /// Weight for distance (closer is better)
    pub distance_weight: f32,
    /// Weight for target health (wounded targets are easier)
    pub health_weight: f32,
    /// Weight for target value (high-value targets)
    pub value_weight: f32,
    /// Weight for ease of engagement
    pub engagement_weight: f32,
}

impl TargetingSystem {
    /// Create a new targeting system
    pub fn new(sensors: Vec<SensorConfig>, acquisition_params: AcquisitionParams) -> Self {
        Self {
            sensors,
            known_targets: RwLock::new(HashMap::new()),
            primary_target: RwLock::new(None),
            acquisition_params,
            priority_weights: TargetPriorityWeights::default(),
        }
    }

    /// Update the targeting system for one frame
    pub fn update(
        &self,
        owner_position: &Coord3D,
        owner_facing: f32,
        delta_time: f32,
    ) -> GameLogicResult<()> {
        // Update sensor scans
        self.update_sensor_data(owner_position, owner_facing, delta_time)?;

        // Age existing target data
        self.age_target_data(delta_time)?;

        // Evaluate target priorities
        self.evaluate_target_priorities(owner_position)?;

        // Select primary target
        self.select_primary_target()?;

        Ok(())
    }

    /// Update sensor data and detect new targets
    fn update_sensor_data(
        &self,
        owner_position: &Coord3D,
        owner_facing: f32,
        delta_time: f32,
    ) -> GameLogicResult<()> {
        for sensor in &self.sensors {
            let detected_objects = self.scan_with_sensor(sensor, owner_position, owner_facing)?;

            let mut targets = self.known_targets.write().map_err(|e| {
                GameLogicError::Threading(format!("Failed to acquire targets lock: {}", e))
            })?;

            for object_info in detected_objects {
                let object_id = object_info.object_id;

                match targets.get_mut(&object_id) {
                    Some(existing_target) => {
                        // Update existing target
                        self.update_target_info(existing_target, &object_info, delta_time);
                    }
                    None => {
                        // New target detected
                        if self.is_valid_target(&object_info)? {
                            targets.insert(object_id, object_info);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Perform sensor scan
    fn scan_with_sensor(
        &self,
        sensor: &SensorConfig,
        owner_position: &Coord3D,
        owner_facing: f32,
    ) -> GameLogicResult<Vec<TargetInfo>> {
        let mut detected = Vec::new();

        // Get potential targets in range
        let potential_targets = self.get_objects_in_range(owner_position, sensor.range)?;

        for object_id in potential_targets {
            let object_position = self.get_object_position(object_id)?;
            let distance = owner_position.distance(object_position);

            // Check if target is within sensor parameters
            if self.is_within_sensor_fov(owner_position, owner_facing, &object_position, sensor)? {
                // Check if sensor can detect this type of object
                if self.can_sensor_detect_object(sensor, object_id)? {
                    let target_info =
                        self.create_target_info(object_id, &object_position, distance, sensor)?;

                    detected.push(target_info);
                }
            }
        }

        Ok(detected)
    }

    /// Check if object is within sensor field of view
    fn is_within_sensor_fov(
        &self,
        sensor_pos: &Coord3D,
        sensor_facing: f32,
        target_pos: &Coord3D,
        sensor: &SensorConfig,
    ) -> GameLogicResult<bool> {
        // Calculate angle to target
        let dx = target_pos.x - sensor_pos.x;
        let dy = target_pos.y - sensor_pos.y;
        let angle_to_target = dy.atan2(dx);

        // Normalize angle difference
        let mut angle_diff = angle_to_target - sensor_facing;
        while angle_diff > std::f32::consts::PI {
            angle_diff -= 2.0 * std::f32::consts::PI;
        }
        while angle_diff < -std::f32::consts::PI {
            angle_diff += 2.0 * std::f32::consts::PI;
        }

        // Check if within field of view
        Ok(angle_diff.abs() <= sensor.fov_angle * 0.5)
    }

    /// Check if sensor can detect specific object
    fn can_sensor_detect_object(
        &self,
        sensor: &SensorConfig,
        object_id: ObjectId,
    ) -> GameLogicResult<bool> {
        let object_info = self.get_object_detection_info(object_id)?;

        // Check size requirements
        if object_info.detection_size < sensor.min_detection_size {
            return Ok(false);
        }

        // Check stealth vs sensor capabilities
        if object_info.is_stealthed && !sensor.stealth_penetration {
            return Ok(false);
        }

        // Sensor-specific detection logic
        match sensor.sensor_type {
            SensorType::Visual => Ok(!object_info.is_invisible),
            SensorType::Radar => Ok(!object_info.is_radar_stealthed),
            SensorType::Infrared => Ok(object_info.heat_signature > 0.1),
            SensorType::Motion => Ok(object_info.is_moving),
            SensorType::Acoustic => Ok(object_info.noise_level > 0.1),
            SensorType::Electronic => Ok(object_info.electronic_signature > 0.1),
        }
    }

    /// Create target info from detected object
    fn create_target_info(
        &self,
        object_id: ObjectId,
        position: &Coord3D,
        distance: f32,
        sensor: &SensorConfig,
    ) -> GameLogicResult<TargetInfo> {
        let object_data = self.get_object_data(object_id)?;
        let velocity = self.get_object_velocity(object_id)?;

        Ok(TargetInfo {
            object_id,
            position: *position,
            velocity,
            target_class: object_data.target_class,
            health_percentage: object_data.health_percentage,
            threat_level: self.calculate_threat_level(object_id)?,
            distance,
            line_of_sight: self.check_line_of_sight(position)?,
            last_updated: 0.0, // Current frame
            engaged_by_others: HashSet::new(),
            priority: TargetPriority::Normal,
            predicted_position: None,
            confidence: sensor.resolution,
        })
    }

    /// Update existing target information
    fn update_target_info(&self, target: &mut TargetInfo, new_info: &TargetInfo, delta_time: f32) {
        // Update position and velocity
        target.position = new_info.position;
        target.velocity = new_info.velocity;

        // Update health
        target.health_percentage = new_info.health_percentage;

        // Update distance
        target.distance = new_info.distance;

        // Update line of sight
        target.line_of_sight = new_info.line_of_sight;

        // Reset age
        target.last_updated = 0.0;

        // Update confidence based on consistency
        let position_consistency = if target.predicted_position.is_some() {
            let predicted = target.predicted_position.unwrap();
            let actual_distance = predicted.distance(new_info.position);
            (1.0 - (actual_distance / 10.0).min(1.0)).max(0.1)
        } else {
            1.0
        };

        target.confidence =
            (target.confidence * 0.9 + new_info.confidence * 0.1) * position_consistency;

        // Predict future position
        target.predicted_position = Some(Coord3D::new(
            target.position.x + target.velocity.x * 1.0, // 1 second prediction
            target.position.y + target.velocity.y * 1.0,
            target.position.z + target.velocity.z * 1.0,
        ));
    }

    /// Age target data and remove stale targets
    fn age_target_data(&self, delta_time: f32) -> GameLogicResult<()> {
        let mut targets = self.known_targets.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire targets lock: {}", e))
        })?;

        let mut to_remove = Vec::new();

        for (object_id, target) in targets.iter_mut() {
            target.last_updated += delta_time;

            // Reduce confidence over time
            let age_factor = (1.0 - target.last_updated / 10.0).max(0.0);
            target.confidence *= age_factor;

            // Remove targets that are too old or have zero confidence
            if target.last_updated > 15.0 || target.confidence < 0.1 {
                to_remove.push(*object_id);
            }
        }

        for object_id in to_remove {
            targets.remove(&object_id);
        }

        Ok(())
    }

    /// Evaluate and assign priorities to all known targets
    fn evaluate_target_priorities(&self, owner_position: &Coord3D) -> GameLogicResult<()> {
        let mut targets = self.known_targets.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire targets lock: {}", e))
        })?;

        for target in targets.values_mut() {
            target.priority = self.calculate_target_priority(target, owner_position)?;
        }

        Ok(())
    }

    /// Calculate priority score for a target
    fn calculate_target_priority(
        &self,
        target: &TargetInfo,
        owner_position: &Coord3D,
    ) -> GameLogicResult<TargetPriority> {
        let mut score = 0.0;

        // Threat level contribution
        score += target.threat_level * self.priority_weights.threat_weight;

        // Distance contribution (closer is better)
        let distance_score = 1.0 - (target.distance / self.acquisition_params.max_range).min(1.0);
        score += distance_score * self.priority_weights.distance_weight;

        // Health contribution (wounded targets are easier)
        let health_score = 1.0 - target.health_percentage;
        score += health_score * self.priority_weights.health_weight;

        // Target value contribution
        let value_score = self.get_target_value_score(target.target_class);
        score += value_score * self.priority_weights.value_weight;

        // Engagement ease contribution
        let engagement_score = self.calculate_engagement_ease(target, owner_position)?;
        score += engagement_score * self.priority_weights.engagement_weight;

        // Line of sight bonus
        if target.line_of_sight {
            score *= 1.2;
        }

        // Confidence modifier
        score *= target.confidence;

        // Convert score to priority level
        Ok(match score {
            s if s >= 4.0 => TargetPriority::Emergency,
            s if s >= 3.0 => TargetPriority::Critical,
            s if s >= 2.0 => TargetPriority::High,
            s if s >= 1.0 => TargetPriority::Normal,
            s if s >= 0.5 => TargetPriority::Low,
            _ => TargetPriority::Ignore,
        })
    }

    /// Select the primary target based on priorities
    fn select_primary_target(&self) -> GameLogicResult<()> {
        let targets = self.known_targets.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire targets lock: {}", e))
        })?;

        let mut best_target = None;
        let mut best_priority = TargetPriority::Ignore;
        let mut best_distance = f32::MAX;

        for (object_id, target) in targets.iter() {
            if target.priority > best_priority
                || (target.priority == best_priority && target.distance < best_distance)
            {
                best_target = Some(*object_id);
                best_priority = target.priority;
                best_distance = target.distance;
            }
        }

        let mut primary = self.primary_target.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire primary target lock: {}", e))
        })?;

        *primary = best_target;

        Ok(())
    }

    /// Check if object is a valid target
    fn is_valid_target(&self, target: &TargetInfo) -> GameLogicResult<bool> {
        // Check distance constraints
        if target.distance < self.acquisition_params.min_range
            || target.distance > self.acquisition_params.max_range
        {
            return Ok(false);
        }

        // Check preferred/avoided target classes
        if !self.acquisition_params.preferred_targets.is_empty()
            && !self
                .acquisition_params
                .preferred_targets
                .contains(&target.target_class)
        {
            return Ok(false);
        }

        if self
            .acquisition_params
            .avoided_targets
            .contains(&target.target_class)
        {
            return Ok(false);
        }

        // Check movement preference
        let is_moving = target.velocity.distance(Coord3D::new(0.0, 0.0, 0.0)) > 1.0;
        if (is_moving && !self.acquisition_params.engage_moving)
            || (!is_moving && !self.acquisition_params.engage_stationary)
        {
            return Ok(false);
        }

        // Check minimum health requirement
        if target.health_percentage < self.acquisition_params.min_target_health {
            return Ok(false);
        }

        Ok(true)
    }

    /// Get target value score based on class
    fn get_target_value_score(&self, target_class: TargetClass) -> f32 {
        match target_class {
            TargetClass::Command => 5.0,
            TargetClass::SpecialWeapon => 4.5,
            TargetClass::Defense => 4.0,
            TargetClass::HeavyArmor => 3.5,
            TargetClass::Aircraft => 3.0,
            TargetClass::LightVehicle => 2.5,
            TargetClass::Support => 2.0,
            TargetClass::Structure => 1.5,
            TargetClass::Infantry => 1.0,
            TargetClass::Naval => 2.5,
        }
    }

    /// Calculate how easy it is to engage this target
    fn calculate_engagement_ease(
        &self,
        target: &TargetInfo,
        owner_position: &Coord3D,
    ) -> GameLogicResult<f32> {
        let mut ease: f32 = 1.0;

        // Line of sight bonus
        if target.line_of_sight {
            ease *= 1.5;
        }

        // Movement penalty for fast targets
        let speed = target.velocity.distance(Coord3D::new(0.0, 0.0, 0.0));
        if speed > 20.0 {
            ease *= 0.7;
        }

        // Engagement by others penalty
        if !target.engaged_by_others.is_empty() {
            ease *= 0.8;
        }

        // Firing arc check
        if let Some(firing_arc) = &self.acquisition_params.firing_arc {
            if !self.is_within_firing_arc(owner_position, &target.position, firing_arc)? {
                ease *= 0.3;
            }
        }

        Ok(ease.max(0.1))
    }

    /// Check if target is within firing arc
    fn is_within_firing_arc(
        &self,
        owner_position: &Coord3D,
        target_position: &Coord3D,
        firing_arc: &FiringArc,
    ) -> GameLogicResult<bool> {
        let dx = target_position.x - owner_position.x;
        let dy = target_position.y - owner_position.y;
        let dz = target_position.z - owner_position.z;

        let horizontal_distance = (dx * dx + dy * dy).sqrt();
        let angle_to_target = dy.atan2(dx);
        let elevation = dz.atan2(horizontal_distance);

        // Check horizontal arc
        let mut angle_diff = angle_to_target - firing_arc.center_direction;
        while angle_diff > std::f32::consts::PI {
            angle_diff -= 2.0 * std::f32::consts::PI;
        }
        while angle_diff < -std::f32::consts::PI {
            angle_diff += 2.0 * std::f32::consts::PI;
        }

        let within_horizontal = angle_diff.abs() <= firing_arc.arc_width * 0.5;

        // Check elevation
        let within_elevation =
            elevation >= firing_arc.min_elevation && elevation <= firing_arc.max_elevation;

        Ok(within_horizontal && within_elevation)
    }

    /// Get current primary target
    pub fn get_primary_target(&self) -> GameLogicResult<Option<ObjectId>> {
        let primary = self.primary_target.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire primary target lock: {}", e))
        })?;
        Ok(*primary)
    }

    /// Get information about a specific target
    pub fn get_target_info(&self, object_id: ObjectId) -> GameLogicResult<Option<TargetInfo>> {
        let targets = self.known_targets.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire targets lock: {}", e))
        })?;
        Ok(targets.get(&object_id).cloned())
    }

    /// Get all known targets
    pub fn get_all_targets(&self) -> GameLogicResult<Vec<TargetInfo>> {
        let targets = self.known_targets.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire targets lock: {}", e))
        })?;
        Ok(targets.values().cloned().collect())
    }

    /// Manually add a target (from external intelligence)
    pub fn add_manual_target(&self, target_info: TargetInfo) -> GameLogicResult<()> {
        let mut targets = self.known_targets.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire targets lock: {}", e))
        })?;
        targets.insert(target_info.object_id, target_info);
        Ok(())
    }

    /// Remove a specific target
    pub fn remove_target(&self, object_id: ObjectId) -> GameLogicResult<()> {
        let mut targets = self.known_targets.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire targets lock: {}", e))
        })?;
        targets.remove(&object_id);

        // Clear primary target if it was removed
        let mut primary = self.primary_target.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire primary target lock: {}", e))
        })?;
        if *primary == Some(object_id) {
            *primary = None;
        }

        Ok(())
    }

    fn get_objects_in_range(
        &self,
        position: &Coord3D,
        range: f32,
    ) -> GameLogicResult<Vec<ObjectId>> {
        if let Some(partition) = ThePartitionManager::get() {
            return Ok(partition.get_objects_in_range(position, range));
        }
        Ok(Vec::new())
    }

    fn get_object_position(&self, object_id: ObjectId) -> GameLogicResult<Coord3D> {
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            return Err(GameLogicError::InvalidObject(object_id));
        };
        let guard = obj_arc
            .read()
            .map_err(|_| GameLogicError::Threading("Target position lock failed".to_string()))?;
        Ok(*guard.get_position())
    }

    fn get_object_velocity(&self, object_id: ObjectId) -> GameLogicResult<Coord3D> {
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            return Err(GameLogicError::InvalidObject(object_id));
        };
        let guard = obj_arc
            .read()
            .map_err(|_| GameLogicError::Threading("Target velocity lock failed".to_string()))?;
        if let Some(physics_arc) = guard.get_physics() {
            if let Ok(physics) = physics_arc.lock() {
                let vel = physics.get_velocity();
                return Ok(Coord3D::new(vel.x, vel.y, vel.z));
            }
        }
        Ok(Coord3D::new(0.0, 0.0, 0.0))
    }

    fn get_object_detection_info(
        &self,
        object_id: ObjectId,
    ) -> GameLogicResult<ObjectDetectionInfo> {
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            return Err(GameLogicError::InvalidObject(object_id));
        };
        let guard = obj_arc
            .read()
            .map_err(|_| GameLogicError::Threading("Target detection lock failed".to_string()))?;
        let detection_size = guard.get_geometry_info().get_bounding_circle_radius();
        let velocity = if let Some(physics_arc) = guard.get_physics() {
            if let Ok(phys) = physics_arc.lock() {
                phys.get_velocity()
            } else {
                Coord3D::origin()
            }
        } else {
            Coord3D::origin()
        };
        let is_moving = velocity.length() > 0.1;

        Ok(ObjectDetectionInfo {
            detection_size,
            is_stealthed: guard.is_stealthed(),
            is_invisible: false,
            is_radar_stealthed: guard.is_stealthed(),
            heat_signature: if guard.is_stealthed() { 0.2 } else { 1.0 },
            is_moving,
            noise_level: if is_moving { 1.0 } else { 0.2 },
            electronic_signature: 1.0,
        })
    }

    fn get_object_data(&self, object_id: ObjectId) -> GameLogicResult<ObjectData> {
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            return Err(GameLogicError::InvalidObject(object_id));
        };
        let guard = obj_arc
            .read()
            .map_err(|_| GameLogicError::Threading("Target data lock failed".to_string()))?;
        let health_percentage = guard.get_health_percentage().clamp(0.0, 1.0);
        let target_class = if guard.is_kind_of(crate::common::KindOf::Defense) {
            TargetClass::Defense
        } else if guard.is_kind_of(crate::common::KindOf::Structure)
            || guard.is_kind_of(crate::common::KindOf::Building)
        {
            TargetClass::Structure
        } else if guard.is_kind_of(crate::common::KindOf::CommandCenter) {
            TargetClass::Command
        } else if guard.is_kind_of(crate::common::KindOf::Aircraft)
            || guard.is_kind_of(crate::common::KindOf::Drone)
        {
            TargetClass::Aircraft
        } else if guard.is_kind_of(crate::common::KindOf::AircraftCarrier) {
            TargetClass::Naval
        } else if guard.is_kind_of(crate::common::KindOf::Infantry) {
            TargetClass::Infantry
        } else if guard.is_kind_of(crate::common::KindOf::Vehicle) {
            TargetClass::HeavyArmor
        } else {
            TargetClass::Support
        };

        Ok(ObjectData {
            target_class,
            health_percentage,
        })
    }

    fn check_line_of_sight(&self, _position: &Coord3D) -> GameLogicResult<bool> {
        Ok(true)
    }

    fn calculate_threat_level(&self, _object_id: ObjectId) -> GameLogicResult<f32> {
        Ok(1.0)
    }
}

/// Object detection information
#[derive(Debug, Default)]
struct ObjectDetectionInfo {
    pub detection_size: f32,
    pub is_stealthed: bool,
    pub is_invisible: bool,
    pub is_radar_stealthed: bool,
    pub heat_signature: f32,
    pub is_moving: bool,
    pub noise_level: f32,
    pub electronic_signature: f32,
}

/// Object data for targeting
#[derive(Debug, Default)]
struct ObjectData {
    pub target_class: TargetClass,
    pub health_percentage: f32,
}

impl Default for TargetClass {
    fn default() -> Self {
        TargetClass::Infantry
    }
}

impl Default for TargetPriorityWeights {
    fn default() -> Self {
        Self {
            threat_weight: 2.0,
            distance_weight: 1.0,
            health_weight: 0.5,
            value_weight: 1.5,
            engagement_weight: 1.2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_priority_calculation() {
        let targeting = TargetingSystem::new(Vec::new(), AcquisitionParams::default());
        let owner_pos = Coord3D::new(0.0, 0.0, 0.0);

        let target = TargetInfo {
            object_id: 1,
            position: Coord3D::new(50.0, 0.0, 0.0),
            velocity: Coord3D::new(0.0, 0.0, 0.0),
            target_class: TargetClass::HeavyArmor,
            health_percentage: 0.8,
            threat_level: 2.0,
            distance: 50.0,
            line_of_sight: true,
            last_updated: 0.0,
            engaged_by_others: HashSet::new(),
            priority: TargetPriority::Normal,
            predicted_position: None,
            confidence: 1.0,
        };

        let priority = targeting
            .calculate_target_priority(&target, &owner_pos)
            .unwrap();
        assert!(priority >= TargetPriority::Normal);
    }

    #[test]
    fn test_sensor_fov() {
        let targeting = TargetingSystem::new(Vec::new(), AcquisitionParams::default());
        let sensor = SensorConfig {
            sensor_type: SensorType::Visual,
            range: 100.0,
            fov_angle: std::f32::consts::PI / 2.0, // 90 degrees
            min_detection_size: 1.0,
            weather_degradation: 1.0,
            resolution: 1.0,
            stealth_penetration: false,
        };

        let sensor_pos = Coord3D::new(0.0, 0.0, 0.0);
        let sensor_facing = 0.0; // Facing east
        let target_pos = Coord3D::new(10.0, 0.0, 0.0); // Directly east

        let within_fov = targeting
            .is_within_sensor_fov(&sensor_pos, sensor_facing, &target_pos, &sensor)
            .unwrap();

        assert!(within_fov);
    }
}

impl Default for AcquisitionParams {
    fn default() -> Self {
        Self {
            max_range: 500.0,
            min_range: 0.0,
            firing_arc: None,
            preferred_targets: Vec::new(),
            avoided_targets: Vec::new(),
            engage_moving: true,
            engage_stationary: true,
            min_target_health: 0.0,
            max_concurrent_targets: 1,
        }
    }
}
