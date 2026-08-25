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
pub const PROJECTILE_COLLIDE_CONTROLLED_STRUCTURES: u32 = 0x0100;

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

/// Whether intervening structure intercept should scan at all.
///
/// C++ Weapon.cpp:716-721: STRUCTURES (other controllers), WALLS (fences),
/// or CONTROLLED_STRUCTURES (same controller) can all require a scan.
pub fn projectile_collides_structures(mask: u32) -> bool {
    (mask
        & (PROJECTILE_COLLIDE_STRUCTURES
            | PROJECTILE_COLLIDE_WALLS
            | PROJECTILE_COLLIDE_CONTROLLED_STRUCTURES))
        != 0
}

/// C++ `getControllingPlayer()` equality (`Weapon.cpp:718`).
///
/// Live objects may omit `owner_player_id`; fall back to team so Neutral
/// walls still intercept and same-faction skirmish slots stay distinct
/// when both player ids are present.
pub fn projectile_structure_same_controller(
    projectile_owner: Option<u32>,
    structure_owner: Option<u32>,
    same_team: bool,
) -> bool {
    match (projectile_owner, structure_owner) {
        (Some(a), Some(b)) => a == b,
        _ => same_team,
    }
}

/// C++ Weapon.cpp:716-721 leftover `should_projectile_collide_with`.
/// Same-controller structures collide only with CONTROLLED_STRUCTURES.
/// Other structures use STRUCTURES (live residual also accepts WALLS).
pub fn projectile_collides_with_structure(mask: u32, same_controller: bool) -> bool {
    if same_controller {
        (mask & PROJECTILE_COLLIDE_CONTROLLED_STRUCTURES) != 0
    } else {
        (mask & (PROJECTILE_COLLIDE_STRUCTURES | PROJECTILE_COLLIDE_WALLS)) != 0
    }
}

/// C++ Weapon.cpp:271 default `m_affectsMask` when RadiusDamageAffects is omitted.
pub const WEAPON_AFFECTS_DEFAULT: u32 =
    crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ALLIES
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

/// Whether splash may affect `victim` given C++ `getRelationship` and residual mask.
///
/// `relationship` is `curVictim->getRelationship(source)` (Weapon.cpp:1360-1372).
/// Leftover `should_apply_damage` uses the same ALLIES/ENEMIES/NEUTRALS bits.
///
/// `producer_id` is `source->getProducerID()` (Weapon.cpp:1330-1337). When
/// `WEAPON_AFFECTS_SELF` is unset, C++ skips both the firer and the building
/// that produced it (War Factory, airfield, Stinger Site).
pub fn radius_damage_affects_victim(
    affects: u32,
    relationship: gamelogic::common::Relationship,
    shooter_id: crate::game_logic::ObjectId,
    victim_id: crate::game_logic::ObjectId,
    producer_id: Option<crate::game_logic::ObjectId>,
    victim_airborne: bool,
    same_template: bool,
) -> bool {
    use crate::game_logic::host_ai_path_combat_residual_wave105::{
        WEAPON_AFFECTS_ALLIES, WEAPON_AFFECTS_ENEMIES, WEAPON_AFFECTS_NEUTRALS,
        WEAPON_AFFECTS_SELF, WEAPON_DOESNT_AFFECT_AIRBORNE, WEAPON_DOESNT_AFFECT_SIMILAR,
    };
    use gamelogic::common::Relationship;

    if shooter_id == victim_id || producer_id == Some(victim_id) {
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
