//! Veterancy Bonuses - Stat multipliers and special abilities per level
//!
//! This module defines the bonuses that units receive at each veterancy level,
//! matching the C++ implementation from Generals Zero Hour.

use crate::common::types::VeterancyLevel;

/// Veterancy bonuses for a specific level (matches C++ behavior)
///
/// These bonuses are applied on top of base stats when a unit gains a veterancy level.
/// All multipliers are relative to base stats (1.0 = no change).
#[derive(Debug, Clone)]
pub struct VeterancyBonuses {
    /// Damage dealt multiplier (e.g., 1.25 = +25% damage)
    pub damage_multiplier: f32,

    /// Damage taken multiplier (armor) (e.g., 0.9 = takes 10% less damage)
    pub armor_multiplier: f32,

    /// Sight range multiplier (e.g., 1.25 = +25% vision)
    pub sight_multiplier: f32,

    /// Movement speed multiplier (e.g., 1.25 = +25% speed)
    pub speed_multiplier: f32,

    /// Rate of fire multiplier (e.g., 1.15 = +15% faster firing)
    pub rate_of_fire_multiplier: f32,

    /// Self-healing rate in HP per second (0 = no self-heal)
    pub self_healing_rate: f32,

    /// Experience gain multiplier (e.g., 1.25 = gains 25% more XP)
    pub experience_multiplier: f32,

    /// Maximum health bonus (percentage, e.g., 0.25 = +25% max health)
    pub max_health_bonus: f32,
}

impl VeterancyBonuses {
    /// Regular level bonuses (no bonuses)
    pub fn regular() -> Self {
        Self {
            damage_multiplier: 1.0,
            armor_multiplier: 1.0,
            sight_multiplier: 1.0,
            speed_multiplier: 1.0,
            rate_of_fire_multiplier: 1.0,
            self_healing_rate: 0.0,
            experience_multiplier: 1.0,
            max_health_bonus: 0.0,
        }
    }

    /// Veteran level bonuses (matches C++ behavior)
    ///
    /// Bonuses:
    /// - +25% damage
    /// - -10% damage taken (10% better armor)
    /// - +10% sight range
    /// - +10% speed
    /// - +15% rate of fire
    /// - Slow self-healing (0.5 HP/sec)
    pub fn veteran() -> Self {
        Self {
            damage_multiplier: 1.25,       // +25% damage
            armor_multiplier: 0.9,         // Takes 10% less damage
            sight_multiplier: 1.1,         // +10% vision
            speed_multiplier: 1.1,         // +10% speed
            rate_of_fire_multiplier: 1.15, // +15% rate of fire
            self_healing_rate: 0.5,        // 0.5 HP per second
            experience_multiplier: 1.25,   // +25% XP gain
            max_health_bonus: 0.0,         // No max health bonus at Veteran
        }
    }

    /// Elite level bonuses (matches C++ behavior)
    ///
    /// Bonuses:
    /// - +50% damage
    /// - -25% damage taken (25% better armor)
    /// - +50% sight range
    /// - +50% speed
    /// - +25% rate of fire
    /// - Moderate self-healing (1.0 HP/sec)
    pub fn elite() -> Self {
        Self {
            damage_multiplier: 1.5,        // +50% damage
            armor_multiplier: 0.75,        // Takes 25% less damage
            sight_multiplier: 1.5,         // +50% vision
            speed_multiplier: 1.5,         // +50% speed
            rate_of_fire_multiplier: 1.25, // +25% rate of fire
            self_healing_rate: 1.0,        // 1.0 HP per second
            experience_multiplier: 1.5,    // +50% XP gain
            max_health_bonus: 0.0,         // No max health bonus at Elite
        }
    }

    /// Heroic level bonuses (matches C++ behavior)
    ///
    /// Bonuses:
    /// - +100% damage (double damage)
    /// - -50% damage taken (50% better armor)
    /// - +100% sight range (double vision)
    /// - +100% speed (double speed)
    /// - +50% rate of fire
    /// - Fast self-healing (2.0 HP/sec)
    pub fn heroic() -> Self {
        Self {
            damage_multiplier: 2.0,       // +100% damage (double)
            armor_multiplier: 0.5,        // Takes 50% less damage
            sight_multiplier: 2.0,        // +100% vision (double)
            speed_multiplier: 2.0,        // +100% speed (double)
            rate_of_fire_multiplier: 1.5, // +50% rate of fire
            self_healing_rate: 2.0,       // 2.0 HP per second
            experience_multiplier: 2.0,   // +100% XP gain
            max_health_bonus: 0.0,        // No max health bonus at Heroic
        }
    }

