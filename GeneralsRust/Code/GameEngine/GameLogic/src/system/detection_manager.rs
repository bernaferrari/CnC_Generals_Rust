//! Detection Manager System
//!
//! Manages detection capabilities and rules for detecting stealth objects:
//! - Per-object detection strength (how good at detecting stealth)
//! - Detection modifiers (distance, unit type, special abilities)
//! - Detection vs stealth comparison logic
//! - Per-player detection capability tracking
//!
//! Works with StealthManager to determine if stealthed objects can be detected.

use crate::common::{ObjectID, UnsignedInt};
use log::{debug, trace, warn};
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;

/// Maximum number of players in game (0-7)
const MAX_PLAYER_COUNT: usize = 8;

/// Detection strength value (0.0-100.0)
/// Higher value = better at detecting stealth
#[derive(Debug, Clone, Copy)]
pub struct DetectionStrength(f32);

impl DetectionStrength {
    /// Create new detection strength value
    pub fn new(value: f32) -> Self {
        // Clamp to 0.0-100.0 range
        Self(value.max(0.0).min(100.0))
    }

    /// Get raw detection strength value
    pub fn value(&self) -> f32 {
        self.0
    }

    /// Standard detector (60.0) - typical infantry unit
    pub fn standard_detector() -> Self {
        Self(60.0)
    }

    /// Strong detector (90.0) - advanced detection unit
    pub fn strong_detector() -> Self {
        Self(90.0)
    }

    /// Weak detector (30.0) - basic detection capability
    pub fn weak_detector() -> Self {
        Self(30.0)
    }

    /// No detection (0.0) - completely blind to stealth
    pub fn none() -> Self {
        Self(0.0)
    }
}

/// Detection modifiers that affect detection effectiveness
#[derive(Debug, Clone, Copy)]
pub struct DetectionModifier {
    /// Distance modifier (0.0-1.0): farther = weaker detection
    pub distance_factor: f32,

    /// Unit type modifier (0.0-1.0): some units are harder to detect than others
    pub unit_type_factor: f32,

    /// Movement modifier (0.0-1.0): moving units easier to detect than stationary
    pub movement_factor: f32,

    /// Special modifier (0.0-1.0): special abilities/tech effects
    pub special_factor: f32,
}

impl Default for DetectionModifier {
    fn default() -> Self {
        Self {
            distance_factor: 1.0,
            unit_type_factor: 1.0,
            movement_factor: 1.0,
            special_factor: 1.0,
        }
    }
}

impl DetectionModifier {
    /// Create new detection modifier
    pub fn new(
        distance_factor: f32,
        unit_type_factor: f32,
        movement_factor: f32,
        special_factor: f32,
    ) -> Self {
        Self {
            distance_factor: distance_factor.max(0.0).min(1.0),
            unit_type_factor: unit_type_factor.max(0.0).min(1.0),
            movement_factor: movement_factor.max(0.0).min(1.0),
            special_factor: special_factor.max(0.0).min(1.0),
        }
    }

    /// Calculate combined modifier (multiplicative effect)
    pub fn combined_factor(&self) -> f32 {
        self.distance_factor * self.unit_type_factor * self.movement_factor * self.special_factor
    }
}

/// Per-object detection capability tracking
#[derive(Debug, Clone)]
struct ObjectDetectionState {
    /// Object ID being tracked
    object_id: ObjectID,

    /// Base detection strength (0.0-100.0)
    detection_strength: DetectionStrength,

    /// Last frame detection was updated
    last_update_frame: UnsignedInt,
}

impl ObjectDetectionState {
    /// Create new detection state for object
    fn new(object_id: ObjectID) -> Self {
        Self {
            object_id,
            detection_strength: DetectionStrength::none(),
            last_update_frame: 0,
        }
    }
}

/// Detection Manager singleton
///
/// Manages detection capabilities for all game objects. Provides detection checks
/// against stealth. Thread-safe access via mutex.
pub struct DetectionManager {
    /// Per-object detection capability tracking
    object_detection: HashMap<ObjectID, ObjectDetectionState>,

