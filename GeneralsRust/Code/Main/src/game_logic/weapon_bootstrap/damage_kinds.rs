use super::*;

/// C++ Weapon.ini ShowsAmmoPips residual.
pub fn host_shows_ammo_pips_for_weapon_name(name: &str) -> bool {
    seed_shows_ammo_pips_for(name)
}

pub(super) fn seed_shows_ammo_pips_for(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    // Aircraft missile/clip residual + raptor/comet/aurora style.
    (n.contains("raptor") && n.contains("missile"))
        || (n.contains("mig") && n.contains("missile"))
        || n.contains("stealthjet")
        || n.contains("aurora")
        || (n.contains("missile") && (n.contains("jet") || n.contains("aircraft")))
        || n.contains("scud") && n.contains("launcher")
}

/// C++ Weapon.ini CapableOfFollowingWaypoints residual.
pub fn host_capable_of_following_waypoint_for_weapon_name(name: &str) -> bool {
    seed_capable_of_following_waypoint_for(name)
}

pub(super) fn seed_capable_of_following_waypoint_for(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    // Cruise / guided residual weapons that C++ marks CapableOfFollowingWaypoints.
    n.contains("scud")
        || n.contains("tomahawk")
        || n.contains("cruise")
        || n.contains("particlecannon")
        || n.contains("nuke") && n.contains("missile")
}

/// C++ Weapon.ini ProjectileStreamName residual.
pub fn host_projectile_stream_name_for_weapon_name(name: &str) -> String {
    seed_projectile_stream_name_for(name)
}

pub(super) fn seed_projectile_stream_name_for(name: &str) -> String {
    let n = name.to_ascii_lowercase();
    if n.contains("flame") || n.contains("dragon") {
        return "DragonTankFlameStream".into();
    }
    if n.contains("toxin") && (n.contains("spray") || n.contains("stream")) {
        return "ToxinStream".into();
    }
    if n.contains("microwave") {
        return "MicrowaveDisableStream".into();
    }
    if n.contains("cleanup") || n.contains("hazard") {
        return "CleanupHazardProjectileStream".into();
    }
    // Continuous MG residual streams (optional empty).
    String::new()
}

/// C++ Weapon.ini MissileCallsOnDie / DieOnDetonate residual.
pub fn host_die_on_detonate_for_weapon_name(name: &str) -> bool {
    seed_die_on_detonate_for(name)
}

pub(super) fn seed_die_on_detonate_for(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    // Retail missiles that call onDie after detonation residual.
    (n.contains("scud") && n.contains("missile"))
        || n.contains("tomahawk")
        || (n.contains("rocket") && (n.contains("buggy") || n.contains("missile")))
        || n.contains("stinger") && n.contains("missile")
        || n.contains("patriot") && n.contains("missile")
        || n.contains("inferno") && n.contains("cannon")
}

/// C++ Weapon.ini DamageType=STATUS residual.
pub fn host_weapon_is_status_damage(name: &str) -> bool {
    seed_weapon_is_status_damage(name)
}

pub(super) fn seed_weapon_is_status_damage(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("designator")
        || n.contains("targetdesignator")
        || (n.contains("avenger") && n.contains("target"))
        || n.contains("statusweapon")
}

/// C++ Weapon.ini DamageType=KILL_PILOT residual (Jarmen Kell sniper).
pub fn host_weapon_is_kill_pilot_damage(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("snipe")
        || n.contains("killpilot")
        || n.contains("kill_pilot")
        || (n.contains("jarmen") && n.contains("rifle"))
        || n.contains("pathfindersniper")
        || n.contains("sniperrifle")
}

/// C++ Weapon.ini DamageType=DISARM residual (Dozer/Worker mine clear).
pub fn host_weapon_is_disarm_damage(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("minedisarm")
        || n.contains("mine_disarm")
        || n.contains("disarming")
        || n.contains("disarmweapon")
        || (n.contains("dozer") && n.contains("mine"))
        || (n.contains("worker") && n.contains("mine"))
}

/// C++ Weapon.ini DamageType=DEPLOY residual (TroopCrawlerAssault).
pub fn host_weapon_is_deploy_damage(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("assault") && (n.contains("deploy") || n.contains("crawler") || n.contains("troop"))
        || n == "troopcrawlerassault"
        || n.contains("deployweapon")
}

/// C++ Weapon.ini DamageType=HACK residual (Black Lotus / Hacker).
pub fn host_weapon_is_hack_damage(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("hack") || n.contains("blacklotus") || n.contains("capturebuilding")
}

/// C++ Weapon.ini DamageType=SURRENDER residual.
pub fn host_weapon_is_surrender_damage(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("surrender")
}

/// C++ Weapon.ini DamageType=KILL_GARRISONED residual (building clearers).
pub fn host_weapon_is_kill_garrisoned_damage(name: &str) -> bool {
    crate::game_logic::host_bunker_buster::is_kill_garrisoned_clearer_weapon(name) || {
        let n = name.to_ascii_lowercase();
        n.contains("buildingclearer") || n.contains("killgarrison") || n.contains("clearbuilding")
    }
}

/// C++ Weapon.ini / body DamageType=HEALING residual (medics, rebuild holes, flight deck).
pub fn host_weapon_is_healing_damage(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("heal")
        || n.contains("medic")
        || n.contains("ambulance")
        || (n.contains("repair") && !n.contains("air"))
        || n.contains("rebuildhole")
}

/// C++ DamageType=WATER residual (terrain underwater / waveguide).
pub fn host_weapon_is_water_damage(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("waterdamage") || n.contains("waveguide") || n == "water"
}

/// C++ Weapon.ini DamageStatusType residual name (OBJECT_STATUS bit name).
pub fn host_damage_status_type_for_weapon_name(name: &str) -> Option<&'static str> {
    if !host_weapon_is_status_damage(name) {
        return None;
    }
    let n = name.to_ascii_lowercase();
    if n.contains("designator") || n.contains("faerie") || n.contains("avenger") {
        return Some("FAERIE_FIRE");
    }
    Some("FAERIE_FIRE")
}

/// C++ STATUS damage duration: PrimaryDamage is msec → logic frames @ 30 FPS.
pub fn host_status_damage_frames_from_primary_damage(primary_damage_msec: f32) -> u32 {
    ((primary_damage_msec.max(0.0) * 30.0) / 1000.0).ceil() as u32
}

