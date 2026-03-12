//! Veterancy Integration - Apply bonuses to weapons, armor, and other systems
//!
//! This module provides the integration layer between the veterancy system
//! and the game's combat, movement, and other systems, matching C++ behavior.

use crate::common::types::VeterancyLevel;
use crate::experience::VeterancyBonuses;

/// Weapon damage calculator with veterancy bonuses
///
/// Applies veterancy damage multipliers to weapon damage values,
/// matching the C++ Weapon::GetActualDamage() implementation
pub struct VeterancyWeaponCalculator;

impl VeterancyWeaponCalculator {
    /// Calculate actual damage with veterancy bonuses
    ///
    /// # Parameters
    /// - `base_damage`: Base weapon damage
    /// - `level`: Veterancy level of the attacker
    ///
    /// # Returns
    /// Modified damage with veterancy bonuses applied
    ///
    /// # Formula (matching C++ exactly)
    /// ```text
    /// actual_damage = base_damage * (1.0 + level * 0.25)
    /// - Regular (0): 1.0x damage
    /// - Veteran (1): 1.25x damage
    /// - Elite (2): 1.5x damage
    /// - Heroic (3): 2.0x damage
    /// ```
    pub fn calculate_damage(base_damage: f32, level: VeterancyLevel) -> f32 {
        let bonuses = VeterancyBonuses::for_level(level);
        bonuses.apply_damage_bonus(base_damage)
    }

    /// Calculate rate of fire with veterancy bonuses
    ///
    /// # Parameters
    /// - `base_reload_time`: Base weapon reload time in seconds
    /// - `level`: Veterancy level of the attacker
    ///
    /// # Returns
    /// Modified reload time (lower = faster firing)
    pub fn calculate_reload_time(base_reload_time: f32, level: VeterancyLevel) -> f32 {
        let bonuses = VeterancyBonuses::for_level(level);
        bonuses.apply_rate_of_fire_bonus(base_reload_time)
    }

    /// Calculate weapon range with veterancy bonuses
    ///
    /// Note: In original C++, weapon range doesn't scale with veterancy,
    /// but sight range does. Including for completeness.
    ///
    /// # Parameters
    /// - `base_range`: Base weapon range
    /// - `level`: Veterancy level
    ///
    /// # Returns
    /// Base range (unchanged in standard gameplay)
    pub fn calculate_range(base_range: f32, _level: VeterancyLevel) -> f32 {
        // In C++, weapon range is not affected by veterancy
        // Only sight range is affected
        base_range
    }
}

/// Armor damage reduction calculator with veterancy bonuses
///
/// Applies veterancy armor multipliers to incoming damage,
/// matching the C++ Object::onDamageReceived() implementation
pub struct VeterancyArmorCalculator;

impl VeterancyArmorCalculator {
    /// Calculate incoming damage after veterancy armor bonuses
    ///
    /// # Parameters
    /// - `incoming_damage`: Damage before armor
    /// - `level`: Veterancy level of the defender
    ///
    /// # Returns
    /// Reduced damage with veterancy armor applied
    ///
    /// # Formula (matching C++ exactly)
    /// ```text
    /// reduced_damage = incoming_damage * (1.0 - level * 0.1)
    /// - Regular (0): 1.0x damage taken
    /// - Veteran (1): 0.9x damage taken (-10%)
    /// - Elite (2): 0.8x damage taken (-20%)
    /// - Heroic (3): 0.7x damage taken (-30%)
    /// ```
    pub fn calculate_damage_taken(incoming_damage: f32, level: VeterancyLevel) -> f32 {
        let bonuses = VeterancyBonuses::for_level(level);
        bonuses.apply_armor_bonus(incoming_damage)
    }

    /// Check if this veterancy level has damage resistance
    pub fn has_armor_bonus(level: VeterancyLevel) -> bool {
        level != VeterancyLevel::Regular
    }
}

/// Movement speed calculator with veterancy bonuses
///
/// Applies veterancy speed multipliers to locomotor speed,
/// matching the C++ Locomotor::getActualSpeed() implementation
pub struct VeterancyMovementCalculator;

