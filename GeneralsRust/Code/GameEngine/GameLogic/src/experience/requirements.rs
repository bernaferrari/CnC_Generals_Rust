//! Experience Requirements - XP thresholds for veterancy levels
//!
//! This module defines the experience point requirements for reaching each
//! veterancy level, based on unit cost and scaling factors.

use crate::common::types::VeterancyLevel;

/// Experience requirements calculator (matches C++ ThingTemplate behavior)
///
/// The C++ implementation stores experience requirements in the ThingTemplate,
/// calculated from the object's build cost using these formulas:
///
/// ```text
/// Veteran:  object_cost * 1.0
/// Elite:    object_cost * 3.0
/// Heroic:   object_cost * 6.0
/// ```
///
/// This provides helpers to calculate those requirements.
#[derive(Debug, Clone)]
pub struct ExperienceRequirements {
    /// Experience required for each level [Regular, Veteran, Elite, Heroic]
    requirements: [i32; 4],
}

impl ExperienceRequirements {
    /// Calculate experience requirements from object build cost
    ///
    /// # Parameters
    /// - `build_cost`: The build cost of the unit/structure
    ///
    /// # Returns
    /// Array of XP requirements for [Regular, Veteran, Elite, Heroic]
    ///
    /// # Formula (matching C++ exactly)
    /// ```text
    /// Regular:  0 (starting level)
    /// Veteran:  build_cost * 1.0
    /// Elite:    build_cost * 3.0
    /// Heroic:   build_cost * 6.0
    /// ```
    pub fn from_build_cost(build_cost: i32) -> Self {
        Self {
            requirements: [
                0,              // Regular - no XP required
                build_cost,     // Veteran - 1x cost
                build_cost * 3, // Elite - 3x cost
                build_cost * 6, // Heroic - 6x cost
            ],
        }
    }

    /// Create with explicit requirements for each level
    pub fn from_array(requirements: [i32; 4]) -> Self {
        Self { requirements }
    }

    /// Create default requirements (for testing or fallback)
    ///
    /// Default values:
    /// - Regular: 0
    /// - Veteran: 100
    /// - Elite: 300
    /// - Heroic: 600
    pub fn default_requirements() -> Self {
        Self {
            requirements: [0, 100, 300, 600],
        }
    }

    /// Get experience required for a specific level
    pub fn get_required(&self, level: VeterancyLevel) -> i32 {
        self.requirements[level as usize]
    }

    /// Get the full requirements array
    pub fn as_array(&self) -> &[i32; 4] {
        &self.requirements
    }

    /// Calculate remaining XP needed for next level
    ///
    /// # Parameters
    /// - `current_level`: Current veterancy level
    /// - `current_xp`: Current experience points
    ///
    /// # Returns
    /// XP needed to reach next level, or 0 if already at max level
    pub fn xp_to_next_level(&self, current_level: VeterancyLevel, current_xp: i32) -> i32 {
        if current_level == VeterancyLevel::Heroic {
            return 0; // Already at max level
        }

        let next_level_index = (current_level as usize) + 1;
        let next_level_req = self.requirements[next_level_index];

        (next_level_req - current_xp).max(0)
    }

    /// Calculate progress toward next level as percentage
    ///
    /// # Parameters
    /// - `current_level`: Current veterancy level
    /// - `current_xp`: Current experience points
    ///
    /// # Returns
    /// Progress as 0.0 to 1.0, or 1.0 if already at max level
    pub fn progress_to_next_level(&self, current_level: VeterancyLevel, current_xp: i32) -> f32 {
        if current_level == VeterancyLevel::Heroic {
            return 1.0; // Already at max level
        }

        let current_level_req = self.requirements[current_level as usize];
        let next_level_index = (current_level as usize) + 1;
        let next_level_req = self.requirements[next_level_index];

        let xp_in_level = current_xp - current_level_req;
        let xp_needed = next_level_req - current_level_req;

        if xp_needed <= 0 {
            return 1.0;
        }

        (xp_in_level as f32 / xp_needed as f32).clamp(0.0, 1.0)
    }

    /// Check if current XP qualifies for a specific level
    pub fn qualifies_for_level(&self, current_xp: i32, level: VeterancyLevel) -> bool {
        current_xp >= self.requirements[level as usize]
    }

