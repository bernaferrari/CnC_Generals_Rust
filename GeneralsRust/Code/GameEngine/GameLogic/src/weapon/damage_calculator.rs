//! Advanced Damage Calculation System
//!
//! This module provides sophisticated damage calculation including armor penetration,
//! damage types, resistances, critical hits, and environmental factors.

use super::{Coord3D, DamageType, DeathType, ObjectId, WeaponBonus, WeaponTemplate};
use crate::{GameLogicError, GameLogicResult};

use std::collections::HashMap;

/// Armor types and their properties
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArmorType {
    /// No armor - full damage
    None,
    /// Light armor - effective against small arms
    Light,
    /// Medium armor - effective against most weapons
    Medium,
    /// Heavy armor - effective against explosives
    Heavy,
    /// Reactive armor - reduces shaped charge effectiveness
    Reactive,
    /// Composite armor - advanced protection
    Composite,
    /// Energy shields - protects against energy weapons
    EnergyShield,
    /// Structural - for buildings
    Structural,
}

/// Armor configuration for an object
#[derive(Debug, Clone)]
pub struct ArmorSet {
    /// Primary armor type
    pub primary_armor: ArmorType,
    /// Armor thickness/strength
    pub armor_value: f32,
    /// Damage type resistances (0.0 = immune, 1.0 = full damage)
    pub resistances: HashMap<DamageType, f32>,
    /// Special armor properties
    pub special_properties: ArmorProperties,
}

/// Special armor properties
#[derive(Debug, Clone, Default)]
pub struct ArmorProperties {
    /// Damage reduction against kinetic weapons
    pub kinetic_reduction: f32,
    /// Damage reduction against explosive weapons  
    pub explosive_reduction: f32,
    /// Damage reduction against energy weapons
    pub energy_reduction: f32,
    /// Chance to deflect attacks (0.0 to 1.0)
    pub deflection_chance: f32,
    /// Armor degradation rate per hit
    pub degradation_rate: f32,
    /// Current armor condition (0.0 to 1.0)
    pub condition: f32,
    /// Whether armor can regenerate
    pub self_repairing: bool,
    /// Regeneration rate per second
    pub repair_rate: f32,
}

/// Damage calculation result
#[derive(Debug, Clone)]
pub struct DamageResult {
    /// Final damage amount after all calculations
    pub final_damage: f32,
    /// Damage before armor calculations
    pub raw_damage: f32,
    /// Damage absorbed by armor
    pub armor_absorption: f32,
    /// Whether this was a critical hit
    pub critical_hit: bool,
    /// Critical hit multiplier applied
    pub critical_multiplier: f32,
    /// Whether attack was deflected
    pub deflected: bool,
    /// Penetration vs armor outcome
    pub penetration_result: PenetrationResult,
    /// Environmental modifiers applied
    pub environmental_modifiers: f32,
    /// Status effects applied
    pub status_effects: Vec<StatusEffect>,
}

/// Armor penetration result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PenetrationResult {
    /// Attack bounced off armor
    Bounced,
    /// Partial penetration - reduced damage
    Partial,
    /// Full penetration - normal damage
    Full,
    /// Overpenetration - reduced damage due to exit
    Overpenetration,
}

/// Status effects that can be applied
#[derive(Debug, Clone)]
pub enum StatusEffect {
    /// Burn damage over time
    Burning {
        damage_per_second: f32,
        duration: f32,
    },
    /// Poison damage over time
    Poisoned {
        damage_per_second: f32,
        duration: f32,
    },
    /// Reduced movement and accuracy
    Suppressed { severity: f32, duration: f32 },
    /// Temporarily disabled systems
    EmpDisabled { duration: f32 },
    /// Reduced morale/effectiveness
    Demoralized { severity: f32, duration: f32 },
    /// Enhanced performance
    Inspired { bonus: f32, duration: f32 },
}

/// Environmental factors affecting damage
#[derive(Debug, Clone)]
pub struct EnvironmentalFactors {
    /// Weather conditions
    pub weather: WeatherCondition,
    /// Terrain type at impact location
    pub terrain: TerrainType,
    /// Cover level (0.0 = no cover, 1.0 = full cover)
    pub cover_level: f32,
    /// Range to target
    pub range: f32,
    /// Elevation difference
    pub elevation_difference: f32,
}

