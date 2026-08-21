use super::*;

/// C++ ProjectileCollideMask residual bits (Weapon.cpp TheWeaponCollideMaskNames).
pub const PROJECTILE_COLLIDE_ALLIES: u32 = 0x0001;
pub const PROJECTILE_COLLIDE_ENEMIES: u32 = 0x0002;
pub const PROJECTILE_COLLIDE_STRUCTURES: u32 = 0x0004;
pub const PROJECTILE_COLLIDE_SHRUBBERY: u32 = 0x0008;
pub const PROJECTILE_COLLIDE_PROJECTILES: u32 = 0x0010;
pub const PROJECTILE_COLLIDE_WALLS: u32 = 0x0020;
pub const PROJECTILE_COLLIDE_SMALL_MISSILES: u32 = 0x0040;
pub const PROJECTILE_COLLIDE_BALLISTIC_MISSILES: u32 = 0x0080;

/// Default ballistic residual: structures + walls (most tank/missile shells).
pub const PROJECTILE_COLLIDE_DEFAULT: u32 =
    PROJECTILE_COLLIDE_STRUCTURES | PROJECTILE_COLLIDE_WALLS;

/// C++ Weapon.ini ProjectileCollidesWith residual mask (Regular).
///
/// Fail-closed: gates intervening-structure intercept only — not full shrubbery /
/// projectile-vs-projectile collide matrix.
pub fn host_projectile_collides_for_weapon_name(name: &str) -> u32 {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store.find_weapon_template(name).map(|wt| {
            // collide_mask bits are public u32 storage on WeaponCollideMask.
            wt.collide_mask.bits()
        })
    })
    .ok()
    .flatten();
    if let Some(bits) = from_store {
        if bits != 0 {
            return bits;
        }
    }
    seed_projectile_collides_for(name)
}

pub(super) fn seed_projectile_collides_for(name: &str) -> u32 {
    let n = name.to_ascii_lowercase();
    // Hitscan / laser residual: no projectile collide (instant).
    if n.contains("laser")
        || n.contains("machinegun")
        || n.contains("minigun")
        || n.contains("gattling")
        || n.contains("flashbang")
        || n.contains("combatrifle")
    {
        return 0;
    }
    // AA missiles residual: often STRUCTURES only (no walls peel).
    if n.contains("stinger") || n.contains("patriot") && n.contains("air") {
        return PROJECTILE_COLLIDE_STRUCTURES;
    }
    // Scud / aurora / howitzer residual: STRUCTURES (retail).
    if n.contains("scud") || n.contains("aurora") || n.contains("howitzer") {
        return PROJECTILE_COLLIDE_STRUCTURES;
    }
    // Avenger / some air weapons include ENEMIES.
    if n.contains("avenger") {
        return PROJECTILE_COLLIDE_ENEMIES
            | PROJECTILE_COLLIDE_STRUCTURES
            | PROJECTILE_COLLIDE_WALLS
            | PROJECTILE_COLLIDE_SHRUBBERY;
    }
    PROJECTILE_COLLIDE_DEFAULT
}

/// Whether intervening structure intercept is enabled for this collide mask.
pub fn projectile_collides_structures(mask: u32) -> bool {
    (mask & (PROJECTILE_COLLIDE_STRUCTURES | PROJECTILE_COLLIDE_WALLS)) != 0
}

/// C++ Weapon.cpp:271 default `m_affectsMask` when RadiusDamageAffects is omitted.
pub const WEAPON_AFFECTS_DEFAULT: u32 = crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ALLIES
    | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ENEMIES
    | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_NEUTRALS;

/// C++ Weapon.ini RadiusDamageAffects residual mask (Regular).
///
/// Live fire reads the leftover WeaponStore (Weapon.cpp:1275 getAffectsMask).
/// Missing template → C++ constructor default ALLIES|ENEMIES|NEUTRALS.
pub fn host_radius_damage_affects_for_weapon_name(name: &str) -> u32 {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store.find_weapon_template(name).map(|wt| wt.affects_mask.bits())
    })
    .ok()
    .flatten();
    from_store.unwrap_or(WEAPON_AFFECTS_DEFAULT)
}

#[allow(dead_code)]
pub(super) fn seed_radius_damage_affects_for(name: &str) -> u32 {
    // C++ getAffectsMask() — leftover WeaponStore / Weapon.cpp:271 default.
    // Do not invent ALLIES / NOT_AIRBORNE from name substrings.
    host_radius_damage_affects_for_weapon_name(name)
}

/// Whether splash may affect `victim` given C++ `getRelationship` and residual mask.
///
/// `relationship` is `curVictim->getRelationship(source)` (Weapon.cpp:1360-1372).
/// Leftover `should_apply_damage` uses the same ALLIES/ENEMIES/NEUTRALS bits.
pub fn radius_damage_affects_victim(
    affects: u32,
    relationship: gamelogic::common::Relationship,
    shooter_id: crate::game_logic::ObjectId,
    victim_id: crate::game_logic::ObjectId,
    victim_airborne: bool,
    same_template: bool,
) -> bool {
    use crate::game_logic::host_ai_path_combat_residual_wave105::{
        WEAPON_AFFECTS_ALLIES, WEAPON_AFFECTS_ENEMIES, WEAPON_AFFECTS_NEUTRALS, WEAPON_AFFECTS_SELF,
        WEAPON_DOESNT_AFFECT_AIRBORNE, WEAPON_DOESNT_AFFECT_SIMILAR,
    };
    use gamelogic::common::Relationship;

    if shooter_id == victim_id {
        return (affects & WEAPON_AFFECTS_SELF) != 0;
    }
    if (affects & WEAPON_DOESNT_AFFECT_AIRBORNE) != 0 && victim_airborne {
        return false;
    }
    if (affects & WEAPON_DOESNT_AFFECT_SIMILAR) != 0 && same_template {
        // C++ source->getRelationship(curVictim) == ALLIES && equivalent template.
        if relationship == Relationship::Allies {
            return false;
        }
    }
    let required_mask = match relationship {
        Relationship::Allies => WEAPON_AFFECTS_ALLIES,
        Relationship::Enemies => WEAPON_AFFECTS_ENEMIES,
        Relationship::Neutral => WEAPON_AFFECTS_NEUTRALS,
    };
    (affects & required_mask) != 0
}