    /// Last frame detection was updated
    last_update_frame: UnsignedInt,
}

impl DetectionManager {
    /// Create new DetectionManager
    pub fn new() -> Self {
        Self {
            object_detection: HashMap::new(),
            last_update_frame: 0,
        }
    }

    /// Register object for detection tracking
    pub fn register_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        if self.object_detection.contains_key(&object_id) {
            return Err(format!("Object {} already registered", object_id));
        }
        self.object_detection
            .insert(object_id, ObjectDetectionState::new(object_id));
        trace!("Registered object {} for detection tracking", object_id);
        Ok(())
    }

    /// Unregister object from detection tracking
    pub fn unregister_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        if self.object_detection.remove(&object_id).is_some() {
            trace!("Unregistered object {} from detection tracking", object_id);
            Ok(())
        } else {
            Err(format!("Object {} not registered", object_id))
        }
    }

    /// Set detection strength for object
    pub fn set_detection_strength(
        &mut self,
        object_id: ObjectID,
        strength: DetectionStrength,
    ) -> Result<(), String> {
        let state = self
            .object_detection
            .get_mut(&object_id)
            .ok_or_else(|| format!("Object {} not registered", object_id))?;

        state.detection_strength = strength;
        trace!(
            "Set detection strength for object {}: {:.1}",
            object_id,
            strength.value()
        );
        Ok(())
    }

    /// Get detection strength for object
    pub fn get_detection_strength(&self, object_id: ObjectID) -> Result<DetectionStrength, String> {
        let state = self
            .object_detection
            .get(&object_id)
            .ok_or_else(|| format!("Object {} not registered", object_id))?;

        Ok(state.detection_strength)
    }

    /// Check if detector can detect stealth
    ///
    /// Compares detector's detection strength (modified by modifiers) against
    /// target's stealth strength. Returns true if detection > stealth.
    pub fn can_detect_stealth(
        &self,
        detector_id: ObjectID,
        stealth_strength: f32,
        modifier: DetectionModifier,
    ) -> Result<bool, String> {
        let detector_state = self
            .object_detection
            .get(&detector_id)
            .ok_or_else(|| format!("Detector object {} not registered", detector_id))?;

        // Apply modifiers to detection strength
        let modified_detection =
            detector_state.detection_strength.value() * modifier.combined_factor();

        // Stealth is detected if modified detection exceeds stealth strength
        let can_detect = modified_detection > stealth_strength;

        trace!(
            "Detection check: detector {} (strength {:.1}, modified {:.1}) vs stealth {:.1} = {}",
            detector_id,
            detector_state.detection_strength.value(),
            modified_detection,
            stealth_strength,
            can_detect
        );

        Ok(can_detect)
    }

    /// Check detection with full parameters
    ///
    /// More detailed version that returns the effective detection value for logging/debugging.
    pub fn get_detection_effectiveness(
        &self,
        detector_id: ObjectID,
        modifier: DetectionModifier,
    ) -> Result<f32, String> {
        let detector_state = self
            .object_detection
            .get(&detector_id)
            .ok_or_else(|| format!("Detector object {} not registered", detector_id))?;

        Ok(detector_state.detection_strength.value() * modifier.combined_factor())
    }

    /// Calculate if a stealth unit is revealed based on distance
    ///
    /// Returns true if the unit is too close and should be revealed (distance < reveal_distance)
    /// This is used by StealthUpdate to break stealth when unit approaches hostile targets.
    pub fn calculate_reveal_distance_factor(distance: f32, reveal_distance: f32) -> bool {
        if reveal_distance <= 0.0 {
            return distance <= 0.0;
        }
        distance < reveal_distance
    }

    /// Check if distance triggers stealth reveal with stealth strength modifier
    ///
    /// More comprehensive check that considers both distance and stealth strength.
    /// Distance advantage can partially overcome stealth if very close.
    pub fn check_distance_reveal_with_strength(
        distance: f32,
        reveal_distance: f32,
        stealth_strength: f32,
    ) -> bool {
        if reveal_distance <= 0.0 {
            return distance <= 0.0;
        }

        if distance < reveal_distance {
            return true;
        }
        if distance <= reveal_distance {
            return false;
        }

        // At extended range, weak stealth can still be revealed.
        // Higher stealth strength reduces the effective reveal range.
        let stealth_factor = (stealth_strength / 100.0).clamp(0.0, 1.0);
        let weakness_factor = 1.0 - stealth_factor;
        let extended_range = reveal_distance * (1.0 + weakness_factor);

        distance < extended_range
    }

    /// Get all detectors (objects with non-zero detection strength)
    pub fn get_detectors(&self) -> Vec<ObjectID> {
        self.object_detection
            .iter()
            .filter(|(_, state)| state.detection_strength.value() > 0.0)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Update last frame detection was modified
    pub fn set_update_frame(&mut self, frame: UnsignedInt) {
        self.last_update_frame = frame;
    }

    /// Get last frame detection was updated
    pub fn get_last_update_frame(&self) -> UnsignedInt {
        self.last_update_frame
    }
}