/// Weather conditions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeatherCondition {
    Clear,
    Overcast,
    Rain,
    Snow,
    Fog,
    Storm,
}

/// Terrain types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainType {
    Open,
    Urban,
    Forest,
    Desert,
    Mountain,
    Water,
    Swamp,
}

/// Advanced damage calculator
pub struct DamageCalculator;

impl DamageCalculator {
    /// Calculate damage with full complexity
    pub fn calculate_damage(
        weapon_template: &WeaponTemplate,
        weapon_bonus: &WeaponBonus,
        target_armor: &ArmorSet,
        impact_location: &Coord3D,
        source_location: &Coord3D,
        environmental_factors: &EnvironmentalFactors,
        is_critical_hit: bool,
    ) -> GameLogicResult<DamageResult> {
        // Start with base weapon damage
        let base_damage = weapon_template.get_primary_damage(weapon_bonus);
        let mut current_damage = base_damage;

        // Calculate range modifier
        let range = source_location.distance(*impact_location);
        let range_modifier = Self::calculate_range_modifier(weapon_template, range);
        current_damage *= range_modifier;

        // Apply environmental modifiers
        let env_modifier =
            Self::calculate_environmental_modifier(weapon_template, environmental_factors);
        current_damage *= env_modifier;

        // Store pre-armor damage
        let raw_damage = current_damage;

        // Check for deflection
        let deflected = Self::check_deflection(target_armor, weapon_template);
        if deflected {
            return Ok(DamageResult {
                final_damage: 0.0,
                raw_damage,
                armor_absorption: raw_damage,
                critical_hit: false,
                critical_multiplier: 1.0,
                deflected: true,
                penetration_result: PenetrationResult::Bounced,
                environmental_modifiers: env_modifier,
                status_effects: Vec::new(),
            });
        }

        // Calculate penetration
        let penetration_result =
            Self::calculate_penetration(weapon_template, weapon_bonus, target_armor, range);

        // Apply penetration modifier
        let penetration_modifier = Self::get_penetration_modifier(penetration_result);
        current_damage *= penetration_modifier;

        // Apply armor resistance
        let damage_type = weapon_template.damage_type;
        let resistance = target_armor
            .resistances
            .get(&damage_type)
            .copied()
            .unwrap_or(1.0);
        current_damage *= resistance;

        // Apply armor properties
        current_damage = Self::apply_armor_properties(current_damage, target_armor, damage_type);

        // Apply critical hit multiplier
        let critical_multiplier = if is_critical_hit {
            Self::get_critical_multiplier(weapon_template, target_armor)
        } else {
            1.0
        };
        current_damage *= critical_multiplier;

        // Calculate armor absorption
        let armor_absorption = raw_damage - current_damage;

        // Generate status effects
        let status_effects =
            Self::generate_status_effects(weapon_template, current_damage, environmental_factors);

        Ok(DamageResult {
            final_damage: current_damage.max(0.0),
            raw_damage,
            armor_absorption,
            critical_hit: is_critical_hit,
            critical_multiplier,
            deflected: false,
            penetration_result,
            environmental_modifiers: env_modifier,
            status_effects,
        })
    }

    /// Calculate damage over area for explosive weapons
    pub fn calculate_area_damage(
        weapon_template: &WeaponTemplate,
        weapon_bonus: &WeaponBonus,
        explosion_center: &Coord3D,
        targets: &[(ObjectId, Coord3D, ArmorSet)],
        environmental_factors: &EnvironmentalFactors,
    ) -> GameLogicResult<HashMap<ObjectId, DamageResult>> {
        let mut results = HashMap::new();
        let blast_radius = weapon_template.get_primary_damage_radius(weapon_bonus);

        for (object_id, target_pos, armor) in targets {
            let distance = explosion_center.distance(*target_pos);

            if distance <= blast_radius {
                // Calculate damage falloff
                let falloff_factor = if blast_radius > 0.0 {
                    1.0 - (distance / blast_radius).powf(1.5) // Non-linear falloff
                } else {
                    1.0
                };

                // Create modified weapon template for this distance
                let mut modified_weapon = weapon_template.clone();
                modified_weapon.primary_damage *= falloff_factor;

                // Calculate line-of-sight obstruction
                let los_modifier = Self::calculate_line_of_sight_modifier(
                    explosion_center,
                    target_pos,
                    environmental_factors,
                );
                modified_weapon.primary_damage *= los_modifier;

                // Calculate damage for this target
                let damage_result = Self::calculate_damage(
                    &modified_weapon,
                    weapon_bonus,
                    armor,
                    target_pos,
                    explosion_center,
                    environmental_factors,
                    false, // Area damage typically doesn't crit
                )?;

                results.insert(*object_id, damage_result);
            }
        }

        Ok(results)
    }