    /// Get bonuses for a specific veterancy level
    pub fn for_level(level: VeterancyLevel) -> Self {
        match level {
            VeterancyLevel::Regular => Self::regular(),
            VeterancyLevel::Veteran => Self::veteran(),
            VeterancyLevel::Elite => Self::elite(),
            VeterancyLevel::Heroic => Self::heroic(),
        }
    }

    /// Apply damage bonus to base damage
    pub fn apply_damage_bonus(&self, base_damage: f32) -> f32 {
        base_damage * self.damage_multiplier
    }

    /// Apply armor bonus to incoming damage (returns reduced damage)
    pub fn apply_armor_bonus(&self, incoming_damage: f32) -> f32 {
        incoming_damage * self.armor_multiplier
    }

    /// Apply sight bonus to base sight range
    pub fn apply_sight_bonus(&self, base_sight: f32) -> f32 {
        base_sight * self.sight_multiplier
    }

    /// Apply speed bonus to base speed
    pub fn apply_speed_bonus(&self, base_speed: f32) -> f32 {
        base_speed * self.speed_multiplier
    }

    /// Apply rate of fire bonus to base reload time (shorter reload = faster fire)
    pub fn apply_rate_of_fire_bonus(&self, base_reload_time: f32) -> f32 {
        base_reload_time / self.rate_of_fire_multiplier
    }

    /// Check if this level has self-healing
    pub fn has_self_healing(&self) -> bool {
        self.self_healing_rate > 0.0
    }

    /// Get self-healing amount for a time delta
    pub fn get_self_healing_amount(&self, delta_time: f32) -> f32 {
        self.self_healing_rate * delta_time
    }
}

impl Default for VeterancyBonuses {
    fn default() -> Self {
        Self::regular()
    }
}

/// Bonus application helper for weapon systems
///
/// This provides helper methods for applying veterancy bonuses to weapon stats,
/// matching the C++ weapon bonus system.
pub struct VeterancyWeaponBonuses;

impl VeterancyWeaponBonuses {
    /// Calculate damage multiplier for a veterancy level
    ///
    /// Uses the same values as `VeterancyBonuses`.
    pub fn damage_multiplier(level: VeterancyLevel) -> f32 {
        VeterancyBonuses::for_level(level).damage_multiplier
    }

    /// Calculate armor multiplier for a veterancy level (damage taken)
    ///
    /// Uses the same values as `VeterancyBonuses`.
    pub fn armor_multiplier(level: VeterancyLevel) -> f32 {
        VeterancyBonuses::for_level(level).armor_multiplier
    }

    /// Calculate sight range multiplier for a veterancy level
    ///
    /// Uses the same values as `VeterancyBonuses`.
    pub fn sight_multiplier(level: VeterancyLevel) -> f32 {
        VeterancyBonuses::for_level(level).sight_multiplier
    }