impl Default for DetectionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global singleton accessor for DetectionManager
static DETECTION_MANAGER: OnceLock<Mutex<DetectionManager>> = OnceLock::new();

/// Get the global DetectionManager singleton
pub fn get_detection_manager() -> &'static Mutex<DetectionManager> {
    DETECTION_MANAGER.get_or_init(|| Mutex::new(DetectionManager::new()))
}

#[cfg(test)]
mod detection_tests {
    use super::*;

    #[test]
    fn test_detection_basic() {
        let mut manager = DetectionManager::new();

        // Register detector
        assert!(manager.register_object(1).is_ok());
        assert!(
            manager.register_object(1).is_err(),
            "Should not register twice"
        );

        // Check initial state (no detection)
        assert_eq!(manager.get_detection_strength(1).unwrap().value(), 0.0);
    }

    #[test]
    fn test_detection_strength() {
        let mut manager = DetectionManager::new();
        manager.register_object(1).unwrap();

        let strength = DetectionStrength::standard_detector();
        manager.set_detection_strength(1, strength).unwrap();

        assert_eq!(
            manager.get_detection_strength(1).unwrap().value(),
            strength.value()
        );
    }

    #[test]
    fn test_can_detect_stealth_basic() {
        let mut manager = DetectionManager::new();
        manager.register_object(1).unwrap();

        // Set detector strength to 60
        manager
            .set_detection_strength(1, DetectionStrength::standard_detector())
            .unwrap();

        // Test detection vs lower stealth (should detect)
        let modifier = DetectionModifier::default();
        assert!(manager.can_detect_stealth(1, 30.0, modifier).unwrap());

        // Test detection vs higher stealth (should not detect)
        assert!(!manager.can_detect_stealth(1, 90.0, modifier).unwrap());

        // Test detection vs equal stealth (should detect - detection > stealth)
        assert!(!manager.can_detect_stealth(1, 60.0, modifier).unwrap());
    }

    #[test]
    fn test_detection_modifiers() {
        let mut manager = DetectionManager::new();
        manager.register_object(1).unwrap();

        manager
            .set_detection_strength(1, DetectionStrength::standard_detector())
            .unwrap();

        // Base detection: 60.0
        // Stealth: 40.0

        // With full modifiers (1.0): 60 > 40, should detect
        let full_modifier = DetectionModifier::default();
        assert!(manager.can_detect_stealth(1, 40.0, full_modifier).unwrap());

        // With half effectiveness: 30 < 40, should not detect
        let half_modifier = DetectionModifier {
            distance_factor: 0.5,
            ..Default::default()
        };
        assert!(!manager.can_detect_stealth(1, 40.0, half_modifier).unwrap());

        // With combined modifiers: 0.5 * 0.5 * 0.5 = 0.125 effectiveness
        // 60 * 0.125 = 7.5 < 40, should not detect
        let weak_modifier = DetectionModifier {
            distance_factor: 0.5,
            unit_type_factor: 0.5,
            movement_factor: 0.5,
            special_factor: 1.0,
        };
        assert!(!manager.can_detect_stealth(1, 40.0, weak_modifier).unwrap());
    }