    /// Calculate range modifier for damage falloff
    fn calculate_range_modifier(weapon_template: &WeaponTemplate, range: f32) -> f32 {
        let max_range = weapon_template.attack_range;
        let min_range = weapon_template.minimum_attack_range;

        if range <= min_range {
            1.0 // Full damage at minimum range
        } else if range >= max_range {
            0.5 // Half damage at maximum range
        } else {
            // Linear interpolation between min and max range
            let range_ratio = (range - min_range) / (max_range - min_range);
            1.0 - (range_ratio * 0.5) // Damage falls off to 50% at max range
        }
    }

    /// Calculate environmental damage modifiers
    fn calculate_environmental_modifier(
        weapon_template: &WeaponTemplate,
        factors: &EnvironmentalFactors,
    ) -> f32 {
        let mut modifier = 1.0;

        // Weather effects
        modifier *= match factors.weather {
            WeatherCondition::Clear => 1.0,
            WeatherCondition::Overcast => 0.95,
            WeatherCondition::Rain => match weapon_template.damage_type {
                DamageType::Flame => 0.7, // Rain reduces flame damage
                DamageType::Laser => 0.9, // Atmospheric interference
                _ => 0.95,
            },
            WeatherCondition::Snow => match weapon_template.damage_type {
                DamageType::Flame => 0.6, // Snow heavily reduces flame
                _ => 0.9,
            },
            WeatherCondition::Fog => 0.85, // Reduced visibility affects accuracy
            WeatherCondition::Storm => 0.8, // Severe weather interference
        };

        // Terrain effects
        modifier *= match factors.terrain {
            TerrainType::Open => 1.0,
            TerrainType::Urban => match weapon_template.damage_type {
                DamageType::Explosion => 1.2, // Confined spaces amplify explosions
                _ => 0.9,
            },
            TerrainType::Forest => 0.85, // Trees provide some protection
            TerrainType::Desert => match weapon_template.damage_type {
                DamageType::Laser => 1.1, // Clear atmosphere
                _ => 1.0,
            },
            TerrainType::Mountain => 0.9, // Rocky terrain absorbs some damage
            TerrainType::Water => match weapon_template.damage_type {
                DamageType::Explosion => 1.3, // Water transmits shock waves well
                DamageType::Flame => 0.1,     // Water extinguishes fire
                _ => 1.0,
            },
            TerrainType::Swamp => 0.8, // Soft ground absorbs impacts
        };

        // Cover modifier
        modifier *= 1.0 - factors.cover_level * 0.5; // Cover reduces damage by up to 50%

        modifier.max(0.1) // Minimum 10% damage
    }

    /// Check if attack is deflected by armor
    fn check_deflection(armor: &ArmorSet, weapon_template: &WeaponTemplate) -> bool {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        let deflection_chance =
            armor.special_properties.deflection_chance * armor.special_properties.condition; // Damaged armor deflects less

        // Some weapon types are harder to deflect
        let type_modifier = match weapon_template.damage_type {
            DamageType::Explosion => 0.5, // Explosions are harder to deflect
            DamageType::Laser => 0.1,     // Energy weapons rarely deflect
            DamageType::Flame => 0.0,     // Flame cannot be deflected
            _ => 1.0,
        };

        let final_chance = deflection_chance * type_modifier;
        rng.gen::<f32>() < final_chance
    }

    /// Calculate armor penetration result
    fn calculate_penetration(
        weapon_template: &WeaponTemplate,
        weapon_bonus: &WeaponBonus,
        armor: &ArmorSet,
        range: f32,
    ) -> PenetrationResult {
        let weapon_penetration = weapon_template.get_primary_damage(weapon_bonus) * 0.1; // Simplified
        let effective_armor = armor.armor_value * armor.special_properties.condition;

        // Range affects penetration for kinetic weapons
        let range_factor = match weapon_template.damage_type {
            DamageType::SmallArms | DamageType::Combat => {
                (1.0 - (range / weapon_template.attack_range) * 0.3).max(0.7)
            }
            _ => 1.0,
        };

        let adjusted_penetration = weapon_penetration * range_factor;

        if adjusted_penetration < effective_armor * 0.5 {
            PenetrationResult::Bounced
        } else if adjusted_penetration < effective_armor {
            PenetrationResult::Partial
        } else if adjusted_penetration < effective_armor * 2.0 {
            PenetrationResult::Full
        } else {
            PenetrationResult::Overpenetration
        }
    }

