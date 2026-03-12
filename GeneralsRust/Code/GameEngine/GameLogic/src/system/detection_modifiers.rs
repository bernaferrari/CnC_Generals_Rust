//! Detection Modifier Calculator System
//!
//! Calculates advanced dynamic modifiers matching C++ detection system:
//! - Distance-based modifiers (0.0 at max range, 1.0 at close range)
//! - Movement velocity-based detection (faster = easier to detect)
//! - Unit type-based detection difficulty (some units harder to detect)
//! - Ride/contained unit detection rules (riders affect detection)
//! - Visibility quality modifiers (line-of-sight, weather)

use crate::common::UnsignedInt;
use log::{debug, trace};
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;

/// Falloff curve type for distance-based modifiers
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DistanceFalloffCurve {
    /// Linear falloff: modifier = 1.0 - (distance / max_range)
    Linear,
    /// Exponential falloff: modifier = (1.0 - (distance / max_range))^2
    Exponential,
    /// Sigmoid falloff: smooth S-curve transition
    Sigmoid,
}

/// Unit type for detection difficulty mapping
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnitTypeCategory {
    /// Infantry units - standard detection difficulty (1.0)
    Infantry,
    /// Vehicle units - moderate detection difficulty (0.8)
    Vehicle,
    /// Aircraft units - high detection difficulty (1.2)
    Aircraft,
    /// Building/structure - very high detection difficulty (1.5)
    Building,
    /// Stealth unit - extreme detection difficulty (0.5)
    Stealth,
}

impl UnitTypeCategory {
    /// Get detection difficulty multiplier for unit type
    /// Higher value = harder to detect
    pub fn difficulty_multiplier(&self) -> f32 {
        match self {
            UnitTypeCategory::Infantry => 1.0,
            UnitTypeCategory::Vehicle => 0.8,
            UnitTypeCategory::Aircraft => 1.2,
            UnitTypeCategory::Building => 1.5,
            UnitTypeCategory::Stealth => 0.5,
        }
    }
}

/// Detection modifier combining all factors
#[derive(Debug, Clone, Copy)]
pub struct DetectionModifier {
    /// Distance modifier (0.0 at max range, 1.0 at close range)
    pub distance_modifier: f32,
    /// Movement modifier (0.0 = stationary, 1.0 = very fast)
    pub movement_modifier: f32,
    /// Unit type modifier (difficulty to detect)
    pub unit_type_modifier: f32,
    /// Rider/contained unit modifier
    pub rider_modifier: f32,
    /// Line of sight modifier
    pub los_modifier: f32,
    /// Garrisoned unit modifier
    pub garrisoned_modifier: f32,
}

impl Default for DetectionModifier {
    fn default() -> Self {
        Self {
            distance_modifier: 1.0,
            movement_modifier: 1.0,
            unit_type_modifier: 1.0,
            rider_modifier: 1.0,
            los_modifier: 1.0,
            garrisoned_modifier: 1.0,
        }
    }
}

impl DetectionModifier {
    /// Calculate combined modifier (multiplicative effect)
    pub fn combined_factor(&self) -> f32 {
        self.distance_modifier
            * self.movement_modifier
            * self.unit_type_modifier
            * self.rider_modifier
            * self.los_modifier
            * self.garrisoned_modifier
    }
}

/// Configuration parameters for detection modifier calculations
#[derive(Debug, Clone)]
pub struct DetectionModifierConfig {
    /// Distance falloff curve type
    pub distance_curve: DistanceFalloffCurve,
    /// Maximum detection range (in game units)
    pub max_detection_range: f32,
    /// Velocity threshold for movement detection
    pub velocity_threshold: f32,
    /// Unit type detection difficulty map
    pub unit_type_difficulties: HashMap<UnitTypeCategory, f32>,
    /// Rider detection penalty
    pub rider_detection_penalty: f32,
    /// Garrisoned unit detection bonus
    pub garrisoned_detection_bonus: f32,
}

