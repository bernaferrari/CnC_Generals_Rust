//! Resolve armor for live objects and provide defaults for mapless fixtures.

use generals_sim::armor::{
    ARMORSET_CRATE_UPGRADE_ONE, ARMORSET_CRATE_UPGRADE_TWO, ARMORSET_ELITE, ARMORSET_HERO,
    ARMORSET_PLAYER_UPGRADE, ARMORSET_SECOND_LIFE, ARMORSET_VETERAN, ArmorTemplate, best_armor_set,
};
use generals_sim::damage::DamageType;

/// Retail HazardousMaterialArmor name for poison and radiation fields.
pub const HAZARDOUS_MATERIAL_ARMOR: &str = "HazardousMaterialArmor";

// DEFAULT 25%; FALLING 0%; LASER 100%; SMALL_ARMS 25%; MICROWAVE 0%;
// GATTLING 25%; HAZARD_CLEANUP 0%; KILL_PILOT 0%; SURRENDER 0%;
// SUBDUAL_MISSILE 100%; SUBDUAL_VEHICLE 0%; SUBDUAL_BUILDING 0%;
// POISON 0%; RADIATION 0%; FLAME 0%.

// DEFAULT 0%; HAZARD_CLEANUP 100%; FLAME 0%.

/// Armor coefficients for the ProjectileArmor fallback.
#[cfg(test)]
fn projectile_armor_fallback() -> ArmorTemplate {
    let mut t = ArmorTemplate::new();
    t.set_default(0.25);
    t.set_coefficient(DamageType::Falling, 0.0);
    t.set_coefficient(DamageType::Laser, 1.0);
    t.set_coefficient(DamageType::SmallArms, 0.25);
    t.set_coefficient(DamageType::Microwave, 0.0);
    t.set_coefficient(DamageType::Gattling, 0.25);
    t.set_coefficient(DamageType::HazardCleanup, 0.0);
    t.set_coefficient(DamageType::KillPilot, 0.0);
    t.set_coefficient(DamageType::Surrender, 0.0);
    t.set_coefficient(DamageType::SubdualMissile, 1.0);
    t.set_coefficient(DamageType::SubdualVehicle, 0.0);
    t.set_coefficient(DamageType::SubdualBuilding, 0.0);
    t.set_coefficient(DamageType::Poison, 0.0);
    t.set_coefficient(DamageType::Radiation, 0.0);
    t.set_coefficient(DamageType::Flame, 0.0);
    t
}

/// Armor coefficients for the HazardousMaterialArmor fallback.
#[cfg(test)]
pub(crate) fn hazardous_material_armor_fallback() -> ArmorTemplate {
    let mut t = ArmorTemplate::new();
    t.set_default(0.0);
    t.set_coefficient(DamageType::HazardCleanup, 1.0);
    t.set_coefficient(DamageType::Flame, 0.0);
    t
}

// HumanArmor: CRUSH 200%, ARMOR_PIERCING 10%, FLAME 150%, SNIPER 200%, LASER 50%,
//             HAZARD_CLEANUP 0%, KILL_PILOT 0%, SURRENDER 100%, SUBDUAL_* 0%.
// TankArmor: CRUSH 50%, SMALL_ARMS 25%, GATTLING 10%, FLAME 25%, RADIATION 50%,
//            POISON 25%, SNIPER 0%, MELEE 0%, LASER 0%, KILL_PILOT 100%,
//            SURRENDER 0%, SUBDUAL_VEHICLE 100%.
// StructureArmor: SMALL_ARMS 50%, GATTLING 10%, RADIATION 0%, SNIPER 0%, POISON 1%,
//                 LASER 0%, PARTICLE_BEAM 200%, AURORA_BOMB 250%, FLAME 50%,
//                 SUBDUAL_BUILDING 100%, LAND_MINE 0%.
// AirplaneArmor: SMALL_ARMS 120%, GATTLING 120%, JET_MISSILES 25%, POISON 25%,
//                RADIATION 25%, SNIPER 0%, LASER 0%, KILL_PILOT 0%.
// TruckArmor: CRUSH 50%, SMALL_ARMS 50%, GATTLING 50%, SNIPER 0%, MELEE 0%,
//             LASER 0%, KILL_PILOT 100%, SUBDUAL_VEHICLE 100%.

