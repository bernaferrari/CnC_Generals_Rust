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

/// C++ Weapon.ini RadiusDamageAffects residual mask (Regular).
///
/// Bits match `host_ai_path_combat_residual_wave105` WeaponAffectsMaskType.
/// Default when unset: ENEMIES | NEUTRALS (retail common residual).
pub fn host_radius_damage_affects_for_weapon_name(name: &str) -> u32 {
    seed_radius_damage_affects_for(name)
}

pub(super) fn seed_radius_damage_affects_for(name: &str) -> u32 {
    use crate::game_logic::host_ai_path_combat_residual_wave105::{
        WEAPON_AFFECTS_ALLIES, WEAPON_AFFECTS_ENEMIES, WEAPON_AFFECTS_NEUTRALS,
        WEAPON_AFFECTS_SELF, WEAPON_DOESNT_AFFECT_AIRBORNE, WEAPON_DOESNT_AFFECT_SIMILAR,
        WEAPON_KILLS_SELF,
    };
    let n = name.to_ascii_lowercase();
    // Default retail residual: enemies + neutrals.
    let enemies_neutrals = WEAPON_AFFECTS_ENEMIES | WEAPON_AFFECTS_NEUTRALS;
    if n.contains("scudstorm")
        || n.contains("nuke")
        || n.contains("moab")
        || n.contains("anthrax")
        || n.contains("tomahawk")
        || n.contains("overlord")
        || n.contains("demotrap")
        || n.contains("terrorist")
        || n.contains("suicide")
        || n.contains("carbomb")
    {
        // Friendly-fire residual weapons.
        return WEAPON_AFFECTS_ALLIES | enemies_neutrals;
    }
    if n.contains("microwave") {
        // Microwave residual: enemies + neutrals only (allies false).
        return enemies_neutrals;
    }
    if n.contains("ecm") || n.contains("jammer") {
        return enemies_neutrals;
    }
    if n.contains("propaganda") {
        // Heal-ish residual: allies only (not used as splash damage usually).
        return WEAPON_AFFECTS_ALLIES;
    }
    if n.contains("demo") && n.contains("truck") {
        return WEAPON_AFFECTS_ALLIES | enemies_neutrals | WEAPON_AFFECTS_SELF | WEAPON_KILLS_SELF;
    }
    // Standard combat splash residual.
    let _ = (
        WEAPON_AFFECTS_SELF,
        WEAPON_DOESNT_AFFECT_SIMILAR,
        WEAPON_DOESNT_AFFECT_AIRBORNE,
    );
    enemies_neutrals
}

/// Whether splash may affect `victim` given shooter team and residual mask.
pub fn radius_damage_affects_victim(
    affects: u32,
    shooter_team: crate::game_logic::Team,
    shooter_id: crate::game_logic::ObjectId,
    victim_id: crate::game_logic::ObjectId,
    victim_team: crate::game_logic::Team,
    victim_airborne: bool,
    same_template: bool,
) -> bool {
    use crate::game_logic::host_ai_path_combat_residual_wave105::{
        WEAPON_AFFECTS_ALLIES, WEAPON_AFFECTS_ENEMIES, WEAPON_AFFECTS_NEUTRALS,
        WEAPON_AFFECTS_SELF, WEAPON_DOESNT_AFFECT_AIRBORNE, WEAPON_DOESNT_AFFECT_SIMILAR,
    };
    use crate::game_logic::Team;

    if shooter_id == victim_id {
        return (affects & WEAPON_AFFECTS_SELF) != 0;
    }
    if (affects & WEAPON_DOESNT_AFFECT_AIRBORNE) != 0 && victim_airborne {
        return false;
    }
    if (affects & WEAPON_DOESNT_AFFECT_SIMILAR) != 0 && same_template {
        // Similar skip only for allies residual (C++ friends same template).
        if shooter_team == victim_team && shooter_team != Team::Neutral {
            return false;
        }
    }
    if victim_team == Team::Neutral {
        return (affects & WEAPON_AFFECTS_NEUTRALS) != 0;
    }
    if shooter_team == victim_team {
        return (affects & WEAPON_AFFECTS_ALLIES) != 0;
    }
    // Different non-neutral factions → enemies residual.
    (affects & WEAPON_AFFECTS_ENEMIES) != 0
}
