use super::*;

/// Wave 77: core host WeaponStore seed residual names that golden/skirmish combat
/// depends on. Fail-closed vs full Weapon.ini table.
pub const HOST_WEAPON_STORE_CORE_SEED_NAMES: &[&str] = &[
    RANGER_PRIMARY_WEAPON,
    RANGER_SECONDARY_WEAPON,
    GLA_REBEL_PRIMARY_WEAPON,
    REDGUARD_PRIMARY_WEAPON,
    HUMVEE_PRIMARY_WEAPON,
    HUMVEE_SECONDARY_WEAPON,
    PATRIOT_PRIMARY_WEAPON,
    PATRIOT_SECONDARY_WEAPON,
    STINGER_PRIMARY_WEAPON,
    GATTLING_BUILDING_PRIMARY_WEAPON,
    CRUSADER_TANK_GUN,
    TOMAHAWK_MISSILE_WEAPON,
    RAPTOR_JET_MISSILE_WEAPON,
    SCUD_GUN_EXPLOSIVE,
    BATTLE_MASTER_TANK_GUN,
    OVERLORD_TANK_GUN,
];

/// Honesty: host WeaponStore seed residual pack (Wave 77).
///
/// Ensures core combat residual names are registered after bootstrap and that
/// Ranger / Patriot clip residual fields match host seed table.
/// Fail-closed: not full Weapon.ini parse / full ClipSize volley state machine.
pub fn honesty_weapon_store_host_seed_residual_wave77() -> bool {
    let _ = ensure_host_weapon_store();
    let all_present = HOST_WEAPON_STORE_CORE_SEED_NAMES
        .iter()
        .all(|name| store_has(name));
    if !all_present {
        return false;
    }
    // Ranger residual: AdvancedCombatRifle damage/range seed residual.
    let ranger_ok = with_weapon_store(|store| {
        store
            .find_weapon_template(RANGER_PRIMARY_WEAPON)
            .map(|t| {
                t.primary_damage > 0.0 && t.attack_range > 0.0 && t.name == RANGER_PRIMARY_WEAPON
            })
            .unwrap_or(false)
    })
    .unwrap_or(false);
    // Patriot residual: primary missile seed present with positive range.
    let patriot_ok = with_weapon_store(|store| {
        store
            .find_weapon_template(PATRIOT_PRIMARY_WEAPON)
            .map(|t| t.primary_damage > 0.0 && t.attack_range > 0.0)
            .unwrap_or(false)
    })
    .unwrap_or(false);
    all_present && ranger_ok && patriot_ok && HOST_WEAPON_STORE_CORE_SEED_NAMES.len() >= 16
}

/// Wave 92 residual deepen: common ZH weapons beyond Wave 77 core seed names.
///
/// Host-testable residual for Marauder / Gattling / Comanche / Dragon / Scorpion
/// / Buggy / Pathfinder / MissileDefender / Paladin / Helix / Technical residual
/// damage/range tables. Fail-closed: not full Weapon.ini / full bonus condition matrix.
pub const HOST_WEAPON_STORE_DEEPEN_SEED_NAMES_WAVE92: &[&str] = &[
    MARAUDER_TANK_GUN,
    GATTLING_TANK_GUN,
    COMANCHE_PRIMARY_WEAPON,
    DRAGON_TANK_FLAME_WEAPON,
    SCORPION_TANK_GUN,
    BUGGY_ROCKET_WEAPON,
    PATHFINDER_SNIPER_WEAPON,
    MISSILE_DEFENDER_MISSILE_WEAPON,
    PALADIN_TANK_GUN,
    HELIX_MINIGUN_WEAPON,
    TECHNICAL_MACHINE_GUN,
    QUAD_CANNON_GUN,
    TOXIN_TRUCK_GUN,
    NAPALM_MISSILE_WEAPON,
    STEALTH_JET_MISSILE_WEAPON,
    TANK_HUNTER_PRIMARY_WEAPON,
];

/// Honesty: Wave 92 weapon template residual deepen pack.
///
/// Ensures deepen residual names are registered after bootstrap and that key
/// damage/range residual scalars match host seed table / Weapon.ini.
pub fn honesty_weapon_store_deepen_residual_wave92() -> bool {
    let _ = ensure_host_weapon_store();
    if !honesty_weapon_store_host_seed_residual_wave77() {
        return false;
    }
    let all_present = HOST_WEAPON_STORE_DEEPEN_SEED_NAMES_WAVE92
        .iter()
        .all(|name| store_has(name));
    if !all_present || HOST_WEAPON_STORE_DEEPEN_SEED_NAMES_WAVE92.len() < 16 {
        return false;
    }
    // Key residual damage/range scalars (Weapon.ini + host seed).
    let check = |name: &str, dmg: f32, range: f32| {
        with_weapon_store(|store| {
            store
                .find_weapon_template(name)
                .map(|t| {
                    (t.primary_damage - dmg).abs() < 0.05 && (t.attack_range - range).abs() < 0.05
                })
                .unwrap_or(false)
        })
        .unwrap_or(false)
    };
    check(MARAUDER_TANK_GUN, 60.0, 170.0)
        && check(GATTLING_TANK_GUN, 15.0, 150.0)
        && check(COMANCHE_PRIMARY_WEAPON, 6.0, 200.0)
        && check(DRAGON_TANK_FLAME_WEAPON, 10.0, 75.0)
        && check(SCORPION_TANK_GUN, 20.0, 150.0)
        && check(BUGGY_ROCKET_WEAPON, 20.0, 300.0)
        && check(PATHFINDER_SNIPER_WEAPON, 100.0, 300.0)
        && check(MISSILE_DEFENDER_MISSILE_WEAPON, 40.0, 175.0)
        && check(PALADIN_TANK_GUN, 60.0, 150.0)
        && check(HELIX_MINIGUN_WEAPON, 6.0, 115.0)
        && check(TECHNICAL_MACHINE_GUN, 10.0, 150.0)
        && check(QUAD_CANNON_GUN, 10.0, 150.0)
        && check(TANK_HUNTER_PRIMARY_WEAPON, 40.0, 175.0)
        && check(CRUSADER_TANK_GUN, 60.0, 150.0)
        && check(TOMAHAWK_MISSILE_WEAPON, 150.0, 350.0)
        && check(TOXIN_TRUCK_GUN, 10.0, 100.0)
        && check(NAPALM_MISSILE_WEAPON, 75.0, 320.0)
        && check(STEALTH_JET_MISSILE_WEAPON, 100.0, 220.0)
}