/// Armor coefficients for the HumanArmor fallback.
pub fn human_armor_fallback() -> ArmorTemplate {
    let mut t = ArmorTemplate::new();
    t.set_coefficient(DamageType::Crush, 2.0);
    t.set_coefficient(DamageType::ArmorPiercing, 0.10);
    t.set_coefficient(DamageType::InfantryMissile, 0.10);
    t.set_coefficient(DamageType::Flame, 1.50);
    t.set_coefficient(DamageType::ParticleBeam, 1.50);
    t.set_coefficient(DamageType::Sniper, 2.0);
    t.set_coefficient(DamageType::Laser, 0.50);
    t.set_coefficient(DamageType::HazardCleanup, 0.0);
    t.set_coefficient(DamageType::KillPilot, 0.0);
    t.set_coefficient(DamageType::Surrender, 1.0);
    t.set_coefficient(DamageType::SubdualMissile, 0.0);
    t.set_coefficient(DamageType::SubdualVehicle, 0.0);
    t.set_coefficient(DamageType::SubdualBuilding, 0.0);
    t
}

/// Armor coefficients for the TankArmor fallback.
pub fn tank_armor_fallback() -> ArmorTemplate {
    let mut t = ArmorTemplate::new();
    t.set_coefficient(DamageType::Crush, 0.50);
    t.set_coefficient(DamageType::SmallArms, 0.25);
    t.set_coefficient(DamageType::Gattling, 0.10);
    t.set_coefficient(DamageType::ComancheVulcan, 0.25);
    t.set_coefficient(DamageType::Flame, 0.25);
    t.set_coefficient(DamageType::Radiation, 0.50);
    t.set_coefficient(DamageType::Microwave, 0.0);
    t.set_coefficient(DamageType::Poison, 0.25);
    t.set_coefficient(DamageType::Sniper, 0.0);
    t.set_coefficient(DamageType::Melee, 0.0);
    t.set_coefficient(DamageType::Laser, 0.0);
    t.set_coefficient(DamageType::HazardCleanup, 0.0);
    t.set_coefficient(DamageType::ParticleBeam, 1.0);
    t.set_coefficient(DamageType::KillPilot, 1.0);
    t.set_coefficient(DamageType::Surrender, 0.0);
    t.set_coefficient(DamageType::SubdualMissile, 0.0);
    t.set_coefficient(DamageType::SubdualVehicle, 1.0);
    t.set_coefficient(DamageType::SubdualBuilding, 0.0);
    t
}

/// Armor coefficients for the StructureArmor fallback.
pub fn structure_armor_fallback() -> ArmorTemplate {
    let mut t = ArmorTemplate::new();
    t.set_coefficient(DamageType::Surrender, 0.0);
    t.set_coefficient(DamageType::SmallArms, 0.50);
    t.set_coefficient(DamageType::Gattling, 0.10);
    t.set_coefficient(DamageType::ComancheVulcan, 0.50);
    t.set_coefficient(DamageType::Radiation, 0.0);
    t.set_coefficient(DamageType::Microwave, 0.0);
    t.set_coefficient(DamageType::Sniper, 0.0);
    t.set_coefficient(DamageType::Poison, 0.01);
    t.set_coefficient(DamageType::Melee, 0.0);
    t.set_coefficient(DamageType::Laser, 0.0);
    t.set_coefficient(DamageType::HazardCleanup, 0.0);
    t.set_coefficient(DamageType::InfantryMissile, 0.50);
    t.set_coefficient(DamageType::ParticleBeam, 2.0);
    t.set_coefficient(DamageType::KillPilot, 0.0);
    t.set_coefficient(DamageType::AuroraBomb, 2.50);
    t.set_coefficient(DamageType::LandMine, 0.0);
    t.set_coefficient(DamageType::Flame, 0.50);
    t.set_coefficient(DamageType::SubdualMissile, 0.0);
    t.set_coefficient(DamageType::SubdualVehicle, 0.0);
    t.set_coefficient(DamageType::SubdualBuilding, 1.0);
    t
}

