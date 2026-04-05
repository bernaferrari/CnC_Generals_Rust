//! Damage Calculator — C++ Armor.cpp Parity
//!
//! PARITY_NOTE: The following fabricated mechanics have been removed because they
//! have NO equivalent in the C++ GeneralsMD codebase:
//!   - Critical hit system (get_critical_multiplier, is_critical_hit flag)
//!   - Armor deflection (deflection_chance, check_deflection)
//!   - Armor penetration/penetration modifier calculations
//!   - Range-based damage falloff (calculate_range_modifier)
//!   - Line-of-sight obstruction for area damage
//!   - Armor degradation / condition / self-repair
//!
//! The C++ armor system is purely a coefficient multiplier table:
//!   final_damage = base_damage * armor_condition_coefficient
//! See Armor.cpp / ArmorTemplate in the C++ source.

use super::{Coord3D, DamageType, ObjectId, WeaponBonus, WeaponTemplate};
use crate::{GameLogicError, GameLogicResult};

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ArmorSet {
    /// PARITY_NOTE: This is the only field with C++ equivalence.
    /// Maps DamageType -> coefficient (0.0 = immune, 1.0 = full damage).
    pub resistances: HashMap<DamageType, f32>,
}

#[derive(Debug, Clone)]
pub struct DamageResult {
    /// Final damage after armor coefficient applied
    pub final_damage: f32,
    /// Damage before armor
    pub raw_damage: f32,
    /// Damage absorbed by armor (raw - final)
    pub armor_absorption: f32,
}

// ---------------------------------------------------------------------------
// DamageCalculator — simplified to coefficient-only multiplication
// ---------------------------------------------------------------------------

/// Damage calculator implementing C++ Armor.cpp coefficient lookup.
///
/// In C++, armor is a simple lookup table: each ArmorTemplate maps every
/// DamageType to a float coefficient.  The damage calculation is:
///   `final_damage = base_damage * armor_coefficient`
pub struct DamageCalculator;

impl DamageCalculator {
    /// Calculate damage using the armor coefficient table.
    ///
    /// PARITY_NOTE: Signature simplified. The C++ equivalent is:
    ///   `float ArmorTemplate::getConditionCoeff(DamageType) const`
    /// which returns a single float multiplier. No penetration, no crits,
    /// no weather, no range falloff.
    pub fn calculate_damage(
        weapon_template: &WeaponTemplate,
        weapon_bonus: &WeaponBonus,
        target_armor: &ArmorSet,
        _impact_location: &Coord3D,
        _source_location: &Coord3D,
    ) -> GameLogicResult<DamageResult> {
        let base_damage = weapon_template.get_primary_damage(weapon_bonus);

        // Look up armor coefficient for this damage type (C++ Armor.cpp)
        let damage_type = weapon_template.damage_type;
        let armor_coefficient = target_armor
            .resistances
            .get(&damage_type)
            .copied()
            .unwrap_or(1.0); // Default: full damage if no entry

        let final_damage = base_damage * armor_coefficient;

        Ok(DamageResult {
            final_damage: final_damage.max(0.0),
            raw_damage: base_damage,
            armor_absorption: base_damage - final_damage,
        })
    }

