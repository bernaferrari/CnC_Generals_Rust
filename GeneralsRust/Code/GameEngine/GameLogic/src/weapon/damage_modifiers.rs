//! Damage Modifiers System
//!
//! This module implements advanced damage modification systems:
//! - Critical hits and headshots
//! - Damage variance and randomization
//! - Range falloff and dropoff
//! - Accuracy and spread effects on damage
//! - Environmental and situational modifiers

use rand::Rng;

use crate::common::{Coord3D, VeterancyLevel};
use crate::weapon::armor_system::ArmorType;
use crate::weapon::DamageType;

/// Critical hit configuration
#[derive(Debug, Clone)]
pub struct CriticalHitConfig {
    /// Base critical hit chance (0.0 to 1.0)
    pub base_crit_chance: f32,
    /// Critical hit damage multiplier
    pub crit_damage_multiplier: f32,
    /// Bonus crit chance per veterancy level
    pub veterancy_crit_bonus: [f32; 4],
    /// Whether sniper weapons always crit on infantry
    pub sniper_auto_crit_infantry: bool,
    /// Whether headshots are possible
    pub enable_headshots: bool,
    /// Headshot damage multiplier
    pub headshot_multiplier: f32,
}

impl CriticalHitConfig {
    /// Create default configuration
    pub fn new() -> Self {
        Self {
            base_crit_chance: 0.05,                        // 5% base crit chance
            crit_damage_multiplier: 2.0,                   // Double damage on crit
            veterancy_crit_bonus: [0.0, 0.02, 0.05, 0.10], // +2%, +5%, +10% for veteran/elite/heroic
            sniper_auto_crit_infantry: true,
            enable_headshots: true,
            headshot_multiplier: 3.0, // Triple damage for headshots
        }
    }

    /// Get total crit chance for a given veterancy level
    pub fn get_crit_chance(&self, veterancy: VeterancyLevel) -> f32 {
        let base = self.base_crit_chance;
        let bonus = self
            .veterancy_crit_bonus
            .get(veterancy as usize)
            .copied()
            .unwrap_or(0.0);
        (base + bonus).min(0.75) // Cap at 75% crit chance
    }
}

impl Default for CriticalHitConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Damage variance configuration
#[derive(Debug, Clone)]
pub struct DamageVarianceConfig {
    /// Enable damage randomization
    pub enabled: bool,
    /// Minimum damage multiplier (e.g., 0.9 = 90% of base)
    pub min_multiplier: f32,
    /// Maximum damage multiplier (e.g., 1.1 = 110% of base)
    pub max_multiplier: f32,
    /// Use normal distribution (vs uniform)
    pub use_normal_distribution: bool,
    /// Standard deviation for normal distribution
    pub std_deviation: f32,
}

impl DamageVarianceConfig {
    /// Create default configuration
    pub fn new() -> Self {
        Self {
            enabled: true,
            min_multiplier: 0.9,
            max_multiplier: 1.1,
            use_normal_distribution: true,
            std_deviation: 0.05, // Small variance around mean
        }
    }

    /// No variance configuration (for testing)
    pub fn no_variance() -> Self {
        Self {
            enabled: false,
            min_multiplier: 1.0,
            max_multiplier: 1.0,
            use_normal_distribution: false,
            std_deviation: 0.0,
        }
    }
}

impl Default for DamageVarianceConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Range falloff configuration
#[derive(Debug, Clone)]
pub struct RangeFalloffConfig {
    /// Enable range-based damage falloff
    pub enabled: bool,
    /// Damage at minimum range (1.0 = full damage)
    pub damage_at_min_range: f32,
    /// Damage at maximum range (0.5 = half damage)
    pub damage_at_max_range: f32,
    /// Falloff curve type
    pub falloff_curve: FalloffCurve,
    /// Range threshold for falloff start (percentage of max range)
    pub falloff_start_threshold: f32,
}

/// Falloff curve types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FalloffCurve {
    /// Linear falloff
    Linear,
    /// Quadratic falloff (damage drops faster at range)
    Quadratic,
    /// Inverse quadratic (damage holds longer then drops)
    InverseQuadratic,
    /// Exponential falloff
    Exponential,
}