/// Armor coefficients for the AirplaneArmor fallback.
pub fn airplane_armor_fallback() -> ArmorTemplate {
    let mut t = ArmorTemplate::new();
    t.set_coefficient(DamageType::SmallArms, 1.20);
    t.set_coefficient(DamageType::Gattling, 1.20);
    t.set_coefficient(DamageType::Explosion, 1.0);
    t.set_coefficient(DamageType::InfantryMissile, 1.20);
    t.set_coefficient(DamageType::Laser, 0.0);
    t.set_coefficient(DamageType::HazardCleanup, 0.0);
    t.set_coefficient(DamageType::KillPilot, 0.0);
    t.set_coefficient(DamageType::Surrender, 0.0);
    t.set_coefficient(DamageType::JetMissiles, 0.25);
    t.set_coefficient(DamageType::Poison, 0.25);
    t.set_coefficient(DamageType::Radiation, 0.25);
    t.set_coefficient(DamageType::Microwave, 0.0);
    t.set_coefficient(DamageType::Sniper, 0.0);
    t.set_coefficient(DamageType::SubdualMissile, 0.0);
    t.set_coefficient(DamageType::SubdualVehicle, 0.0);
    t.set_coefficient(DamageType::SubdualBuilding, 0.0);
    t.set_coefficient(DamageType::Melee, 0.0);
    t
}

// HazMatHumanArmor: FLAME 25%, POISON 0%, RADIATION 0%, SNIPER 200%, LASER 25%.
// ChemSuitHumanArmor: FLAME 150%, POISON 20%, RADIATION 20%, SNIPER 200%.
// DozerArmor: SMALL_ARMS 25%, GATTLING 10%, FLAME 25%, SNIPER 0%.
// UpgradedTankArmor: SMALL_ARMS 20%, GATTLING 10%, FLAME 10%, POISON 10%, SNIPER 0%.
// HumveeArmor: SMALL_ARMS 50%, GATTLING 50%, JET_MISSILES 30%, FLAME 50%, SNIPER 0%.
// DragonTankArmor: SMALL_ARMS 25%, GATTLING 25%, FLAME 0%, POISON 25%, SNIPER 0%.
// ToxinTruckArmor: SMALL_ARMS 50%, GATTLING 50%, POISON 0%, SNIPER 0%.
// ComancheArmor: SMALL_ARMS 120%, GATTLING 120%, EXPLOSION 130%, POISON 25%, SNIPER 0%.
// StructureArmorTough: SMALL_ARMS 50%, GATTLING 10%, EXPLOSION 80%, SNIPER 0%, FLAME 50%.

/// Armor coefficients for the HazMatHumanArmor fallback.
#[cfg(test)]
fn hazmat_human_armor_fallback() -> ArmorTemplate {
    let mut t = ArmorTemplate::new();
    t.set_coefficient(DamageType::Crush, 2.0);
    t.set_coefficient(DamageType::ArmorPiercing, 0.10);
    t.set_coefficient(DamageType::Sniper, 2.0);
    t.set_coefficient(DamageType::Flame, 0.25);
    t.set_coefficient(DamageType::Laser, 0.25);
    t.set_coefficient(DamageType::Poison, 0.0);
    t.set_coefficient(DamageType::Radiation, 0.0);
    t.set_coefficient(DamageType::Microwave, 0.0);
    t.set_coefficient(DamageType::ParticleBeam, 1.50);
    t.set_coefficient(DamageType::HazardCleanup, 0.0);
    t.set_coefficient(DamageType::KillPilot, 0.0);
    t.set_coefficient(DamageType::Surrender, 1.0);
    t.set_coefficient(DamageType::SubdualMissile, 0.0);
    t.set_coefficient(DamageType::SubdualVehicle, 0.0);
    t.set_coefficient(DamageType::SubdualBuilding, 0.0);
    t
}