impl Default for DetectionModifierConfig {
    fn default() -> Self {
        let mut difficulties = HashMap::new();
        difficulties.insert(UnitTypeCategory::Infantry, 1.0);
        difficulties.insert(UnitTypeCategory::Vehicle, 0.8);
        difficulties.insert(UnitTypeCategory::Aircraft, 1.2);
        difficulties.insert(UnitTypeCategory::Building, 1.5);
        difficulties.insert(UnitTypeCategory::Stealth, 0.5);

        Self {
            distance_curve: DistanceFalloffCurve::Exponential,
            max_detection_range: 300.0,
            velocity_threshold: 2.0,
            unit_type_difficulties: difficulties,
            rider_detection_penalty: 0.7,
            garrisoned_detection_bonus: 1.5,
        }
    }
}

/// Detection Modifier Calculator
///
/// Calculates advanced dynamic modifiers for detection system matching C++ behavior.
/// Handles distance, velocity, unit type, riders, line-of-sight, and garrisoned states.
pub struct DetectionModifierCalculator {
    /// Configuration for calculations
    config: DetectionModifierConfig,
}

impl DetectionModifierCalculator {
    /// Create new calculator with default configuration
    pub fn new() -> Self {
        Self {
            config: DetectionModifierConfig::default(),
        }
    }

    /// Create calculator with custom configuration
    pub fn with_config(config: DetectionModifierConfig) -> Self {
        Self { config }
    }

    /// Get current configuration
    pub fn config(&self) -> &DetectionModifierConfig {
        &self.config
    }

    /// Update configuration
    pub fn set_config(&mut self, config: DetectionModifierConfig) {
        self.config = config;
    }

    /// Calculate distance modifier
    ///
    /// Returns 1.0 at close range, 0.0 at maximum range.
    /// Falls off according to configured curve type.
    ///
    /// # Arguments
    /// * `distance` - Current distance to target (game units)
    /// * `max_range` - Maximum detection range (game units)
    ///
    /// # Returns
    /// Distance modifier (0.0 to 1.0)
    pub fn calculate_distance_modifier(&self, distance: f32, max_range: f32) -> f32 {
        if distance <= 0.0 {
            return 1.0;
        }

        let normalized = (distance / max_range).min(1.0);

        match self.config.distance_curve {
            DistanceFalloffCurve::Linear => 1.0 - normalized,
            DistanceFalloffCurve::Exponential => {
                let factor = 1.0 - normalized;
                factor * factor
            }
            DistanceFalloffCurve::Sigmoid => {
                // Sigmoid: smooth S-curve from 1.0 to 0.0
                let x = (normalized - 0.5) * 6.0; // Stretch to -3..3 range
                1.0 / (1.0 + x.exp())
            }
        }
    }

    /// Calculate movement modifier
    ///
    /// Moving units are easier to detect than stationary ones.
    /// Returns higher values for faster movement.
    ///
    /// # Arguments
    /// * `velocity` - Current movement velocity
    /// * `threshold_speed` - Speed threshold for detection bonus
    ///
    /// # Returns
    /// Movement modifier (0.0 to 1.0+)
    pub fn calculate_movement_modifier(&self, velocity: f32, threshold_speed: f32) -> f32 {
        if velocity <= 0.0 {
            return 0.7; // Stationary units harder to detect
        }

        if velocity < threshold_speed {
            // Linear increase from 0.7 to 1.0
            0.7 + (0.3 * (velocity / threshold_speed))
        } else {
            // Above threshold, detection improves linearly
            1.0 + (0.5 * ((velocity - threshold_speed) / threshold_speed).min(1.0))
        }
    }

    /// Calculate unit type modifier
    ///
    /// Different unit types have different detection difficulty.
    /// Returns multiplier based on unit type category.
    ///
    /// # Arguments
    /// * `unit_kindof` - Unit type category
    ///
    /// # Returns
    /// Unit type modifier (affects how hard to detect)
    pub fn calculate_unit_type_modifier(&self, unit_kindof: UnitTypeCategory) -> f32 {
        self.config
            .unit_type_difficulties
            .get(&unit_kindof)
            .copied()
            .unwrap_or_else(|| unit_kindof.difficulty_multiplier())
    }