/// Wave 103 residual deepen: more Weapon.ini residual names beyond Wave 92.
///
/// Host-testable residual for NukeCannon / Inferno / Aurora / FireBase /
/// SentryDrone / Hellfire / JarmenKell / TunnelDefender / MiniGunner /
/// Overlord / BattleMaster / Comanche AT+pods / Avenger AA / SCUD toxin /
/// BlackNapalm residual damage/range tables.
/// Fail-closed: not full Weapon.ini / full ClipSize volley state machine.
pub const HOST_WEAPON_STORE_DEEPEN_SEED_NAMES_WAVE103: &[&str] = &[
    NUKE_CANNON_PRIMARY_WEAPON,
    INFERNO_CANNON_PRIMARY_WEAPON,
    AURORA_BOMB_PRIMARY_WEAPON,
    FIRE_BASE_HOWITZER_WEAPON,
    SENTRY_DRONE_GUN_WEAPON,
    HELLFIRE_MISSILE_WEAPON,
    JARMEN_KELL_RIFLE,
    TUNNEL_DEFENDER_ROCKET_WEAPON,
    MINIGUNNER_GUN,
    OVERLORD_TANK_GUN,
    BATTLE_MASTER_TANK_GUN,
    COMANCHE_ANTITANK_WEAPON,
    COMANCHE_ROCKET_POD_WEAPON,
    AVENGER_AIR_LASER,
    SCUD_GUN_TOXIN,
    BLACK_NAPALM_MISSILE_WEAPON,
];

/// Honesty: Wave 103 weapon template residual deepen pack.
///
/// Ensures deepen residual names are registered after bootstrap and that key
/// damage/range residual scalars match host seed table / Weapon.ini.
/// Fail-closed: not full Weapon.ini parse / multi-bonus condition matrix.
pub fn honesty_weapon_store_deepen_residual_wave103() -> bool {
    let _ = ensure_host_weapon_store();
    if !honesty_weapon_store_deepen_residual_wave92() {
        return false;
    }
    let all_present = HOST_WEAPON_STORE_DEEPEN_SEED_NAMES_WAVE103
        .iter()
        .all(|name| store_has(name));
    if !all_present || HOST_WEAPON_STORE_DEEPEN_SEED_NAMES_WAVE103.len() < 16 {
        return false;
    }
    let check = |name: &str, dmg: f32, range: f32| {
        with_weapon_store(|store| {
            store
                .find_weapon_template(name)
                .map(|t| {
                    (t.primary_damage - dmg).abs() < 0.05 && (t.attack_range - range).abs() < 0.05
                })
                .unwrap_or(false)
        })
        .unwrap_or(false)
    };
    check(NUKE_CANNON_PRIMARY_WEAPON, 400.0, 350.0)
        && check(INFERNO_CANNON_PRIMARY_WEAPON, 30.0, 300.0)
        && check(AURORA_BOMB_PRIMARY_WEAPON, 400.0, 300.0)
        && check(FIRE_BASE_HOWITZER_WEAPON, 75.0, 275.0)
        && check(SENTRY_DRONE_GUN_WEAPON, 8.0, 150.0)
        && check(HELLFIRE_MISSILE_WEAPON, 40.0, 150.0)
        && check(JARMEN_KELL_RIFLE, 180.0, 225.0)
        && check(TUNNEL_DEFENDER_ROCKET_WEAPON, 40.0, 175.0)
        && check(MINIGUNNER_GUN, 10.0, 125.0)
        && check(OVERLORD_TANK_GUN, 80.0, 175.0)
        && check(BATTLE_MASTER_TANK_GUN, 60.0, 150.0)
        && check(COMANCHE_ANTITANK_WEAPON, 50.0, 200.0)
        && check(COMANCHE_ROCKET_POD_WEAPON, 30.0, 200.0)
        && check(AVENGER_AIR_LASER, 10.0, 300.0)
        && check(SCUD_GUN_TOXIN, 200.0, 350.0)
        && check(BLACK_NAPALM_MISSILE_WEAPON, 75.0, 320.0)
}