/// Armor coefficients for the ChemSuitHumanArmor fallback.
pub fn chem_suit_human_armor_fallback() -> ArmorTemplate {
    let mut t = ArmorTemplate::new();
    t.set_coefficient(DamageType::Crush, 2.0);
    t.set_coefficient(DamageType::ArmorPiercing, 0.10);
    t.set_coefficient(DamageType::InfantryMissile, 0.10);
    t.set_coefficient(DamageType::Flame, 1.50);
    t.set_coefficient(DamageType::Poison, 0.20);
    t.set_coefficient(DamageType::Radiation, 0.20);
    t.set_coefficient(DamageType::Microwave, 0.20);
    t.set_coefficient(DamageType::ParticleBeam, 1.50);
    t.set_coefficient(DamageType::Sniper, 2.0);
    t.set_coefficient(DamageType::Laser, 0.50);
    t.set_coefficient(DamageType::HazardCleanup, 0.0);
    t.set_coefficient(DamageType::KillPilot, 0.0);
    t.set_coefficient(DamageType::Surrender, 1.0);
    t.set_coefficient(DamageType::SubdualMissile, 0.0);
    t.set_coefficient(DamageType::SubdualVehicle, 0.0);
    t.set_coefficient(DamageType::SubdualBuilding, 0.0);
    t
}

/// Armor coefficients for the UpgradedTankArmor fallback.
pub fn upgraded_tank_armor_fallback() -> ArmorTemplate {
    let mut t = ArmorTemplate::new();
    t.set_coefficient(DamageType::SmallArms, 0.20);
    t.set_coefficient(DamageType::Gattling, 0.10);
    t.set_coefficient(DamageType::Flame, 0.10);
    t.set_coefficient(DamageType::Poison, 0.10);
    t.set_coefficient(DamageType::Sniper, 0.0);
    t.set_coefficient(DamageType::Laser, 0.0);
    t.set_coefficient(DamageType::KillPilot, 1.0);
    t.set_coefficient(DamageType::SubdualVehicle, 1.0);
    t
}

/// Armor coefficients for the DragonTankArmor fallback.
#[cfg(test)]
fn dragon_tank_armor_fallback() -> ArmorTemplate {
    let mut t = ArmorTemplate::new();
    t.set_coefficient(DamageType::SmallArms, 0.25);
    t.set_coefficient(DamageType::Gattling, 0.25);
    t.set_coefficient(DamageType::Flame, 0.0);
    t.set_coefficient(DamageType::Poison, 0.25);
    t.set_coefficient(DamageType::Sniper, 0.0);
    t.set_coefficient(DamageType::KillPilot, 1.0);
    t.set_coefficient(DamageType::SubdualVehicle, 1.0);
    t
}

/// Armor coefficients for the StructureArmorTough fallback.
#[cfg(test)]
fn structure_armor_tough_fallback() -> ArmorTemplate {
    let mut t = ArmorTemplate::new();
    t.set_coefficient(DamageType::SmallArms, 0.50);
    t.set_coefficient(DamageType::Gattling, 0.10);
    t.set_coefficient(DamageType::Sniper, 0.0);
    t.set_coefficient(DamageType::Poison, 0.01);
    t.set_coefficient(DamageType::Flame, 0.50);
    t.set_coefficient(DamageType::Explosion, 0.80);
    t.set_coefficient(DamageType::SubdualBuilding, 1.0);
    t
}

/// Map host combat damage class → Armor.ini DamageType residual.
/// Identity through crate/C++ ordinals (`DamageType::to_store`); no collapse.
pub fn map_host_damage_type(dt: crate::game_logic::combat::DamageType) -> DamageType {
    dt.to_store()
}