    /// Calculate rider modifier
    ///
    /// Units with riders become easier to detect, especially if attacking.
    ///
    /// # Arguments
    /// * `has_riders` - Whether unit contains riders
    /// * `riders_attacking` - Whether riders are actively attacking
    ///
    /// # Returns
    /// Rider modifier (0.0 to 1.0)
    pub fn calculate_rider_modifier(&self, has_riders: bool, riders_attacking: bool) -> f32 {
        if !has_riders {
            return 1.0;
        }

        if riders_attacking {
            1.0 // Attacking riders are easily detectable
        } else {
            self.config.rider_detection_penalty
        }
    }

    /// Calculate line-of-sight modifier
    ///
    /// Units with clear line-of-sight are more easily detected.
    ///
    /// # Arguments
    /// * `has_line_of_sight` - Whether detector has line-of-sight to target
    ///
    /// # Returns
    /// Line-of-sight modifier (0.0 to 1.0)
    pub fn calculate_los_modifier(&self, has_line_of_sight: bool) -> f32 {
        if has_line_of_sight {
            1.0 // Full detection bonus with LOS
        } else {
            0.6 // Reduced detection without LOS
        }
    }

    /// Calculate garrisoned modifier
    ///
    /// Garrisoned units are more easily detected by area effects.
    ///
    /// # Arguments
    /// * `is_garrisoned` - Whether unit is garrisoned
    ///
    /// # Returns
    /// Garrisoned modifier (amplifies detection)
    pub fn calculate_garrisoned_modifier(&self, is_garrisoned: bool) -> f32 {
        if is_garrisoned {
            self.config.garrisoned_detection_bonus
        } else {
            1.0
        }
    }

    /// Calculate combined detection modifier
    ///
    /// Combines all individual modifiers into single DetectionModifier structure.
    ///
    /// # Arguments
    /// * `distance` - Distance to target
    /// * `max_range` - Maximum detection range
    /// * `velocity` - Target movement velocity
    /// * `unit_type` - Unit type category
    /// * `has_riders` - Whether unit has riders
    /// * `riders_attacking` - Whether riders attacking
    /// * `has_los` - Whether line-of-sight exists
    /// * `is_garrisoned` - Whether unit is garrisoned
    ///
    /// # Returns
    /// Combined DetectionModifier
    pub fn calculate_combined_modifier(
        &self,
        distance: f32,
        max_range: f32,
        velocity: f32,
        unit_type: UnitTypeCategory,
        has_riders: bool,
        riders_attacking: bool,
        has_los: bool,
        is_garrisoned: bool,
    ) -> Result<DetectionModifier, String> {
        let distance_mod = self.calculate_distance_modifier(distance, max_range);
        let movement_mod =
            self.calculate_movement_modifier(velocity, self.config.velocity_threshold);
        let unit_type_mod = self.calculate_unit_type_modifier(unit_type);
        let rider_mod = self.calculate_rider_modifier(has_riders, riders_attacking);
        let los_mod = self.calculate_los_modifier(has_los);
        let garrisoned_mod = self.calculate_garrisoned_modifier(is_garrisoned);

        Ok(DetectionModifier {
            distance_modifier: distance_mod,
            movement_modifier: movement_mod,
            unit_type_modifier: unit_type_mod,
            rider_modifier: rider_mod,
            los_modifier: los_mod,
            garrisoned_modifier: garrisoned_mod,
        })
    }

    /// Calculate distance from coordinates
    ///
    /// Helper method for coordinate-based distance calculations.
    ///
    /// # Arguments
    /// * `from_x`, `from_y` - Source coordinates
    /// * `to_x`, `to_y` - Target coordinates
    ///
    /// # Returns
    /// Euclidean distance
    pub fn calculate_distance(from_x: f32, from_y: f32, to_x: f32, to_y: f32) -> f32 {
        let dx = to_x - from_x;
        let dy = to_y - from_y;
        (dx * dx + dy * dy).sqrt()
    }
}

impl Default for DetectionModifierCalculator {
    fn default() -> Self {
        Self::new()
    }
}

/// Global singleton accessor for DetectionModifierCalculator
static DETECTION_MODIFIER_CALCULATOR: OnceLock<Mutex<DetectionModifierCalculator>> =
    OnceLock::new();