    /// Get damage modifier based on penetration result
    fn get_penetration_modifier(result: PenetrationResult) -> f32 {
        match result {
            PenetrationResult::Bounced => 0.1,
            PenetrationResult::Partial => 0.5,
            PenetrationResult::Full => 1.0,
            PenetrationResult::Overpenetration => 0.8, // Some damage lost on exit
        }
    }

    /// Apply armor special properties
    fn apply_armor_properties(damage: f32, armor: &ArmorSet, damage_type: DamageType) -> f32 {
        let mut modified_damage = damage;

        // Apply type-specific reductions
        modified_damage *= match damage_type {
            DamageType::SmallArms | DamageType::Combat => {
                1.0 - armor.special_properties.kinetic_reduction
            }
            DamageType::Explosion => 1.0 - armor.special_properties.explosive_reduction,
            DamageType::Laser | DamageType::Particle => {
                1.0 - armor.special_properties.energy_reduction
            }
            _ => 1.0,
        };

        // Armor condition affects protection
        let condition_factor = armor.special_properties.condition;
        let protection_efficiency = condition_factor * condition_factor; // Quadratic falloff

        // Interpolate between no protection and full protection
        damage * (1.0 - protection_efficiency) + modified_damage * protection_efficiency
    }

    /// Calculate critical hit multiplier
    fn get_critical_multiplier(weapon_template: &WeaponTemplate, armor: &ArmorSet) -> f32 {
        let base_multiplier = 2.0;

        // Some armor types resist critical hits
        let armor_modifier = match armor.primary_armor {
            ArmorType::Heavy | ArmorType::Composite => 0.8,
            ArmorType::Structural => 0.5, // Buildings resist crits
            _ => 1.0,
        };

        base_multiplier * armor_modifier
    }

    /// Generate status effects based on weapon and damage
    fn generate_status_effects(
        weapon_template: &WeaponTemplate,
        damage: f32,
        _environmental_factors: &EnvironmentalFactors,
    ) -> Vec<StatusEffect> {
        let mut effects = Vec::new();

        match weapon_template.damage_type {
            DamageType::Flame => {
                if damage > 10.0 {
                    effects.push(StatusEffect::Burning {
                        damage_per_second: damage * 0.1,
                        duration: 5.0,
                    });
                }
            }
            DamageType::Toxin => {
                effects.push(StatusEffect::Poisoned {
                    damage_per_second: damage * 0.05,
                    duration: 10.0,
                });
            }
            DamageType::Emp => {
                effects.push(StatusEffect::EmpDisabled { duration: 5.0 });
            }
            DamageType::DemoralizingShock => {
                effects.push(StatusEffect::Demoralized {
                    severity: 0.5,
                    duration: 10.0,
                });
            }
            _ => {}
        }

        effects
    }

    /// Calculate line-of-sight modifier for area damage
    fn calculate_line_of_sight_modifier(
        _source: &Coord3D,
        _target: &Coord3D,
        factors: &EnvironmentalFactors,
    ) -> f32 {
        // Simplified LOS calculation based on cover
        1.0 - factors.cover_level * 0.7 // Cover blocks up to 70% of area damage
    }
}

/// Default armor configurations for common unit types
impl ArmorSet {
    /// Create armor set for infantry
    pub fn infantry() -> Self {
        let mut resistances = HashMap::new();
        resistances.insert(DamageType::SmallArms, 0.8);
        resistances.insert(DamageType::Explosion, 1.2);
        resistances.insert(DamageType::Flame, 1.3);
        resistances.insert(DamageType::Toxin, 1.5);

        Self {
            primary_armor: ArmorType::None,
            armor_value: 1.0,
            resistances,
            special_properties: ArmorProperties {
                condition: 1.0,
                ..Default::default()
            },
        }
    }