impl RangeFalloffConfig {
    /// Create default configuration
    pub fn new() -> Self {
        Self {
            enabled: true,
            damage_at_min_range: 1.0,
            damage_at_max_range: 0.5,
            falloff_curve: FalloffCurve::Linear,
            falloff_start_threshold: 0.5, // Falloff starts at 50% of max range
        }
    }

    /// No falloff configuration
    pub fn no_falloff() -> Self {
        Self {
            enabled: false,
            damage_at_min_range: 1.0,
            damage_at_max_range: 1.0,
            falloff_curve: FalloffCurve::Linear,
            falloff_start_threshold: 1.0,
        }
    }
}

impl Default for RangeFalloffConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Complete damage modifier system
pub struct DamageModifierSystem {
    crit_config: CriticalHitConfig,
    variance_config: DamageVarianceConfig,
    falloff_config: RangeFalloffConfig,
}

impl DamageModifierSystem {
    /// Create new damage modifier system
    pub fn new(
        crit_config: CriticalHitConfig,
        variance_config: DamageVarianceConfig,
        falloff_config: RangeFalloffConfig,
    ) -> Self {
        Self {
            crit_config,
            variance_config,
            falloff_config,
        }
    }

    /// Create with default configurations
    pub fn with_defaults() -> Self {
        Self::new(
            CriticalHitConfig::new(),
            DamageVarianceConfig::new(),
            RangeFalloffConfig::new(),
        )
    }

    /// Check if attack is a critical hit
    pub fn check_critical_hit(
        &self,
        damage_type: DamageType,
        target_armor: ArmorType,
        attacker_veterancy: VeterancyLevel,
    ) -> bool {
        let mut rng = rand::thread_rng();

        // Sniper weapons always crit on infantry
        if self.crit_config.sniper_auto_crit_infantry
            && damage_type == DamageType::Sniper
            && target_armor == ArmorType::Human
        {
            return true;
        }

        let crit_chance = self.crit_config.get_crit_chance(attacker_veterancy);
        rng.gen::<f32>() < crit_chance
    }

    /// Apply critical hit multiplier
    pub fn apply_critical_multiplier(&self, base_damage: f32, is_critical: bool) -> f32 {
        if is_critical {
            base_damage * self.crit_config.crit_damage_multiplier
        } else {
            base_damage
        }
    }

    /// Apply damage variance
    pub fn apply_variance(&self, base_damage: f32) -> f32 {
        if !self.variance_config.enabled {
            return base_damage;
        }

        let mut rng = rand::thread_rng();

        let multiplier = if self.variance_config.use_normal_distribution {
            // Normal distribution centered at 1.0
            use rand_distr::{Distribution, Normal};
            let normal = Normal::new(1.0, self.variance_config.std_deviation).unwrap();
            normal.sample(&mut rng).clamp(
                self.variance_config.min_multiplier,
                self.variance_config.max_multiplier,
            )
        } else {
            // Uniform distribution
            rng.gen_range(self.variance_config.min_multiplier..=self.variance_config.max_multiplier)
        };

        base_damage * multiplier
    }

    /// Calculate range falloff multiplier
    pub fn calculate_range_falloff(&self, distance: f32, min_range: f32, max_range: f32) -> f32 {
        if !self.falloff_config.enabled {
            return 1.0;
        }

        // Clamp distance to valid range
        let distance = distance.clamp(min_range, max_range);

        // Calculate falloff start distance
        let falloff_start =
            min_range + (max_range - min_range) * self.falloff_config.falloff_start_threshold;

        // No falloff before threshold
        if distance <= falloff_start {
            return self.falloff_config.damage_at_min_range;
        }

        // Calculate normalized range (0.0 at falloff_start, 1.0 at max_range)
        let range_normalized = (distance - falloff_start) / (max_range - falloff_start);

        // Apply falloff curve
        let falloff_factor = match self.falloff_config.falloff_curve {
            FalloffCurve::Linear => range_normalized,
            // "Quadratic" falloff is steeper than linear (more drop at mid-range).
            FalloffCurve::Quadratic => {
                let inv = 1.0 - range_normalized;
                1.0 - (inv * inv)
            }
            // Inverse quadratic holds damage longer, then drops towards max range.
            FalloffCurve::InverseQuadratic => range_normalized * range_normalized,
            FalloffCurve::Exponential => {
                // e^(-2x) approximation
                (-2.0 * range_normalized).exp()
            }
        };

        // Interpolate between min and max damage
        let min_dmg = self.falloff_config.damage_at_min_range;
        let max_dmg = self.falloff_config.damage_at_max_range;
        min_dmg + (max_dmg - min_dmg) * falloff_factor
    }

