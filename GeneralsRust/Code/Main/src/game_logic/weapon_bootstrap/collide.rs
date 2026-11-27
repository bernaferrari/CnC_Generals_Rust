use super::*;

pub use generals_sim::combat::projectile_contact::{
    projectile_collides_structures_or_fences, projectile_collides_with_structure_or_fence,
    projectile_structure_same_controller,
};
pub use generals_sim::combat::projectile_impact::radius_damage_affects_victim;
use generals_sim::weapon::WeaponCollideMask;

/// C++ ProjectileCollideMask bits (Weapon.cpp TheWeaponCollideMaskNames).
pub const PROJECTILE_COLLIDE_ALLIES: u32 = WeaponCollideMask::ALLIES;
pub const PROJECTILE_COLLIDE_ENEMIES: u32 = WeaponCollideMask::ENEMIES;
pub const PROJECTILE_COLLIDE_STRUCTURES: u32 = WeaponCollideMask::STRUCTURES;
pub const PROJECTILE_COLLIDE_SHRUBBERY: u32 = WeaponCollideMask::SHRUBBERY;
pub const PROJECTILE_COLLIDE_PROJECTILES: u32 = WeaponCollideMask::PROJECTILE;
pub const PROJECTILE_COLLIDE_WALLS: u32 = WeaponCollideMask::WALLS;
pub const PROJECTILE_COLLIDE_SMALL_MISSILES: u32 = WeaponCollideMask::SMALL_MISSILES;
pub const PROJECTILE_COLLIDE_BALLISTIC_MISSILES: u32 = WeaponCollideMask::BALLISTIC_MISSILES;
pub const PROJECTILE_COLLIDE_CONTROLLED_STRUCTURES: u32 = WeaponCollideMask::CONTROLLED_STRUCTURES;

/// Default ballistic residual: structures + walls (most tank/missile shells).
/// C++ Weapon.cpp:273 default is STRUCTURES only — own buildings require
/// CONTROLLED_STRUCTURES (0x0100), which this default does not set.
pub const PROJECTILE_COLLIDE_DEFAULT: u32 =
    PROJECTILE_COLLIDE_STRUCTURES | PROJECTILE_COLLIDE_WALLS;

/// C++ Weapon.ini ProjectileCollidesWith residual mask (Regular).
///
/// Fail-closed: gates intervening-structure intercept only — not full shrubbery /
/// projectile-vs-projectile collide matrix.
pub fn host_projectile_collides_for_weapon_name(name: &str) -> u32 {
    use super::with_host_weapon_store as with_weapon_store;
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

/// C++ Weapon.cpp:271 default `m_affectsMask` when RadiusDamageAffects is omitted.
pub const WEAPON_AFFECTS_DEFAULT: u32 = generals_sim::weapon::WeaponAffectsMask::DEFAULT;

/// C++ Weapon.ini RadiusDamageAffects residual mask (Regular).
///
/// Live fire reads the leftover WeaponStore (Weapon.cpp:1275 getAffectsMask).
/// Missing template → C++ constructor default ALLIES|ENEMIES|NEUTRALS.
pub fn host_radius_damage_affects_for_weapon_name(name: &str) -> u32 {
    use super::with_host_weapon_store as with_weapon_store;
    let from_store = with_weapon_store(|store| {
        store
            .find_weapon_template(name)
            .map(|wt| wt.affects_mask.bits())
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

/// Compare templates in the driving match's effective content. C++ resolves
/// both objects' ThingTemplates and compares their reskins/build variations;
/// another match's factory cannot supply either side of that comparison.
pub(in crate::game_logic) fn receiving_splash_templates_equivalent(
    templates: &std::collections::HashMap<String, crate::game_logic::ThingTemplate>,
    shooter: &str,
    victim: &str,
) -> bool {
    match (templates.get(shooter), templates.get(victim)) {
        (Some(source), Some(target)) => source.is_equivalent_to(target),
        _ => false,
    }
}

/// C++ `source->getTemplate()->isEquivalentTo(curVictim->getTemplate())`
/// (`Weapon.cpp:1345`, `ThingTemplate.cpp:1454`).
///
/// Live splash only stores template names. Identity is case-insensitive; reskin
/// / `BuildVariations` walk leftover `ThingTemplate::is_equivalent_to` on the
/// already-loaded factory. Never `TheThingFactory::find_template` — that helper
/// can rebuild the Object INI database on a miss.
pub fn splash_templates_equivalent(shooter: &str, victim: &str) -> bool {
    if shooter.is_empty() || victim.is_empty() {
        return false;
    }
    if shooter.eq_ignore_ascii_case(victim) {
        return true;
    }
    leftover_templates_equivalent(shooter, victim)
}

fn leftover_templates_equivalent(shooter: &str, victim: &str) -> bool {
    let Some(guard) = game_engine::common::thing::thing_factory::try_get_thing_factory() else {
        return false;
    };
    let Some(factory) = guard.as_ref() else {
        return false;
    };
    let Some(source) = factory.find_template(shooter, false) else {
        return false;
    };
    let Some(other) = factory.find_template(victim, false) else {
        return false;
    };
    source.is_equivalent_to(other.as_ref())
}