    /// Calculate what level current XP qualifies for
    pub fn calculate_level_from_xp(&self, current_xp: i32) -> VeterancyLevel {
        if current_xp >= self.requirements[VeterancyLevel::Heroic as usize] {
            VeterancyLevel::Heroic
        } else if current_xp >= self.requirements[VeterancyLevel::Elite as usize] {
            VeterancyLevel::Elite
        } else if current_xp >= self.requirements[VeterancyLevel::Veteran as usize] {
            VeterancyLevel::Veteran
        } else {
            VeterancyLevel::Regular
        }
    }
}

impl Default for ExperienceRequirements {
    fn default() -> Self {
        Self::default_requirements()
    }
}

/// Preset experience requirements for common unit costs
pub struct PresetRequirements;

impl PresetRequirements {
    /// Cheap units (cost: 100)
    /// Veteran: 100, Elite: 300, Heroic: 600
    pub fn cheap_unit() -> ExperienceRequirements {
        ExperienceRequirements::from_build_cost(100)
    }

    /// Standard infantry (cost: 200)
    /// Veteran: 200, Elite: 600, Heroic: 1200
    pub fn standard_infantry() -> ExperienceRequirements {
        ExperienceRequirements::from_build_cost(200)
    }

    /// Standard vehicle (cost: 600)
    /// Veteran: 600, Elite: 1800, Heroic: 3600
    pub fn standard_vehicle() -> ExperienceRequirements {
        ExperienceRequirements::from_build_cost(600)
    }

    /// Standard tank (cost: 1200)
    /// Veteran: 1200, Elite: 3600, Heroic: 7200
    pub fn standard_tank() -> ExperienceRequirements {
        ExperienceRequirements::from_build_cost(1200)
    }

    /// Standard aircraft (cost: 1400)
    /// Veteran: 1400, Elite: 4200, Heroic: 8400
    pub fn standard_aircraft() -> ExperienceRequirements {
        ExperienceRequirements::from_build_cost(1400)
    }

    /// Heavy tank (cost: 2000)
    /// Veteran: 2000, Elite: 6000, Heroic: 12000
    pub fn heavy_tank() -> ExperienceRequirements {
        ExperienceRequirements::from_build_cost(2000)
    }