    /// Apply all damage modifiers
    pub fn apply_all_modifiers(
        &self,
        base_damage: f32,
        damage_type: DamageType,
        target_armor: ArmorType,
        attacker_veterancy: VeterancyLevel,
        distance: f32,
        min_range: f32,
        max_range: f32,
    ) -> DamageModifierResult {
        let mut result = DamageModifierResult {
            base_damage,
            final_damage: base_damage,
            is_critical: false,
            crit_multiplier: 1.0,
            variance_multiplier: 1.0,
            range_multiplier: 1.0,
        };

        // Check critical hit
        result.is_critical = self.check_critical_hit(damage_type, target_armor, attacker_veterancy);
        if result.is_critical {
            result.crit_multiplier = self.crit_config.crit_damage_multiplier;
            result.final_damage *= result.crit_multiplier;
        }

        // Apply variance
        result.variance_multiplier = if self.variance_config.enabled {
            self.apply_variance(1.0) // Get multiplier
        } else {
            1.0
        };
        result.final_damage *= result.variance_multiplier;

        // Apply range falloff
        result.range_multiplier = self.calculate_range_falloff(distance, min_range, max_range);
        result.final_damage *= result.range_multiplier;

        result
    }

    /// Get configuration references
    pub fn crit_config(&self) -> &CriticalHitConfig {
        &self.crit_config
    }

    pub fn variance_config(&self) -> &DamageVarianceConfig {
        &self.variance_config
    }

    pub fn falloff_config(&self) -> &RangeFalloffConfig {
        &self.falloff_config
    }
}

impl Default for DamageModifierSystem {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// Result of applying damage modifiers
#[derive(Debug, Clone)]
pub struct DamageModifierResult {
    /// Base damage before modifiers
    pub base_damage: f32,
    /// Final damage after all modifiers
    pub final_damage: f32,
    /// Whether this was a critical hit
    pub is_critical: bool,
    /// Critical hit multiplier applied
    pub crit_multiplier: f32,
    /// Variance multiplier applied
    pub variance_multiplier: f32,
    /// Range falloff multiplier applied
    pub range_multiplier: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crit_chance_calculation() {
        let config = CriticalHitConfig::new();

        assert_eq!(config.get_crit_chance(VeterancyLevel::Regular), 0.05);
        assert_eq!(config.get_crit_chance(VeterancyLevel::Veteran), 0.07);
        assert_eq!(config.get_crit_chance(VeterancyLevel::Elite), 0.10);
        assert_eq!(config.get_crit_chance(VeterancyLevel::Heroic), 0.15);
    }

    #[test]
    fn test_sniper_auto_crit() {
        let system = DamageModifierSystem::with_defaults();

        // Sniper vs infantry should always crit
        let is_crit = system.check_critical_hit(
            DamageType::Sniper,
            ArmorType::Human,
            VeterancyLevel::Regular,
        );
        assert!(is_crit);

        // Sniper vs tank should not always crit
        let mut any_non_crit = false;
        for _ in 0..100 {
            let is_crit = system.check_critical_hit(
                DamageType::Sniper,
                ArmorType::Tank,
                VeterancyLevel::Regular,
            );
            if !is_crit {
                any_non_crit = true;
                break;
            }
        }
        assert!(any_non_crit, "Sniper vs tank should sometimes not crit");
    }

    #[test]
    fn test_critical_multiplier() {
        let system = DamageModifierSystem::with_defaults();

        assert_eq!(system.apply_critical_multiplier(100.0, false), 100.0);
        assert_eq!(system.apply_critical_multiplier(100.0, true), 200.0);
    }

    #[test]
    fn test_damage_variance_disabled() {
        let config = DamageVarianceConfig::no_variance();
        let system =
            DamageModifierSystem::new(CriticalHitConfig::new(), config, RangeFalloffConfig::new());

        let damage = system.apply_variance(100.0);
        assert_eq!(damage, 100.0);
    }