/// C++ `ActiveBody::m_curArmorSetFlags` from live host body flags.
pub fn live_armor_set_flags(obj: &crate::game_logic::Object) -> u8 {
    use crate::game_logic::VeterancyLevel;
    let mut flags = 0u8;
    if obj.armor_set_veteran {
        flags |= ARMORSET_VETERAN;
    } else if obj.armor_set_elite {
        flags |= ARMORSET_ELITE;
    } else if obj.armor_set_hero {
        flags |= ARMORSET_HERO;
    } else {
        match obj.experience.level {
            VeterancyLevel::Veteran => flags |= ARMORSET_VETERAN,
            VeterancyLevel::Elite => flags |= ARMORSET_ELITE,
            VeterancyLevel::Heroic => flags |= ARMORSET_HERO,
            VeterancyLevel::Rookie => {}
        }
    }
    if obj.armor_set_player_upgrade {
        flags |= ARMORSET_PLAYER_UPGRADE;
    }
    if obj.armor_set_second_life {
        flags |= ARMORSET_SECOND_LIFE;
    }
    match obj.armor_crate_upgrade {
        1 => flags |= ARMORSET_CRATE_UPGRADE_ONE,
        n if n >= 2 => flags |= ARMORSET_CRATE_UPGRADE_TWO,
        _ => {}
    }
    flags
}

fn receiving_armor_for_object(obj: &crate::game_logic::Object) -> Option<&ArmorTemplate> {
    let name = best_armor_set(&obj.thing().template.armor_sets, live_armor_set_flags(obj))?
        .armor
        .as_deref()?;
    if name.trim().is_empty() || name.eq_ignore_ascii_case("none") {
        return None;
    }
    obj.thing()
        .template
        .armor_definitions
        .as_ref()?
        .find_template(name.trim())
}

/// Programmatic fixtures can use default armor by unit class. An authored None or miss uses neutral armor.
fn mapless_armor(obj: &crate::game_logic::Object) -> ArmorTemplate {
    use crate::game_logic::KindOf;
    if obj.thing().template.armor_definitions.is_some()
        || !obj.thing().template.armor_sets.is_empty()
    {
        return ArmorTemplate::new();
    }
    if obj.is_kind_of(KindOf::Aircraft) {
        return airplane_armor_fallback();
    }
    if obj.is_kind_of(KindOf::Structure) || obj.is_kind_of(KindOf::Immobile) {
        return structure_armor_fallback();
    }
    if obj.is_kind_of(KindOf::Infantry) {
        // C++ ChemSuitHumanArmor when Upgrade_AmericaChemicalSuits residual is active.
        if obj.has_upgrade_tag("Upgrade_AmericaChemicalSuits")
            || obj.has_upgrade_tag("UpgradeChemicalSuits")
            || obj
                .applied_upgrades
                .iter()
                .any(|u| u.to_ascii_lowercase().contains("chemicalsuit"))
        {
            return chem_suit_human_armor_fallback();
        }
        return human_armor_fallback();
    }
    if obj.is_kind_of(KindOf::Vehicle) {
        // Salvage crate with no authored ArmorSet still switches to crate armor.
        if obj.armor_crate_upgrade > 0 {
            return upgraded_tank_armor_fallback();
        }
        return tank_armor_fallback();
    }
    ArmorTemplate::new()
}

/// Compatibility inspection helper. Production damage borrows the selected row.
pub fn armor_for_object(obj: &crate::game_logic::Object) -> ArmorTemplate {
    receiving_armor_for_object(obj)
        .cloned()
        .unwrap_or_else(|| mapless_armor(obj))
}

/// Adjust incoming damage using the object's selected armor.
/// C++ `ArmorTemplate::adjustDamage` only (Armor.cpp:43-50).
pub fn adjust_damage(
    obj: &crate::game_logic::Object,
    host_damage_type: crate::game_logic::combat::DamageType,
    amount: f32,
) -> f32 {
    let dt = map_host_damage_type(host_damage_type);
    if let Some(armor) = receiving_armor_for_object(obj) {
        return armor.adjust_damage(dt, amount).max(0.0);
    }
    mapless_armor(obj).adjust_damage(dt, amount).max(0.0)
}