    /// Calculate area damage with distance-based falloff.
    pub fn calculate_area_damage(
        weapon_template: &WeaponTemplate,
        weapon_bonus: &WeaponBonus,
        explosion_center: &Coord3D,
        targets: &[(ObjectId, Coord3D, ArmorSet)],
    ) -> GameLogicResult<HashMap<ObjectId, DamageResult>> {
        let mut results = HashMap::new();
        let primary_radius = weapon_template.get_primary_damage_radius(weapon_bonus);
        let secondary_radius = weapon_template.get_secondary_damage_radius(weapon_bonus);
        let primary_damage = weapon_template.get_primary_damage(weapon_bonus);
        let secondary_damage = weapon_template.get_secondary_damage(weapon_bonus);

        let valid_secondary_radius = if secondary_radius > primary_radius {
            secondary_radius
        } else {
            0.0
        };
        let blast_radius = primary_radius.max(valid_secondary_radius);

        for (object_id, target_pos, armor) in targets {
            let distance = explosion_center.distance(*target_pos);

            if distance > blast_radius {
                continue;
            }

            let zone_damage = if valid_secondary_radius > 0.0 {
                if distance <= primary_radius {
                    primary_damage
                } else {
                    let span = valid_secondary_radius - primary_radius;
                    let t = if span > 0.0 {
                        (distance - primary_radius) / span
                    } else {
                        1.0
                    };
                    secondary_damage * (1.0 - t).clamp(0.0, 1.0)
                }
            } else if primary_radius > 0.0 {
                primary_damage * (1.0 - (distance / primary_radius)).clamp(0.0, 1.0)
            } else {
                primary_damage
            };

            if zone_damage <= 0.0 {
                continue;
            }

            let mut modified_weapon = weapon_template.clone();
            modified_weapon.primary_damage = zone_damage;

            let damage_result = Self::calculate_damage(
                &modified_weapon,
                &WeaponBonus::new(),
                armor,
                target_pos,
                explosion_center,
            )?;

            results.insert(*object_id, damage_result);
        }

        Ok(results)
    }
}

// ---------------------------------------------------------------------------
// Default armor configurations
// ---------------------------------------------------------------------------

impl ArmorSet {
    /// Create an empty armor set (all damage types at 100%)
    pub fn new() -> Self {
        Self {
            resistances: HashMap::new(),
        }
    }

    /// Create armor set for infantry
    pub fn infantry() -> Self {
        let mut resistances = HashMap::new();
        resistances.insert(DamageType::SmallArms, 0.8);
        resistances.insert(DamageType::Explosion, 1.2);
        resistances.insert(DamageType::Flame, 1.3);
        Self { resistances }
    }

    /// Create armor set for light vehicles
    pub fn light_vehicle() -> Self {
        let mut resistances = HashMap::new();
        resistances.insert(DamageType::SmallArms, 0.3);
        resistances.insert(DamageType::Explosion, 1.0);
        Self { resistances }
    }

    /// Create armor set for heavy tanks
    pub fn heavy_tank() -> Self {
        let mut resistances = HashMap::new();
        resistances.insert(DamageType::SmallArms, 0.1);
        resistances.insert(DamageType::Explosion, 0.8);
        Self { resistances }
    }

    /// Create armor set for buildings/structures
    pub fn structure() -> Self {
        let mut resistances = HashMap::new();
        resistances.insert(DamageType::SmallArms, 0.1);
        resistances.insert(DamageType::Explosion, 1.0);
        Self { resistances }
    }
}

impl Default for ArmorSet {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weapon::WeaponTemplate;

    #[test]
    fn test_damage_calculation_coefficient_only() {
        let weapon = WeaponTemplate::new("TestWeapon".to_string());
        let bonus = WeaponBonus::new();
        let armor = ArmorSet::infantry();
        let source = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(10.0, 0.0, 0.0);

        let result =
            DamageCalculator::calculate_damage(&weapon, &bonus, &armor, &target, &source).unwrap();

        assert!(result.final_damage >= 0.0);
        assert_eq!(
            result.raw_damage,
            result.final_damage + result.armor_absorption
        );
    }

    #[test]
    fn test_armor_coefficient_lookup() {
        let weapon = WeaponTemplate::new("TestWeapon".to_string());
        let bonus = WeaponBonus::new();

        // Armor with 50% resistance to explosions
        let mut armor = ArmorSet::new();
        armor.resistances.insert(DamageType::Explosion, 0.5);

        let result = DamageCalculator::calculate_damage(
            &weapon,
            &bonus,
            &armor,
            &Coord3D::ZERO,
            &Coord3D::ZERO,
        )
        .unwrap();

        assert_eq!(result.final_damage, result.raw_damage * 0.5);
    }
}
