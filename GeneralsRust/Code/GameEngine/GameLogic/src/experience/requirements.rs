//! Experience threshold tables for veterancy levels.
//!
//! C++ stores per-template XP thresholds on the ThingTemplate (parsed from
//! INI) and ExperienceTracker always reads them via
//! `getTemplate()->getExperienceRequired` (ExperienceTracker.cpp:75, 91,
//! 106, 152, 192). This struct is a Rust-side container for explicit
//! threshold arrays (degraded wiring, tests); it does NOT derive thresholds
//! from unit cost — the former cost-scaled `from_build_cost` table (Veteran =
//! 1x / Elite = 3x / Heroic = 6x cost) had no C++ counterpart and was
//! removed. See
//! [`crate::experience::ExperienceTracker::DEFAULT_EXPERIENCE_REQUIRED`] for
//! the degraded default table.

use crate::common::types::VeterancyLevel;

/// Explicit experience thresholds per level [Regular, Veteran, Elite, Heroic].
#[derive(Debug, Clone)]
pub struct ExperienceRequirements {
    requirements: [i32; 4],
}

impl ExperienceRequirements {
    /// Create with explicit requirements for each level.
    pub fn from_array(requirements: [i32; 4]) -> Self {
        Self { requirements }
    }

    /// Degraded default table (Regular 0 / Veteran 100 / Elite 300 / Heroic
    /// 600). This is a Rust-only placeholder, NOT C++ data: C++ has no global
    /// thresholds and always reads the owner template. Kept for the
    /// crate-collide wiring that feeds the tracker's fail-closed fallback
    /// path.
    pub fn default_requirements() -> Self {
        Self {
            requirements: [0, 100, 300, 600],
        }
    }

    /// Get experience required for a specific level.
    pub fn get_required(&self, level: VeterancyLevel) -> i32 {
        self.requirements[level as usize]
    }

    /// Get the full requirements array.
    pub fn as_array(&self) -> &[i32; 4] {
        &self.requirements
    }
}

impl Default for ExperienceRequirements {
    fn default() -> Self {
        Self::default_requirements()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_requirements() {
        let req = ExperienceRequirements::default_requirements();

        assert_eq!(req.get_required(VeterancyLevel::Regular), 0);
        assert_eq!(req.get_required(VeterancyLevel::Veteran), 100);
        assert_eq!(req.get_required(VeterancyLevel::Elite), 300);
        assert_eq!(req.get_required(VeterancyLevel::Heroic), 600);
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
    fn test_as_array() {
        let req = ExperienceRequirements::from_array([0, 1000, 3000, 6000]);
        let array = req.as_array();

        assert_eq!(array.len(), 4);
        assert_eq!(array[0], 0);
        assert_eq!(array[1], 1000);
        assert_eq!(array[2], 3000);
        assert_eq!(array[3], 6000);
    }
}