    #[test]
    fn test_detection_strength_values() {
        assert_eq!(DetectionStrength::none().value(), 0.0);
        assert_eq!(DetectionStrength::weak_detector().value(), 30.0);
        assert_eq!(DetectionStrength::standard_detector().value(), 60.0);
        assert_eq!(DetectionStrength::strong_detector().value(), 90.0);
    }

    #[test]
    fn test_detection_strength_clamping() {
        let weak = DetectionStrength::new(-10.0);
        assert_eq!(weak.value(), 0.0);

        let strong = DetectionStrength::new(150.0);
        assert_eq!(strong.value(), 100.0);
    }

    #[test]
    fn test_detection_modifier_clamping() {
        let modifier = DetectionModifier::new(1.5, -0.5, 2.0, 0.0);
        assert_eq!(modifier.distance_factor, 1.0);
        assert_eq!(modifier.unit_type_factor, 0.0);
        assert_eq!(modifier.movement_factor, 1.0);
        assert_eq!(modifier.special_factor, 0.0);
    }

    #[test]
    fn test_detection_effectiveness() {
        let mut manager = DetectionManager::new();
        manager.register_object(1).unwrap();

        manager
            .set_detection_strength(1, DetectionStrength::standard_detector())
            .unwrap();

        let modifier = DetectionModifier {
            distance_factor: 0.8,
            unit_type_factor: 0.9,
            movement_factor: 1.0,
            special_factor: 1.0,
        };

        let effectiveness = manager.get_detection_effectiveness(1, modifier).unwrap();
        let expected = 60.0 * 0.8 * 0.9;
        assert!((effectiveness - expected).abs() < 0.01);
    }

    #[test]
    fn test_get_detectors() {
        let mut manager = DetectionManager::new();
        manager.register_object(1).unwrap();
        manager.register_object(2).unwrap();
        manager.register_object(3).unwrap();

        // Only set detection on objects 1 and 3
        manager
            .set_detection_strength(1, DetectionStrength::standard_detector())
            .unwrap();
        manager
            .set_detection_strength(3, DetectionStrength::weak_detector())
            .unwrap();

        let detectors = manager.get_detectors();
        assert_eq!(detectors.len(), 2);
        assert!(detectors.contains(&1));
        assert!(detectors.contains(&3));
        assert!(!detectors.contains(&2));
    }

    #[test]
    fn test_detection_registration() {
        let mut manager = DetectionManager::new();

        manager.register_object(1).unwrap();
        manager.register_object(2).unwrap();

        // Unregister first object
        assert!(manager.unregister_object(1).is_ok());
        assert!(
            manager.get_detection_strength(1).is_err(),
            "Should not find unregistered object"
        );
        assert!(
            manager.get_detection_strength(2).is_ok(),
            "Should still find other object"
        );
    }

    #[test]
    fn test_calculate_reveal_distance_factor_within_threshold() {
        // Stealth is revealed when distance is less than reveal distance
        assert!(DetectionManager::calculate_reveal_distance_factor(
            50.0, 100.0
        ));
        assert!(DetectionManager::calculate_reveal_distance_factor(
            99.9, 100.0
        ));
    }

    #[test]
    fn test_calculate_reveal_distance_factor_at_threshold() {
        // At threshold, stealth should hold (not less than)
        assert!(!DetectionManager::calculate_reveal_distance_factor(
            100.0, 100.0
        ));
    }

    #[test]
    fn test_calculate_reveal_distance_factor_beyond_threshold() {
        // Stealth holds when distance exceeds reveal distance
        assert!(!DetectionManager::calculate_reveal_distance_factor(
            150.0, 100.0
        ));
        assert!(!DetectionManager::calculate_reveal_distance_factor(
            1000.0, 100.0
        ));
    }

    #[test]
    fn test_calculate_reveal_distance_factor_zero_distance() {
        // Unit at same position as target - definitely revealed
        assert!(DetectionManager::calculate_reveal_distance_factor(
            0.0, 100.0
        ));
    }