/// Map gamelogic / Weapon.ini DamageType → host combat damage class residual.
/// Keeps all 38 C++ types (Damage.h:26-70); no SUBDUAL_*/Toppling/HazardCleanup
/// → Unresistable and no Gattling/Sniper/AP/Melee/missile collapse to Bullet.
pub fn map_store_damage_type(
    dt: gamelogic::damage::DamageType,
) -> crate::game_logic::combat::DamageType {
    crate::game_logic::combat::DamageType::from_store(dt)
}

/// Map leftover Weapon.ini DamageType name (`EXPLOSION`, `POISON`, …) to host combat type.
/// Unknown names fail closed to C++ ordinal 0 (`DAMAGE_EXPLOSION`), not UNRESISTABLE.
pub fn damage_type_from_legacy_name(name: &str) -> crate::game_logic::combat::DamageType {
    use crate::game_logic::host_enum_table_residual::damage_type_bit_name_index;
    if let Some(idx) = damage_type_bit_name_index(name) {
        return crate::game_logic::combat::DamageType::from_store(
            gamelogic::damage::DamageType::from_u32(idx as u32),
        );
    }
    crate::game_logic::combat::DamageType::Explosive
}

/// Map leftover Weapon.ini DeathType name (`EXPLODED`, `POISONED`, …) to host death type.
pub fn death_type_from_legacy_name(name: &str) -> crate::game_logic::host_usa_pilot::HostDeathType {
    use crate::game_logic::host_enum_table_residual::death_type_name_index;
    if let Some(idx) = death_type_name_index(name) {
        return crate::game_logic::host_usa_pilot::HostDeathType::from_store(
            gamelogic::damage::DeathType::from_u32(idx as u32),
        );
    }
    crate::game_logic::host_usa_pilot::HostDeathType::Normal
}

/// Resolve the authored WeaponTemplate damage type. An absent definition cannot
/// fire or estimate damage; its name is not a damage rule (Weapon.cpp:559).
pub fn host_damage_type_for_weapon_name(
    name: &str,
) -> Option<crate::game_logic::combat::DamageType> {
    crate::game_logic::weapon_bootstrap::with_host_weapon_store(|store| {
        store
            .find_weapon_template(name)
            .map(|weapon| map_store_damage_type(weapon.damage_type))
    })
    .ok()
    .flatten()
}

/// Look up Weapon.ini DeathType residual by weapon template name.
pub fn host_death_type_for_weapon_name(
    name: &str,
) -> crate::game_logic::host_usa_pilot::HostDeathType {
    use crate::game_logic::host_usa_pilot::HostDeathType;
    use gamelogic::weapon::with_weapon_store;
    let dt = with_weapon_store(|store| {
        store.find_weapon_template(name).map(|wt| {
            let d = HostDeathType::from_store(wt.death_type);
            // When store leaves Normal, keep Normal (caller may refine by DamageType).
            d
        })
    })
    .ok()
    .flatten();
    dt.unwrap_or(HostDeathType::Normal)
}