impl VeterancyMovementCalculator {
    /// Calculate actual movement speed with veterancy bonuses
    ///
    /// # Parameters
    /// - `base_speed`: Base movement speed
    /// - `level`: Veterancy level
    ///
    /// # Returns
    /// Modified speed with veterancy bonuses applied
    ///
    /// # Formula (matching C++ exactly)
    /// ```text
    /// actual_speed = base_speed * (1.0 + level * 0.25)
    /// - Regular (0): 1.0x speed
    /// - Veteran (1): 1.25x speed
    /// - Elite (2): 1.5x speed
    /// - Heroic (3): 2.0x speed
    /// ```
    pub fn calculate_speed(base_speed: f32, level: VeterancyLevel) -> f32 {
        let bonuses = VeterancyBonuses::for_level(level);
        bonuses.apply_speed_bonus(base_speed)
    }
}

/// Vision range calculator with veterancy bonuses
///
/// Applies veterancy sight multipliers to vision range,
/// matching the C++ Object::getSightRange() implementation
pub struct VeterancyVisionCalculator;

impl VeterancyVisionCalculator {
    /// Calculate actual vision range with veterancy bonuses
    ///
    /// # Parameters
    /// - `base_sight`: Base vision range
    /// - `level`: Veterancy level
    ///
    /// # Returns
    /// Modified sight range with veterancy bonuses applied
    ///
    /// # Formula (matching C++ exactly)
    /// ```text
    /// actual_sight = base_sight * (1.0 + level * 0.25)
    /// - Regular (0): 1.0x sight
    /// - Veteran (1): 1.25x sight
    /// - Elite (2): 1.5x sight
    /// - Heroic (3): 2.0x sight
    /// ```
    pub fn calculate_sight_range(base_sight: f32, level: VeterancyLevel) -> f32 {
        let bonuses = VeterancyBonuses::for_level(level);
        bonuses.apply_sight_bonus(base_sight)
    }
}

/// Self-healing calculator for veteran units
///
/// Handles the self-healing mechanic for veteran units,
/// matching the C++ BodyModule::update() implementation
pub struct VeterancySelfHealCalculator;

impl VeterancySelfHealCalculator {
    /// Calculate self-healing amount for a time step
    ///
    /// # Parameters
    /// - `level`: Veterancy level
    /// - `delta_time`: Time elapsed in seconds
    ///
    /// # Returns
    /// Health to restore
    ///
    /// # Formula (matching C++ exactly)
    /// ```text
    /// - Veteran: 0.5 HP/sec
    /// - Elite: 1.0 HP/sec
    /// - Heroic: 2.0 HP/sec
    /// ```
    pub fn calculate_healing(level: VeterancyLevel, delta_time: f32) -> f32 {
        let bonuses = VeterancyBonuses::for_level(level);
        bonuses.get_self_healing_amount(delta_time)
    }

    /// Check if this veterancy level has self-healing
    pub fn has_self_healing(level: VeterancyLevel) -> bool {
        let bonuses = VeterancyBonuses::for_level(level);
        bonuses.has_self_healing()
    }
}

/// Complete veterancy stat calculator
///
/// Provides a unified interface for applying all veterancy bonuses
pub struct VeterancyStatCalculator;

impl VeterancyStatCalculator {
    /// Apply all combat-related veterancy bonuses
    ///
    /// Returns a complete set of modified stats
    pub fn calculate_combat_stats(level: VeterancyLevel, base_stats: &CombatStats) -> CombatStats {
        CombatStats {
            damage: VeterancyWeaponCalculator::calculate_damage(base_stats.damage, level),
            armor_damage_reduction: VeterancyArmorCalculator::calculate_damage_taken(1.0, level),
            reload_time: VeterancyWeaponCalculator::calculate_reload_time(
                base_stats.reload_time,
                level,
            ),
            range: base_stats.range, // Not affected by veterancy
            speed: VeterancyMovementCalculator::calculate_speed(base_stats.speed, level),
            sight: VeterancyVisionCalculator::calculate_sight_range(base_stats.sight, level),
            self_heal_rate: VeterancySelfHealCalculator::calculate_healing(level, 1.0), // Per second
        }
    }

    /// Get the damage multiplier for a level (for UI display)
    pub fn get_damage_multiplier(level: VeterancyLevel) -> f32 {
        VeterancyBonuses::for_level(level).damage_multiplier
    }

    /// Get the armor multiplier for a level (for UI display)
    pub fn get_armor_multiplier(level: VeterancyLevel) -> f32 {
        VeterancyBonuses::for_level(level).armor_multiplier
    }