/// Get the global DetectionModifierCalculator singleton
pub fn get_detection_modifier_calculator() -> &'static Mutex<DetectionModifierCalculator> {
    DETECTION_MODIFIER_CALCULATOR.get_or_init(|| Mutex::new(DetectionModifierCalculator::new()))
}

#[cfg(test)]
mod detection_modifier_tests {
    use super::*;

    #[test]
    fn test_distance_modifier_calculation() {
        let calc = DetectionModifierCalculator::new();

        // At distance 0, should be full detection (1.0)
        let mod_zero = calc.calculate_distance_modifier(0.0, 300.0);
        assert!((mod_zero - 1.0).abs() < 0.01);

        // At distance 150 (halfway), should be roughly 0.25 with exponential
        let mod_half = calc.calculate_distance_modifier(150.0, 300.0);
        assert!(mod_half < 1.0 && mod_half > 0.0);

        // At max range 300, should be near 0
        let mod_max = calc.calculate_distance_modifier(300.0, 300.0);
        assert!(mod_max < 0.1);
    }

    #[test]
    fn test_distance_modifier_extremes() {
        let calc = DetectionModifierCalculator::new();

        // Zero distance should always be 1.0
        assert!((calc.calculate_distance_modifier(0.0, 100.0) - 1.0).abs() < 0.01);
        assert!((calc.calculate_distance_modifier(0.0, 500.0) - 1.0).abs() < 0.01);

        // Beyond max range should clamp to 0.0
        let beyond = calc.calculate_distance_modifier(500.0, 300.0);
        assert!(beyond <= 0.1);
    }

    #[test]
    fn test_distance_modifier_curves() {
        let mut config = DetectionModifierConfig::default();

        // Linear curve
        config.distance_curve = DistanceFalloffCurve::Linear;
        let calc_linear = DetectionModifierCalculator::with_config(config.clone());
        let linear_mid = calc_linear.calculate_distance_modifier(150.0, 300.0);

        // Exponential curve (should drop off faster)
        config.distance_curve = DistanceFalloffCurve::Exponential;
        let calc_exp = DetectionModifierCalculator::with_config(config.clone());
        let exp_mid = calc_exp.calculate_distance_modifier(150.0, 300.0);

        // Exponential should be lower than linear at midpoint
        assert!(exp_mid < linear_mid);
    }

    #[test]
    fn test_movement_modifier_stationary() {
        let calc = DetectionModifierCalculator::new();

        let stationary = calc.calculate_movement_modifier(0.0, 2.0);
        assert!((stationary - 0.7).abs() < 0.01);
    }

    #[test]
    fn test_movement_modifier_moving() {
        let calc = DetectionModifierCalculator::new();

        // Below threshold
        let slow = calc.calculate_movement_modifier(1.0, 2.0);
        assert!(slow > 0.7 && slow < 1.0);

        // At threshold
        let threshold = calc.calculate_movement_modifier(2.0, 2.0);
        assert!((threshold - 1.0).abs() < 0.01);

        // Above threshold
        let fast = calc.calculate_movement_modifier(4.0, 2.0);
        assert!(fast > 1.0);
    }

    #[test]
    fn test_movement_modifier_thresholds() {
        let calc = DetectionModifierCalculator::new();

        // Low velocity should give penalty
        let low = calc.calculate_movement_modifier(0.5, 2.0);
        assert!(low < 1.0);

        // Very high velocity should give significant bonus
        let high = calc.calculate_movement_modifier(10.0, 2.0);
        assert!(high > 1.2);
    }