    /// Create armor set for light vehicles
    pub fn light_vehicle() -> Self {
        let mut resistances = HashMap::new();
        resistances.insert(DamageType::SmallArms, 0.3);
        resistances.insert(DamageType::Explosion, 1.0);
        resistances.insert(DamageType::Combat, 0.8);

        Self {
            primary_armor: ArmorType::Light,
            armor_value: 10.0,
            resistances,
            special_properties: ArmorProperties {
                condition: 1.0,
                kinetic_reduction: 0.2,
                ..Default::default()
            },
        }
    }

    /// Create armor set for heavy tanks
    pub fn heavy_tank() -> Self {
        let mut resistances = HashMap::new();
        resistances.insert(DamageType::SmallArms, 0.1);
        resistances.insert(DamageType::Combat, 0.5);
        resistances.insert(DamageType::Explosion, 0.8);

        Self {
            primary_armor: ArmorType::Heavy,
            armor_value: 50.0,
            resistances,
            special_properties: ArmorProperties {
                condition: 1.0,
                kinetic_reduction: 0.4,
                explosive_reduction: 0.3,
                deflection_chance: 0.1,
                ..Default::default()
            },
        }
    }

    /// Create armor set for buildings
    pub fn structure() -> Self {
        let mut resistances = HashMap::new();
        resistances.insert(DamageType::SmallArms, 0.1);
        resistances.insert(DamageType::Combat, 0.3);
        resistances.insert(DamageType::Explosion, 1.0);
        resistances.insert(DamageType::Flame, 1.2);

        Self {
            primary_armor: ArmorType::Structural,
            armor_value: 100.0,
            resistances,
            special_properties: ArmorProperties {
                condition: 1.0,
                kinetic_reduction: 0.6,
                explosive_reduction: 0.2,
                ..Default::default()
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weapon::WeaponTemplate;

    #[test]
    fn test_damage_calculation() {
        let weapon = WeaponTemplate::new("TestWeapon".to_string());
        let bonus = WeaponBonus::new();
        let armor = ArmorSet::infantry();
        let source = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(10.0, 0.0, 0.0);
        let env_factors = EnvironmentalFactors {
            weather: WeatherCondition::Clear,
            terrain: TerrainType::Open,
            cover_level: 0.0,
            range: 10.0,
            elevation_difference: 0.0,
        };

        let result = DamageCalculator::calculate_damage(
            &weapon,
            &bonus,
            &armor,
            &target,
            &source,
            &env_factors,
            false,
        )
        .unwrap();

        assert!(result.final_damage >= 0.0);
        assert!(!result.critical_hit);
        assert!(!result.deflected);
    }

    #[test]
    fn test_armor_penetration() {
        let mut weapon = WeaponTemplate::new("TestWeapon".to_string());
        weapon.primary_damage = 200.0;
        let bonus = WeaponBonus::new();
        let light_armor = ArmorSet::light_vehicle();
        let heavy_armor = ArmorSet::heavy_tank();

        let light_result =
            DamageCalculator::calculate_penetration(&weapon, &bonus, &light_armor, 10.0);
        let heavy_result =
            DamageCalculator::calculate_penetration(&weapon, &bonus, &heavy_armor, 10.0);

        // Light armor should be more easily penetrated
        assert!(matches!(
            light_result,
            PenetrationResult::Full | PenetrationResult::Overpenetration
        ));
        // Heavy armor should provide better protection
        assert!(matches!(
            heavy_result,
            PenetrationResult::Bounced | PenetrationResult::Partial
        ));
    }

    #[test]
    fn test_environmental_modifiers() {
        let weapon = WeaponTemplate::new("TestWeapon".to_string());

        let clear_env = EnvironmentalFactors {
            weather: WeatherCondition::Clear,
            terrain: TerrainType::Open,
            cover_level: 0.0,
            range: 10.0,
            elevation_difference: 0.0,
        };

        let stormy_env = EnvironmentalFactors {
            weather: WeatherCondition::Storm,
            terrain: TerrainType::Forest,
            cover_level: 0.5,
            range: 10.0,
            elevation_difference: 0.0,
        };

        let clear_modifier =
            DamageCalculator::calculate_environmental_modifier(&weapon, &clear_env);
        let stormy_modifier =
            DamageCalculator::calculate_environmental_modifier(&weapon, &stormy_env);

        assert!(clear_modifier > stormy_modifier);
        assert!(clear_modifier <= 1.0);
        assert!(stormy_modifier >= 0.1);
    }
}
