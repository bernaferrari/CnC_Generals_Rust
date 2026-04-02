//! Damage Calculator — C++ Armor.cpp Parity
//!
//! PARITY_NOTE: The following fabricated mechanics have been removed because they
//! have NO equivalent in the C++ GeneralsMD codebase:
//!   - PenetrationResult enum (bounced/partial/full/overpenetration)
//!   - Critical hit system (get_critical_multiplier, is_critical_hit flag)
//!   - Environmental modifiers (WeatherCondition, TerrainType, EnvironmentalFactors)
//!   - Armor deflection (deflection_chance, check_deflection)
//!   - Armor penetration/penetration modifier calculations
//!   - Status effects from damage (StatusEffect enum)
//!   - Range-based damage falloff (calculate_range_modifier)
//!   - Line-of-sight obstruction for area damage
//!   - Armor degradation / condition / self-repair
//!   - Armor type categories (Reactive, Composite, EnergyShield)
//!
//! The C++ armor system is purely a coefficient multiplier table:
//!   final_damage = base_damage * armor_condition_coefficient
//! See Armor.cpp / ArmorTemplate in the C++ source.

use super::{Coord3D, DamageType, ObjectId, WeaponBonus, WeaponTemplate};
use crate::{GameLogicError, GameLogicResult};

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Stub types kept for backward-compatible public API
// ---------------------------------------------------------------------------

/// PARITY_NOTE: Fabricated enum — C++ has no armor type categories.
/// Kept as a unit-only stub to avoid breaking callers that reference the name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArmorType {
    None,
}

/// PARITY_NOTE: Most fields were fabricated. Only `resistances` (the coefficient
/// table) maps to C++ ArmorTemplate::m_damageMultiplier. All other fields
/// (deflection, degradation, condition, self-repair, kinetic/explosive/energy
/// reductions) are removed.
#[derive(Debug, Clone)]
pub struct ArmorSet {
    /// PARITY_NOTE: This is the only field with C++ equivalence.
    /// Maps DamageType -> coefficient (0.0 = immune, 1.0 = full damage).
    pub resistances: HashMap<DamageType, f32>,
}

/// PARITY_NOTE: Entirely fabricated — all fields removed.
/// Kept as empty struct to avoid breaking callers.
#[derive(Debug, Clone, Default)]
pub struct ArmorProperties {}

/// PARITY_NOTE: Fabricated — removed PenetrationResult, critical_hit,
/// critical_multiplier, deflected, penetration_result, environmental_modifiers,
/// and status_effects. Only the coefficient-based fields remain.
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
// Stub types for removed fabricated enums (kept so mod.rs glob re-exports
// don't break downstream)
// ---------------------------------------------------------------------------

/// PARITY_NOTE: Fabricated — removed. Stub kept for API compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PenetrationResult {
    Full,
}

/// PARITY_NOTE: Fabricated — removed. Stub kept for API compatibility.
#[derive(Debug, Clone)]
pub enum StatusEffect {}

/// PARITY_NOTE: Fabricated — removed. Stub kept for API compatibility.
#[derive(Debug, Clone)]
pub struct EnvironmentalFactors {}

/// PARITY_NOTE: Fabricated — removed. Stub kept for API compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeatherCondition {
    Clear,
}

/// PARITY_NOTE: Fabricated — removed. Stub kept for API compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainType {
    Open,
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
    ///
    /// PARITY_NOTE: Simplified. C++ Weapon.cpp uses WeaponTemplate's
    /// primary/secondary damage radii with linear falloff between them.
    /// Non-linear falloff (powf(1.5)) and LOS obstruction are removed.
    pub fn calculate_area_damage(
        weapon_template: &WeaponTemplate,
        weapon_bonus: &WeaponBonus,
        explosion_center: &Coord3D,
        targets: &[(ObjectId, Coord3D, ArmorSet)],
    ) -> GameLogicResult<HashMap<ObjectId, DamageResult>> {
        let mut results = HashMap::new();
        let blast_radius = weapon_template.get_primary_damage_radius(weapon_bonus);

        for (object_id, target_pos, armor) in targets {
            let distance = explosion_center.distance(*target_pos);

            if distance <= blast_radius {
                // C++ linear falloff: full damage at center, zero at edge
                let falloff_factor = if blast_radius > 0.0 {
                    1.0 - (distance / blast_radius)
                } else {
                    1.0
                };

                let mut modified_weapon = weapon_template.clone();
                modified_weapon.primary_damage *= falloff_factor;

                let damage_result = Self::calculate_damage(
                    &modified_weapon,
                    weapon_bonus,
                    armor,
                    target_pos,
                    explosion_center,
                )?;

                results.insert(*object_id, damage_result);
            }
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