/// Resolve kill DeathType: store DeathType, else DamageType residual mapping.
pub fn resolve_host_death_type(
    weapon_name: Option<&str>,
    damage_type: crate::game_logic::combat::DamageType,
) -> crate::game_logic::host_usa_pilot::HostDeathType {
    use crate::game_logic::host_usa_pilot::HostDeathType;
    if let Some(n) = weapon_name {
        let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
        if crate::game_logic::thing::ThingTemplate::weapon_from_store(n).is_some() {
            let d = host_death_type_for_weapon_name(n);
            if d != HostDeathType::Normal {
                return d;
            }
        }
    }
    HostDeathType::from_host_damage_type(damage_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projectile_and_hazard_fallback_damage() {
        let p = projectile_armor_fallback();
        assert!((p.adjust_damage(DamageType::Laser, 40.0) - 40.0).abs() < 0.01);
        let h = hazardous_material_armor_fallback();
        assert!((h.adjust_damage(DamageType::HazardCleanup, 40.0) - 40.0).abs() < 0.01);
        assert!((h.adjust_damage(DamageType::Explosion, 40.0)).abs() < 0.01);
    }

    #[test]
    fn unit_fallback_damage() {
        let tank = tank_armor_fallback();
        assert!((tank.adjust_damage(DamageType::SmallArms, 40.0) - 10.0).abs() < 0.01);
        let human = human_armor_fallback();
        assert!((human.adjust_damage(DamageType::Sniper, 50.0) - 100.0).abs() < 0.01);
        let structure = structure_armor_fallback();
        assert!((structure.adjust_damage(DamageType::AuroraBomb, 100.0) - 250.0).abs() < 0.01);
    }

    #[test]
    fn specialized_fallback_damage() {
        let dragon = dragon_tank_armor_fallback();
        assert!((dragon.adjust_damage(DamageType::Flame, 100.0)).abs() < 0.01);
        let hazmat = hazmat_human_armor_fallback();
        assert!((hazmat.adjust_damage(DamageType::Poison, 100.0)).abs() < 0.01);
        let tough = structure_armor_tough_fallback();
        assert!((tough.adjust_damage(DamageType::Explosion, 100.0) - 80.0).abs() < 0.01);
    }

    #[test]
    fn store_damage_type_maps_explosion_and_laser() {
        use gamelogic::damage::DamageType as G;
        assert_eq!(
            map_store_damage_type(G::Explosion),
            crate::game_logic::combat::DamageType::Explosive
        );
        assert_eq!(
            map_store_damage_type(G::Laser),
            crate::game_logic::combat::DamageType::Laser
        );
        assert_eq!(
            map_store_damage_type(G::SmallArms),
            crate::game_logic::combat::DamageType::Bullet
        );
    }

    #[test]
    fn store_damage_type_keeps_cpp_38_without_collapse() {
        // C++ Damage.h:26-70 / ArmorTemplate::adjustDamage — live map_store_damage_type
        // must not collapse SUBDUAL_* / Toppling / HazardCleanup to Unresistable,
        // or Gattling / Sniper / AP / Melee / missiles to Bullet.
        use crate::game_logic::combat::DamageType as H;
        use gamelogic::damage::DamageType as G;
        assert_eq!(map_store_damage_type(G::SubdualMissile), H::SubdualMissile);
        assert_eq!(map_store_damage_type(G::SubdualVehicle), H::SubdualVehicle);
        assert_eq!(
            map_store_damage_type(G::SubdualBuilding),
            H::SubdualBuilding
        );
        assert_eq!(
            map_store_damage_type(G::SubdualUnresistable),
            H::SubdualUnresistable
        );
        assert_eq!(map_store_damage_type(G::Toppling), H::Toppling);
        assert_eq!(map_store_damage_type(G::HazardCleanup), H::HazardCleanup);
        assert_eq!(map_store_damage_type(G::Gattling), H::Gattling);
        assert_eq!(map_store_damage_type(G::Sniper), H::Sniper);
        assert_eq!(map_store_damage_type(G::ArmorPiercing), H::ArmorPiercing);
        assert_eq!(map_store_damage_type(G::Melee), H::Melee);
        assert_eq!(
            map_store_damage_type(G::InfantryMissile),
            H::InfantryMissile
        );
        assert_eq!(map_store_damage_type(G::JetMissiles), H::JetMissiles);
        assert_eq!(
            map_store_damage_type(G::StealthJetMissiles),
            H::StealthJetMissiles
        );
        assert_eq!(map_store_damage_type(G::ComancheVulcan), H::ComancheVulcan);
        assert_eq!(map_store_damage_type(G::Microwave), H::Microwave);
        assert_eq!(map_host_damage_type(H::Microwave), DamageType::Microwave);
        assert_eq!(map_host_damage_type(H::EMP), DamageType::Microwave);
        assert_ne!(map_store_damage_type(G::SubdualMissile), H::Unresistable);
        assert_ne!(map_store_damage_type(G::Gattling), H::Bullet);
        assert_eq!(
            map_host_damage_type(H::SubdualMissile),
            DamageType::SubdualMissile
        );
        assert_eq!(map_host_damage_type(H::Gattling), DamageType::Gattling);
    }

    #[test]
    fn gattling_weapon_name_stays_gattling() {
        let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
        assert_eq!(
            host_damage_type_for_weapon_name("GattlingTankGun"),
            Some(crate::game_logic::combat::DamageType::Gattling)
        );
    }

    #[test]
    fn host_weapon_name_damage_type_uses_seeded_store() {
        let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
        let laser = host_damage_type_for_weapon_name("PaladinPointDefenseLaser");
        assert_eq!(laser, Some(crate::game_logic::combat::DamageType::Laser));
        let rifle = host_damage_type_for_weapon_name("RangerAdvancedCombatRifle");
        assert_eq!(rifle, Some(crate::game_logic::combat::DamageType::Bullet));
        let bomb = host_damage_type_for_weapon_name("AuroraBombWeapon").expect("authored weapon");
        // Retail Weapon.ini AuroraBombWeapon declares AURORA_BOMB, and
        // C++ Damage.h keeps that ordinal25 separate from EXPLOSION ordinal0.
        assert_eq!(bomb, crate::game_logic::combat::DamageType::AuroraBomb);
        assert_ne!(bomb, crate::game_logic::combat::DamageType::Explosive);
        assert_eq!(map_host_damage_type(bomb), DamageType::AuroraBomb);
        assert_eq!(map_host_damage_type(bomb) as u32, 25);
    }

    #[test]
    fn seeded_weapon_death_type_residual() {
        use crate::game_logic::host_usa_pilot::HostDeathType;
        let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
        assert_eq!(
            host_death_type_for_weapon_name("PaladinPointDefenseLaser"),
            HostDeathType::Lasered
        );
        assert_eq!(
            host_death_type_for_weapon_name("AuroraBombWeapon"),
            HostDeathType::Exploded
        );
        assert_eq!(
            resolve_host_death_type(
                Some("PaladinPointDefenseLaser"),
                crate::game_logic::combat::DamageType::Bullet
            ),
            HostDeathType::Lasered
        );
    }

    #[test]
    fn receiving_armor_same_name_isolated_and_live_flags_select_without_global_store() {
        use crate::game_logic::{HostArmorSet, KindOf, Object, ObjectId, Team, ThingTemplate};
        fn object(coefficient: f32) -> Object {
            let mut catalog = generals_sim::armor::ArmorDefinitions::new();
            for (name, coefficient) in [("SharedArmor", coefficient), ("VeteranArmor", 0.75)] {
                let mut armor = ArmorTemplate::new();
                armor.set_default(coefficient);
                catalog.register_template(name, armor);
            }
            let mut template = ThingTemplate::new("ReceivingArmor");
            template.add_kind_of(KindOf::Vehicle);
            template.armor_definitions = Some(std::sync::Arc::new(catalog));
            template.armor_sets = vec![
                HostArmorSet {
                    conditions: 0,
                    armor: Some("SharedArmor".into()),
                    damage_fx: None,
                },
                HostArmorSet {
                    conditions: ARMORSET_SECOND_LIFE,
                    armor: Some("VeteranArmor".into()),
                    damage_fx: None,
                },
            ];
            Object::new(template, ObjectId(1), Team::USA)
        }
        let mut a = object(0.25);
        let b = object(0.5);
        let damage = crate::game_logic::combat::DamageType::Explosive;
        assert_eq!(adjust_damage(&a, damage, 100.0), 25.0);
        assert_eq!(adjust_damage(&b, damage, 100.0), 50.0);
        a.armor_set_second_life = true;
        assert_eq!(adjust_damage(&a, damage, 100.0), 75.0);
        a.armor_set_second_life = false;
        a.template_mut().armor_sets[0].armor = Some("MissingArmor".into());
        assert_eq!(adjust_damage(&a, damage, 100.0), 100.0);
        a.template_mut().armor_sets[0].armor = None;
        assert_eq!(adjust_damage(&a, damage, 100.0), 100.0);
        assert_eq!(adjust_damage(&b, damage, 100.0), 50.0);
    }
}