    /// Calculate speed multiplier for a veterancy level
    ///
    /// Uses the same values as `VeterancyBonuses`.
    pub fn speed_multiplier(level: VeterancyLevel) -> f32 {
        VeterancyBonuses::for_level(level).speed_multiplier
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regular_bonuses() {
        let bonuses = VeterancyBonuses::regular();
        assert_eq!(bonuses.damage_multiplier, 1.0);
        assert_eq!(bonuses.armor_multiplier, 1.0);
        assert_eq!(bonuses.sight_multiplier, 1.0);
        assert_eq!(bonuses.speed_multiplier, 1.0);
        assert_eq!(bonuses.self_healing_rate, 0.0);
        assert!(!bonuses.has_self_healing());
    }

    #[test]
    fn test_veteran_bonuses() {
        let bonuses = VeterancyBonuses::veteran();
        assert_eq!(bonuses.damage_multiplier, 1.25);
        assert_eq!(bonuses.armor_multiplier, 0.9);
        assert_eq!(bonuses.sight_multiplier, 1.1);
        assert!(bonuses.has_self_healing());
    }

    #[test]
    fn test_elite_bonuses() {
        let bonuses = VeterancyBonuses::elite();
        assert_eq!(bonuses.damage_multiplier, 1.5);
        assert_eq!(bonuses.armor_multiplier, 0.75);
        assert_eq!(bonuses.sight_multiplier, 1.5);
        assert_eq!(bonuses.speed_multiplier, 1.5);
    }

    #[test]
    fn test_heroic_bonuses() {
        let bonuses = VeterancyBonuses::heroic();
        assert_eq!(bonuses.damage_multiplier, 2.0);
        assert_eq!(bonuses.armor_multiplier, 0.5);
        assert_eq!(bonuses.sight_multiplier, 2.0);
        assert_eq!(bonuses.speed_multiplier, 2.0);
    }

    #[test]
    fn test_for_level() {
        let regular = VeterancyBonuses::for_level(VeterancyLevel::Regular);
        assert_eq!(regular.damage_multiplier, 1.0);

        let veteran = VeterancyBonuses::for_level(VeterancyLevel::Veteran);
        assert_eq!(veteran.damage_multiplier, 1.25);

        let elite = VeterancyBonuses::for_level(VeterancyLevel::Elite);
        assert_eq!(elite.damage_multiplier, 1.5);

        let heroic = VeterancyBonuses::for_level(VeterancyLevel::Heroic);
        assert_eq!(heroic.damage_multiplier, 2.0);
    }

    #[test]
    fn test_apply_damage_bonus() {
        let bonuses = VeterancyBonuses::veteran();
        let base_damage = 100.0;
        let modified = bonuses.apply_damage_bonus(base_damage);
        assert_eq!(modified, 125.0); // 100 * 1.25
    }

    #[test]
    fn test_apply_armor_bonus() {
        let bonuses = VeterancyBonuses::veteran();
        let incoming_damage = 100.0;
        let reduced = bonuses.apply_armor_bonus(incoming_damage);
        assert_eq!(reduced, 90.0); // 100 * 0.9
    }

    #[test]
    fn test_apply_sight_bonus() {
        let bonuses = VeterancyBonuses::elite();
        let base_sight = 200.0;
        let modified = bonuses.apply_sight_bonus(base_sight);
        assert_eq!(modified, 300.0); // 200 * 1.5
    }

    #[test]
    fn test_apply_speed_bonus() {
        let bonuses = VeterancyBonuses::heroic();
        let base_speed = 50.0;
        let modified = bonuses.apply_speed_bonus(base_speed);
        assert_eq!(modified, 100.0); // 50 * 2.0
    }

    #[test]
    fn test_apply_rate_of_fire_bonus() {
        let bonuses = VeterancyBonuses::veteran();
        let base_reload = 3.0; // 3 seconds
        let modified = bonuses.apply_rate_of_fire_bonus(base_reload);
        assert!((modified - 2.6087).abs() < 0.01); // 3.0 / 1.15 ≈ 2.61
    }

    #[test]
    fn test_self_healing_amount() {
        let bonuses = VeterancyBonuses::heroic();
        let delta_time = 5.0; // 5 seconds
        let healing = bonuses.get_self_healing_amount(delta_time);
        assert_eq!(healing, 10.0); // 2.0 HP/sec * 5 sec = 10 HP
    }

    #[test]
    fn test_weapon_damage_multiplier() {
        assert_eq!(
            VeterancyWeaponBonuses::damage_multiplier(VeterancyLevel::Regular),
            1.0
        );
        assert_eq!(
            VeterancyWeaponBonuses::damage_multiplier(VeterancyLevel::Veteran),
            1.25
        );
        assert_eq!(
            VeterancyWeaponBonuses::damage_multiplier(VeterancyLevel::Elite),
            1.5
        );
        assert_eq!(
            VeterancyWeaponBonuses::damage_multiplier(VeterancyLevel::Heroic),
            2.0
        );
    }

    #[test]
    fn test_weapon_armor_multiplier() {
        assert_eq!(
            VeterancyWeaponBonuses::armor_multiplier(VeterancyLevel::Regular),
            1.0
        );
        assert_eq!(
            VeterancyWeaponBonuses::armor_multiplier(VeterancyLevel::Veteran),
            0.9
        );
        assert_eq!(
            VeterancyWeaponBonuses::armor_multiplier(VeterancyLevel::Elite),
            0.75
        );
        assert_eq!(
            VeterancyWeaponBonuses::armor_multiplier(VeterancyLevel::Heroic),
            0.5
        );
    }

    #[test]
    fn test_bonus_progression() {
        // Verify bonuses scale correctly across levels
        let regular = VeterancyBonuses::regular();
        let veteran = VeterancyBonuses::veteran();
        let elite = VeterancyBonuses::elite();
        let heroic = VeterancyBonuses::heroic();

        // Damage should increase
        assert!(veteran.damage_multiplier > regular.damage_multiplier);
        assert!(elite.damage_multiplier > veteran.damage_multiplier);
        assert!(heroic.damage_multiplier > elite.damage_multiplier);

        // Armor should improve (lower multiplier = takes less damage)
        assert!(veteran.armor_multiplier < regular.armor_multiplier);
        assert!(elite.armor_multiplier < veteran.armor_multiplier);
        assert!(heroic.armor_multiplier < elite.armor_multiplier);

        // Self-healing should increase
        assert!(veteran.self_healing_rate > regular.self_healing_rate);
        assert!(elite.self_healing_rate > veteran.self_healing_rate);
        assert!(heroic.self_healing_rate > elite.self_healing_rate);
    }
}