    /// Get the speed multiplier for a level (for UI display)
    pub fn get_speed_multiplier(level: VeterancyLevel) -> f32 {
        VeterancyBonuses::for_level(level).speed_multiplier
    }
}

/// Combat stats structure for veterancy calculations
#[derive(Debug, Clone, Copy)]
pub struct CombatStats {
    /// Weapon damage
    pub damage: f32,

    /// Armor damage reduction (1.0 = no reduction)
    pub armor_damage_reduction: f32,

    /// Reload time in seconds
    pub reload_time: f32,

    /// Weapon/sight range
    pub range: f32,

    /// Movement speed
    pub speed: f32,

    /// Vision range
    pub sight: f32,

    /// Self-healing rate (HP per second)
    pub self_heal_rate: f32,
}

impl CombatStats {
    /// Create base combat stats (Regular level)
    pub fn new(damage: f32, reload_time: f32, range: f32, speed: f32, sight: f32) -> Self {
        Self {
            damage,
            armor_damage_reduction: 1.0,
            reload_time,
            range,
            speed,
            sight,
            self_heal_rate: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weapon_damage_calculation() {
        let base_damage = 100.0;

        assert_eq!(
            VeterancyWeaponCalculator::calculate_damage(base_damage, VeterancyLevel::Regular),
            100.0
        );
        assert_eq!(
            VeterancyWeaponCalculator::calculate_damage(base_damage, VeterancyLevel::Veteran),
            125.0
        );
        assert_eq!(
            VeterancyWeaponCalculator::calculate_damage(base_damage, VeterancyLevel::Elite),
            150.0
        );
        assert_eq!(
            VeterancyWeaponCalculator::calculate_damage(base_damage, VeterancyLevel::Heroic),
            200.0
        );
    }

    #[test]
    fn test_armor_damage_reduction() {
        let incoming = 100.0;

        assert_eq!(
            VeterancyArmorCalculator::calculate_damage_taken(incoming, VeterancyLevel::Regular),
            100.0
        );
        assert_eq!(
            VeterancyArmorCalculator::calculate_damage_taken(incoming, VeterancyLevel::Veteran),
            90.0
        );
        assert_eq!(
            VeterancyArmorCalculator::calculate_damage_taken(incoming, VeterancyLevel::Elite),
            75.0
        );
        assert_eq!(
            VeterancyArmorCalculator::calculate_damage_taken(incoming, VeterancyLevel::Heroic),
            50.0
        );
    }

    #[test]
    fn test_reload_time_calculation() {
        let base_reload = 3.0;

        let regular =
            VeterancyWeaponCalculator::calculate_reload_time(base_reload, VeterancyLevel::Regular);
        assert_eq!(regular, 3.0);

        let veteran =
            VeterancyWeaponCalculator::calculate_reload_time(base_reload, VeterancyLevel::Veteran);
        assert!((veteran - 2.6087).abs() < 0.01); // 3.0 / 1.15

        let heroic =
            VeterancyWeaponCalculator::calculate_reload_time(base_reload, VeterancyLevel::Heroic);
        assert_eq!(heroic, 2.0); // 3.0 / 1.5
    }

    #[test]
    fn test_speed_calculation() {
        let base_speed = 50.0;

        assert_eq!(
            VeterancyMovementCalculator::calculate_speed(base_speed, VeterancyLevel::Regular),
            50.0
        );
        assert_eq!(
            VeterancyMovementCalculator::calculate_speed(base_speed, VeterancyLevel::Veteran),
            55.0
        );
        assert_eq!(
            VeterancyMovementCalculator::calculate_speed(base_speed, VeterancyLevel::Elite),
            75.0
        );
        assert_eq!(
            VeterancyMovementCalculator::calculate_speed(base_speed, VeterancyLevel::Heroic),
            100.0
        );
    }

    #[test]
    fn test_sight_calculation() {
        let base_sight = 200.0;

        assert_eq!(
            VeterancyVisionCalculator::calculate_sight_range(base_sight, VeterancyLevel::Regular),
            200.0
        );
        assert_eq!(
            VeterancyVisionCalculator::calculate_sight_range(base_sight, VeterancyLevel::Veteran),
            220.0
        );
        assert_eq!(
            VeterancyVisionCalculator::calculate_sight_range(base_sight, VeterancyLevel::Elite),
            300.0
        );
        assert_eq!(
            VeterancyVisionCalculator::calculate_sight_range(base_sight, VeterancyLevel::Heroic),
            400.0
        );
    }

    #[test]
    fn test_self_healing() {
        assert!(!VeterancySelfHealCalculator::has_self_healing(
            VeterancyLevel::Regular
        ));
        assert!(VeterancySelfHealCalculator::has_self_healing(
            VeterancyLevel::Veteran
        ));

        // 5 seconds at Veteran = 2.5 HP
        let healing = VeterancySelfHealCalculator::calculate_healing(VeterancyLevel::Veteran, 5.0);
        assert_eq!(healing, 2.5);

        // 5 seconds at Heroic = 10 HP
        let healing = VeterancySelfHealCalculator::calculate_healing(VeterancyLevel::Heroic, 5.0);
        assert_eq!(healing, 10.0);
    }

    #[test]
    fn test_complete_stat_calculation() {
        let base_stats = CombatStats::new(
            100.0, // damage
            3.0,   // reload
            400.0, // range
            50.0,  // speed
            200.0, // sight
        );

        let veteran_stats =
            VeterancyStatCalculator::calculate_combat_stats(VeterancyLevel::Veteran, &base_stats);

        assert_eq!(veteran_stats.damage, 125.0);
        assert_eq!(veteran_stats.armor_damage_reduction, 0.9);
        assert!((veteran_stats.reload_time - 2.6087).abs() < 0.01);
        assert_eq!(veteran_stats.range, 400.0); // Unchanged
        assert_eq!(veteran_stats.speed, 55.0);
        assert_eq!(veteran_stats.sight, 220.0);
        assert_eq!(veteran_stats.self_heal_rate, 0.5);
    }

    #[test]
    fn test_heroic_stats() {
        let base_stats = CombatStats::new(50.0, 4.0, 300.0, 40.0, 150.0);

        let heroic_stats =
            VeterancyStatCalculator::calculate_combat_stats(VeterancyLevel::Heroic, &base_stats);

        // All stats should be doubled (except armor)
        assert_eq!(heroic_stats.damage, 100.0); // 2x
        assert_eq!(heroic_stats.armor_damage_reduction, 0.5); // Takes 50% damage
        assert!((heroic_stats.reload_time - 2.6667).abs() < 0.01); // 4.0 / 1.5
        assert_eq!(heroic_stats.speed, 80.0); // 2x
        assert_eq!(heroic_stats.sight, 300.0); // 2x
        assert_eq!(heroic_stats.self_heal_rate, 2.0); // 2 HP/sec
    }

    #[test]
    fn test_multiplier_getters() {
        assert_eq!(
            VeterancyStatCalculator::get_damage_multiplier(VeterancyLevel::Regular),
            1.0
        );
        assert_eq!(
            VeterancyStatCalculator::get_damage_multiplier(VeterancyLevel::Veteran),
            1.25
        );

        assert_eq!(
            VeterancyStatCalculator::get_armor_multiplier(VeterancyLevel::Elite),
            0.75
        );

        assert_eq!(
            VeterancyStatCalculator::get_speed_multiplier(VeterancyLevel::Heroic),
            2.0
        );
    }

    #[test]
    fn test_weapon_range_unchanged() {
        // Verify weapon range is not affected by veterancy (only sight is)
        let base_range = 500.0;

        assert_eq!(
            VeterancyWeaponCalculator::calculate_range(base_range, VeterancyLevel::Regular),
            500.0
        );
        assert_eq!(
            VeterancyWeaponCalculator::calculate_range(base_range, VeterancyLevel::Heroic),
            500.0
        );
    }

    #[test]
    fn test_armor_bonus_check() {
        assert!(!VeterancyArmorCalculator::has_armor_bonus(
            VeterancyLevel::Regular
        ));
        assert!(VeterancyArmorCalculator::has_armor_bonus(
            VeterancyLevel::Veteran
        ));
        assert!(VeterancyArmorCalculator::has_armor_bonus(
            VeterancyLevel::Elite
        ));
        assert!(VeterancyArmorCalculator::has_armor_bonus(
            VeterancyLevel::Heroic
        ));
    }
}