    #[test]
    fn test_unit_type_modifier_infantry() {
        let calc = DetectionModifierCalculator::new();

        let infantry = calc.calculate_unit_type_modifier(UnitTypeCategory::Infantry);
        assert!((infantry - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_unit_type_modifier_vehicle() {
        let calc = DetectionModifierCalculator::new();

        let vehicle = calc.calculate_unit_type_modifier(UnitTypeCategory::Vehicle);
        assert!((vehicle - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_unit_type_modifier_aircraft() {
        let calc = DetectionModifierCalculator::new();

        let aircraft = calc.calculate_unit_type_modifier(UnitTypeCategory::Aircraft);
        assert!((aircraft - 1.2).abs() < 0.01);
    }

    #[test]
    fn test_unit_type_modifier_building() {
        let calc = DetectionModifierCalculator::new();

        let building = calc.calculate_unit_type_modifier(UnitTypeCategory::Building);
        assert!((building - 1.5).abs() < 0.01);
    }

    #[test]
    fn test_unit_type_modifier_stealth() {
        let calc = DetectionModifierCalculator::new();

        let stealth = calc.calculate_unit_type_modifier(UnitTypeCategory::Stealth);
        assert!((stealth - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_rider_modifier_no_riders() {
        let calc = DetectionModifierCalculator::new();

        let no_riders = calc.calculate_rider_modifier(false, false);
        assert!((no_riders - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_rider_modifier_attacking_riders() {
        let calc = DetectionModifierCalculator::new();

        let attacking = calc.calculate_rider_modifier(true, true);
        assert!((attacking - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_rider_modifier_non_attacking_riders() {
        let calc = DetectionModifierCalculator::new();

        let not_attacking = calc.calculate_rider_modifier(true, false);
        assert!(not_attacking < 1.0);
        assert!(not_attacking > 0.5);
    }

    #[test]
    fn test_los_modifier_with_without() {
        let calc = DetectionModifierCalculator::new();

        let with_los = calc.calculate_los_modifier(true);
        let without_los = calc.calculate_los_modifier(false);

        assert!((with_los - 1.0).abs() < 0.01);
        assert!(without_los < with_los);
    }

    #[test]
    fn test_garrisoned_modifier() {
        let calc = DetectionModifierCalculator::new();

        let not_garrisoned = calc.calculate_garrisoned_modifier(false);
        let garrisoned = calc.calculate_garrisoned_modifier(true);

        assert!((not_garrisoned - 1.0).abs() < 0.01);
        assert!(garrisoned > 1.0);
    }

    #[test]
    fn test_combined_modifier_stacking() {
        let calc = DetectionModifierCalculator::new();

        let modifier = calc
            .calculate_combined_modifier(
                50.0,  // distance
                300.0, // max_range
                1.0,   // velocity
                UnitTypeCategory::Infantry,
                false, // has_riders
                false, // riders_attacking
                true,  // has_los
                false, // is_garrisoned
            )
            .unwrap();

        // All factors should multiply together
        let expected = modifier.distance_modifier
            * modifier.movement_modifier
            * modifier.unit_type_modifier
            * modifier.rider_modifier
            * modifier.los_modifier
            * modifier.garrisoned_modifier;

        assert!((modifier.combined_factor() - expected).abs() < 0.001);
    }

    #[test]
    fn test_combined_modifier_no_detection() {
        let calc = DetectionModifierCalculator::new();

        // Create scenario with minimal detection
        let modifier = calc
            .calculate_combined_modifier(
                300.0, // at max range
                300.0,
                0.0, // stationary
                UnitTypeCategory::Stealth,
                false,
                false,
                false, // no LOS
                false,
            )
            .unwrap();

        // Combined factor should be quite low
        assert!(modifier.combined_factor() < 0.3);
    }

    #[test]
    fn test_combined_modifier_maximum_detection() {
        let calc = DetectionModifierCalculator::new();

        // Create scenario with maximum detection
        let modifier = calc
            .calculate_combined_modifier(
                0.0, // at zero distance
                300.0,
                10.0, // very fast
                UnitTypeCategory::Infantry,
                false,
                false,
                true, // clear LOS
                false,
            )
            .unwrap();

        // Combined factor should be high
        assert!(modifier.combined_factor() > 0.8);
    }

    #[test]
    fn test_combined_modifier_with_riders_attacking() {
        let calc = DetectionModifierCalculator::new();

        let with_riders = calc
            .calculate_combined_modifier(
                100.0,
                300.0,
                2.0,
                UnitTypeCategory::Vehicle,
                true, // has_riders
                true, // attacking
                true,
                false,
            )
            .unwrap();

        let without_riders = calc
            .calculate_combined_modifier(
                100.0,
                300.0,
                2.0,
                UnitTypeCategory::Vehicle,
                false, // no_riders
                false,
                true,
                false,
            )
            .unwrap();

        // Attacking riders should increase detectability
        assert!(with_riders.combined_factor() >= without_riders.combined_factor());
    }

    #[test]
    fn test_combined_modifier_garrisoned() {
        let calc = DetectionModifierCalculator::new();

        let garrisoned = calc
            .calculate_combined_modifier(
                100.0,
                300.0,
                0.0,
                UnitTypeCategory::Infantry,
                false,
                false,
                true,
                true, // garrisoned
            )
            .unwrap();

        let not_garrisoned = calc
            .calculate_combined_modifier(
                100.0,
                300.0,
                0.0,
                UnitTypeCategory::Infantry,
                false,
                false,
                true,
                false, // not garrisoned
            )
            .unwrap();

        // Garrisoned should be more detectable
        assert!(garrisoned.combined_factor() > not_garrisoned.combined_factor());
    }

    #[test]
    fn test_detection_modifier_combined_factor() {
        let modifier = DetectionModifier {
            distance_modifier: 0.5,
            movement_modifier: 0.8,
            unit_type_modifier: 1.2,
            rider_modifier: 0.9,
            los_modifier: 1.0,
            garrisoned_modifier: 1.0,
        };

        let expected = 0.5 * 0.8 * 1.2 * 0.9 * 1.0 * 1.0;
        assert!((modifier.combined_factor() - expected).abs() < 0.001);
    }

    #[test]
    fn test_distance_calculation() {
        // Distance from (0, 0) to (3, 4) should be 5
        let dist = DetectionModifierCalculator::calculate_distance(0.0, 0.0, 3.0, 4.0);
        assert!((dist - 5.0).abs() < 0.01);

        // Distance from (1, 1) to (1, 1) should be 0
        let dist_zero = DetectionModifierCalculator::calculate_distance(1.0, 1.0, 1.0, 1.0);
        assert!(dist_zero < 0.001);

        // Distance from (0, 0) to (100, 0) should be 100
        let dist_straight = DetectionModifierCalculator::calculate_distance(0.0, 0.0, 100.0, 0.0);
        assert!((dist_straight - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_unit_type_category_difficulty() {
        assert!((UnitTypeCategory::Infantry.difficulty_multiplier() - 1.0).abs() < 0.01);
        assert!((UnitTypeCategory::Vehicle.difficulty_multiplier() - 0.8).abs() < 0.01);
        assert!((UnitTypeCategory::Aircraft.difficulty_multiplier() - 1.2).abs() < 0.01);
        assert!((UnitTypeCategory::Building.difficulty_multiplier() - 1.5).abs() < 0.01);
        assert!((UnitTypeCategory::Stealth.difficulty_multiplier() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_custom_config() {
        let mut config = DetectionModifierConfig::default();
        config.max_detection_range = 500.0;
        config.velocity_threshold = 5.0;
        config.rider_detection_penalty = 0.5;

        let calc = DetectionModifierCalculator::with_config(config);

        // Test that custom config is used
        let mod_vel = calc.calculate_movement_modifier(2.5, 5.0);
        assert!(mod_vel < 1.0); // Below custom threshold

        let mod_vel_above = calc.calculate_movement_modifier(5.0, 5.0);
        assert!((mod_vel_above - 1.0).abs() < 0.01); // At custom threshold
    }

    #[test]
    fn test_sigmoid_falloff_curve() {
        let mut config = DetectionModifierConfig::default();
        config.distance_curve = DistanceFalloffCurve::Sigmoid;

        let calc = DetectionModifierCalculator::with_config(config);

        // Sigmoid should be smooth and symmetric
        let at_quarter = calc.calculate_distance_modifier(75.0, 300.0);
        let at_three_quarter = calc.calculate_distance_modifier(225.0, 300.0);

        // Should be symmetric around 0.5
        assert!(((at_quarter + at_three_quarter) - 1.0).abs() < 0.1);
    }
}