    #[test]
    fn test_damage_variance_range() {
        let system = DamageModifierSystem::with_defaults();

        for _ in 0..100 {
            let damage = system.apply_variance(100.0);
            assert!(
                damage >= 90.0 && damage <= 110.0,
                "Damage {} out of range",
                damage
            );
        }
    }

    #[test]
    fn test_range_falloff_linear() {
        let system = DamageModifierSystem::with_defaults();

        // At minimum range
        let mult = system.calculate_range_falloff(10.0, 10.0, 100.0);
        assert_eq!(mult, 1.0);

        // At falloff start (50% of range = 55.0)
        let mult = system.calculate_range_falloff(55.0, 10.0, 100.0);
        assert_eq!(mult, 1.0);

        // At maximum range
        let mult = system.calculate_range_falloff(100.0, 10.0, 100.0);
        assert_eq!(mult, 0.5);

        // Halfway through falloff zone (77.5)
        let mult = system.calculate_range_falloff(77.5, 10.0, 100.0);
        assert!((mult - 0.75).abs() < 0.01);
    }

    #[test]
    fn test_range_falloff_disabled() {
        let config = RangeFalloffConfig::no_falloff();
        let system = DamageModifierSystem::new(
            CriticalHitConfig::new(),
            DamageVarianceConfig::new(),
            config,
        );

        let mult = system.calculate_range_falloff(100.0, 10.0, 100.0);
        assert_eq!(mult, 1.0);
    }

    #[test]
    fn test_falloff_curves() {
        let linear_config = RangeFalloffConfig {
            falloff_curve: FalloffCurve::Linear,
            ..RangeFalloffConfig::new()
        };
        let quad_config = RangeFalloffConfig {
            falloff_curve: FalloffCurve::Quadratic,
            ..RangeFalloffConfig::new()
        };

        let linear_system = DamageModifierSystem::new(
            CriticalHitConfig::new(),
            DamageVarianceConfig::new(),
            linear_config,
        );
        let quad_system = DamageModifierSystem::new(
            CriticalHitConfig::new(),
            DamageVarianceConfig::new(),
            quad_config,
        );

        // At 75% of range, quadratic should have more damage than linear
        let linear_mult = linear_system.calculate_range_falloff(77.5, 10.0, 100.0);
        let quad_mult = quad_system.calculate_range_falloff(77.5, 10.0, 100.0);

        // Quadratic falloff should be steeper, so lower multiplier
        assert!(quad_mult < linear_mult);
    }

    #[test]
    fn test_apply_all_modifiers() {
        let crit_never = CriticalHitConfig {
            base_crit_chance: 0.0,
            veterancy_crit_bonus: [0.0; 4],
            sniper_auto_crit_infantry: false,
            enable_headshots: false,
            ..CriticalHitConfig::new()
        };
        let config_no_variance = DamageVarianceConfig::no_variance();
        let system =
            DamageModifierSystem::new(crit_never, config_no_variance, RangeFalloffConfig::new());

        // Non-crit at min range
        let result = system.apply_all_modifiers(
            100.0,
            DamageType::SmallArms,
            ArmorType::Tank,
            VeterancyLevel::Regular,
            10.0,
            10.0,
            100.0,
        );

        assert!(!result.is_critical);
        assert_eq!(result.base_damage, 100.0);
        assert_eq!(result.range_multiplier, 1.0);
        assert_eq!(result.final_damage, 100.0);

        // At max range
        let result = system.apply_all_modifiers(
            100.0,
            DamageType::SmallArms,
            ArmorType::Tank,
            VeterancyLevel::Regular,
            100.0,
            10.0,
            100.0,
        );

        assert_eq!(result.range_multiplier, 0.5);
        assert_eq!(result.final_damage, 50.0);
    }

    #[test]
    fn test_sniper_auto_crit_in_full_modifiers() {
        let config_no_variance = DamageVarianceConfig::no_variance();
        let system = DamageModifierSystem::new(
            CriticalHitConfig::new(),
            config_no_variance,
            RangeFalloffConfig::new(),
        );

        let result = system.apply_all_modifiers(
            100.0,
            DamageType::Sniper,
            ArmorType::Human,
            VeterancyLevel::Regular,
            50.0,
            10.0,
            100.0,
        );

        assert!(result.is_critical);
        assert_eq!(result.crit_multiplier, 2.0);
    }
}