    #[test]
    fn test_calculate_reveal_distance_factor_zero_reveal_distance() {
        // No reveal distance configured - only revealed if exactly at same position
        assert!(!DetectionManager::calculate_reveal_distance_factor(
            0.1, 0.0
        ));
        assert!(DetectionManager::calculate_reveal_distance_factor(0.0, 0.0));
    }

    #[test]
    fn test_check_distance_reveal_with_strength_within_base_range() {
        // Within base reveal distance, always revealed regardless of stealth
        assert!(DetectionManager::check_distance_reveal_with_strength(
            50.0, 100.0, 0.0
        ));
        assert!(DetectionManager::check_distance_reveal_with_strength(
            50.0, 100.0, 100.0
        ));
        assert!(DetectionManager::check_distance_reveal_with_strength(
            99.9, 100.0, 100.0
        ));
    }

    #[test]
    fn test_check_distance_reveal_with_strength_weak_stealth_extended_range() {
        // Weak stealth (low strength) reveals at extended range
        let weak_stealth = 10.0; // Very weak stealth
        assert!(DetectionManager::check_distance_reveal_with_strength(
            110.0,
            100.0,
            weak_stealth
        ));
    }

    #[test]
    fn test_check_distance_reveal_with_strength_strong_stealth() {
        // Strong stealth (high strength) reduces extended range effectiveness
        let strong_stealth = 100.0; // Maximum stealth strength
        assert!(DetectionManager::check_distance_reveal_with_strength(
            99.9,
            100.0,
            strong_stealth
        ));
        assert!(!DetectionManager::check_distance_reveal_with_strength(
            100.1,
            100.0,
            strong_stealth
        ));
    }

    #[test]
    fn test_check_distance_reveal_multiple_stealth_levels() {
        let reveal_distance = 100.0;

        // Test progression of stealth strength levels
        assert!(DetectionManager::check_distance_reveal_with_strength(
            175.0,
            reveal_distance,
            0.0
        ));
        assert!(DetectionManager::check_distance_reveal_with_strength(
            174.9,
            reveal_distance,
            25.0
        ));
        assert!(DetectionManager::check_distance_reveal_with_strength(
            149.9,
            reveal_distance,
            50.0
        ));
        assert!(DetectionManager::check_distance_reveal_with_strength(
            124.9,
            reveal_distance,
            75.0
        ));
        assert!(!DetectionManager::check_distance_reveal_with_strength(
            100.1,
            reveal_distance,
            100.0
        ));
    }

    #[test]
    fn test_check_distance_reveal_boundary_conditions() {
        let reveal_distance = 100.0;

        // At exact base threshold
        assert!(!DetectionManager::check_distance_reveal_with_strength(
            100.0,
            reveal_distance,
            50.0
        ));

        // Just below extended range for medium stealth (50%)
        let stealth_50 = 50.0;
        let extended_range = reveal_distance * (1.0 + (1.0 - stealth_50 / 100.0)); // 150.0
        assert!(DetectionManager::check_distance_reveal_with_strength(
            extended_range - 0.1,
            reveal_distance,
            stealth_50
        ));
        assert!(!DetectionManager::check_distance_reveal_with_strength(
            extended_range,
            reveal_distance,
            stealth_50
        ));

        // Just above extended range
        assert!(!DetectionManager::check_distance_reveal_with_strength(
            extended_range + 0.1,
            reveal_distance,
            stealth_50
        ));
    }

    #[test]
    fn test_distance_reveal_clamping_stealth_over_100() {
        // Stealth strength clamped at 1.0x for extended range calculation
        let high_stealth = 150.0; // Over 100%, should be clamped
        let reveal_distance = 100.0;
        let expected_extended = reveal_distance; // clamped to 100% stealth => no extension

        assert!(DetectionManager::check_distance_reveal_with_strength(
            expected_extended - 0.1,
            reveal_distance,
            high_stealth
        ));
        assert!(!DetectionManager::check_distance_reveal_with_strength(
            expected_extended + 0.1,
            reveal_distance,
            high_stealth
        ));
    }
}