    /// Superweapon/Hero (cost: 5000)
    /// Veteran: 5000, Elite: 15000, Heroic: 30000
    pub fn hero_unit() -> ExperienceRequirements {
        ExperienceRequirements::from_build_cost(5000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_build_cost() {
        let req = ExperienceRequirements::from_build_cost(1000);

        assert_eq!(req.get_required(VeterancyLevel::Regular), 0);
        assert_eq!(req.get_required(VeterancyLevel::Veteran), 1000);
        assert_eq!(req.get_required(VeterancyLevel::Elite), 3000);
        assert_eq!(req.get_required(VeterancyLevel::Heroic), 6000);
    }

    #[test]
    fn test_default_requirements() {
        let req = ExperienceRequirements::default_requirements();

        assert_eq!(req.get_required(VeterancyLevel::Regular), 0);
        assert_eq!(req.get_required(VeterancyLevel::Veteran), 100);
        assert_eq!(req.get_required(VeterancyLevel::Elite), 300);
        assert_eq!(req.get_required(VeterancyLevel::Heroic), 600);
    }

    #[test]
    fn test_xp_to_next_level() {
        let req = ExperienceRequirements::from_build_cost(1000);

        // At Regular with 500 XP, need 500 more for Veteran
        let needed = req.xp_to_next_level(VeterancyLevel::Regular, 500);
        assert_eq!(needed, 500);

        // At Veteran with 2000 XP, need 1000 more for Elite
        let needed = req.xp_to_next_level(VeterancyLevel::Veteran, 2000);
        assert_eq!(needed, 1000);

        // Already at Heroic
        let needed = req.xp_to_next_level(VeterancyLevel::Heroic, 10000);
        assert_eq!(needed, 0);
    }

    #[test]
    fn test_progress_to_next_level() {
        let req = ExperienceRequirements::from_build_cost(1000);

        // At Regular with 500 XP (halfway to Veteran)
        let progress = req.progress_to_next_level(VeterancyLevel::Regular, 500);
        assert_eq!(progress, 0.5);

        // At Veteran with 1500 XP (25% to Elite: (1500-1000)/(3000-1000) = 500/2000)
        let progress = req.progress_to_next_level(VeterancyLevel::Veteran, 1500);
        assert_eq!(progress, 0.25);

        // Already at Heroic
        let progress = req.progress_to_next_level(VeterancyLevel::Heroic, 10000);
        assert_eq!(progress, 1.0);
    }

    #[test]
    fn test_qualifies_for_level() {
        let req = ExperienceRequirements::from_build_cost(1000);

        assert!(req.qualifies_for_level(0, VeterancyLevel::Regular));
        assert!(!req.qualifies_for_level(500, VeterancyLevel::Veteran));
        assert!(req.qualifies_for_level(1000, VeterancyLevel::Veteran));
        assert!(req.qualifies_for_level(3000, VeterancyLevel::Elite));
        assert!(req.qualifies_for_level(6000, VeterancyLevel::Heroic));
    }

    #[test]
    fn test_calculate_level_from_xp() {
        let req = ExperienceRequirements::from_build_cost(1000);

        assert_eq!(req.calculate_level_from_xp(0), VeterancyLevel::Regular);
        assert_eq!(req.calculate_level_from_xp(500), VeterancyLevel::Regular);
        assert_eq!(req.calculate_level_from_xp(1000), VeterancyLevel::Veteran);
        assert_eq!(req.calculate_level_from_xp(2000), VeterancyLevel::Veteran);
        assert_eq!(req.calculate_level_from_xp(3000), VeterancyLevel::Elite);
        assert_eq!(req.calculate_level_from_xp(5000), VeterancyLevel::Elite);
        assert_eq!(req.calculate_level_from_xp(6000), VeterancyLevel::Heroic);
        assert_eq!(req.calculate_level_from_xp(10000), VeterancyLevel::Heroic);
    }

    #[test]
    fn test_preset_cheap_unit() {
        let req = PresetRequirements::cheap_unit();
        assert_eq!(req.get_required(VeterancyLevel::Veteran), 100);
        assert_eq!(req.get_required(VeterancyLevel::Elite), 300);
        assert_eq!(req.get_required(VeterancyLevel::Heroic), 600);
    }

    #[test]
    fn test_preset_standard_tank() {
        let req = PresetRequirements::standard_tank();
        assert_eq!(req.get_required(VeterancyLevel::Veteran), 1200);
        assert_eq!(req.get_required(VeterancyLevel::Elite), 3600);
        assert_eq!(req.get_required(VeterancyLevel::Heroic), 7200);
    }

    #[test]
    fn test_preset_hero_unit() {
        let req = PresetRequirements::hero_unit();
        assert_eq!(req.get_required(VeterancyLevel::Veteran), 5000);
        assert_eq!(req.get_required(VeterancyLevel::Elite), 15000);
        assert_eq!(req.get_required(VeterancyLevel::Heroic), 30000);
    }

    #[test]
    fn test_as_array() {
        let req = ExperienceRequirements::from_build_cost(1000);
        let array = req.as_array();

        assert_eq!(array.len(), 4);
        assert_eq!(array[0], 0);
        assert_eq!(array[1], 1000);
        assert_eq!(array[2], 3000);
        assert_eq!(array[3], 6000);
    }

    #[test]
    fn test_from_array() {
        let custom = [0, 50, 150, 300];
        let req = ExperienceRequirements::from_array(custom);

        assert_eq!(req.get_required(VeterancyLevel::Regular), 0);
        assert_eq!(req.get_required(VeterancyLevel::Veteran), 50);
        assert_eq!(req.get_required(VeterancyLevel::Elite), 150);
        assert_eq!(req.get_required(VeterancyLevel::Heroic), 300);
    }

    #[test]
    fn test_scaling_matches_cpp_formula() {
        // Test that our scaling matches the C++ formula exactly
        let build_cost = 1000;
        let req = ExperienceRequirements::from_build_cost(build_cost);

        // Verify: Veteran = cost * 1, Elite = cost * 3, Heroic = cost * 6
        assert_eq!(req.get_required(VeterancyLevel::Veteran), build_cost);
        assert_eq!(req.get_required(VeterancyLevel::Elite), build_cost * 3);
        assert_eq!(req.get_required(VeterancyLevel::Heroic), build_cost * 6);
    }
}
