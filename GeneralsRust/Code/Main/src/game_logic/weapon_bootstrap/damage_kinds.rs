use super::*;

/// C++ Weapon.ini ShowsAmmoPips / WeaponTemplate::m_isShowsAmmoPips.
///
/// Leftover store is source of truth. Default false when the template is
/// missing — do not invent the flag from weapon-name substrings.
pub fn host_shows_ammo_pips_for_weapon_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    with_weapon_store(|store| {
        store
            .find_weapon_template(name)
            .map(|weapon| weapon.is_shows_ammo_pips)
    })
    .ok()
    .flatten()
    .unwrap_or(false)
}

/// C++ Weapon.ini CapableOfFollowingWaypoints / WeaponTemplate::m_capableOfFollowingWaypoint.
///
/// Leftover store is source of truth. Default false when the template is
/// missing — do not invent the flag from weapon-name substrings.
pub fn host_capable_of_following_waypoint_for_weapon_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    with_weapon_store(|store| {
        store
            .find_weapon_template(name)
            .map(|weapon| weapon.capable_of_following_waypoint)
    })
    .ok()
    .flatten()
    .unwrap_or(false)
}

/// C++ Weapon.ini ProjectileStreamName residual.
///
/// Prefer the live WeaponStore field (this template's stream). Seed is only
/// a fallback when the store has no authored name.
pub fn host_projectile_stream_name_for_weapon_name(name: &str) -> String {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store.find_weapon_template(name).and_then(|wt| {
            let authored = wt.projectile_stream_name.trim();
            if authored.is_empty() {
                None
            } else {
                Some(authored.to_string())
            }
        })
    })
    .ok()
    .flatten();
    if let Some(authored) = from_store {
        return authored;
    }
    seed_projectile_stream_name_for(name)
}

/// Resolve ProjectileStreamName from the firing slot, then any other slot.
pub fn host_projectile_stream_name_for_slots(
    preferred: Option<&str>,
    primary: Option<&str>,
    secondary: Option<&str>,
    tertiary: Option<&str>,
) -> String {
    for name in [preferred, primary, secondary, tertiary]
        .into_iter()
        .flatten()
    {
        let authored = host_projectile_stream_name_for_weapon_name(name);
        if !authored.is_empty() {
            return authored;
        }
    }
    String::new()
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

/// C++ Weapon.ini MissileCallsOnDie / WeaponTemplate::m_dieOnDetonate.
///
/// Leftover store is source of truth. Default false when the template is
/// missing — do not invent the flag from weapon-name substrings.
pub fn host_die_on_detonate_for_weapon_name(name: &str) -> bool {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    with_weapon_store(|store| {
        store
            .find_weapon_template(name)
            .map(|weapon| weapon.die_on_detonate)
    })
    .ok()
    .flatten()
    .unwrap_or(false)
}

/// C++ Weapon.ini DamageDealtAtSelfPosition / WeaponTemplate::m_damageDealtAtSelfPosition.
///
/// Leftover store is source of truth. Default false when the template is
/// missing — do not invent the flag from weapon-name substrings.
pub fn host_damage_dealt_at_self_position_for_weapon_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    with_weapon_store(|store| {
        store
            .find_weapon_template(name)
            .map(|weapon| weapon.damage_dealt_at_self_position)
    })
    .ok()
    .flatten()
    .unwrap_or(false)
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
    n.contains("surrender") || n.contains("flashbang")
}

/// C++ Weapon.ini DamageType=KILL_GARRISONED.
///
/// Leftover store is the occupant-kill gate. Default false when the template
/// is missing — do not invent KILL_GARRISONED from weapon-name substrings.
/// AllowAttackGarrisonedBldgs is estimate-only and must not trip this.
pub fn host_weapon_is_kill_garrisoned_damage(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    with_weapon_store(|store| {
        store
            .find_weapon_template(name)
            .map(|weapon| weapon.damage_type == gamelogic::damage::DamageType::KillGarrisoned)
    })
    .ok()
    .flatten()
    .unwrap_or(false)
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
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store
            .find_weapon_template(name)
            .map(|wt| object_status_bit_name(wt.damage_status_type.into()))
    })
    .ok()
    .flatten()
    .flatten();
    if from_store.is_some() {
        return from_store;
    }
    if !host_weapon_is_status_damage(name) {
        return None;
    }
    Some("FAERIE_FIRE")
}

fn object_status_bit_name(status: gamelogic::common::ObjectStatusTypes) -> Option<&'static str> {
    use gamelogic::common::ObjectStatusTypes::{self, *};
    match status {
        ObjectStatusTypes::None => Option::<&str>::None,
        Destroyed => Some("DESTROYED"),
        CanAttack => Some("CAN_ATTACK"),
        UnderConstruction => Some("UNDER_CONSTRUCTION"),
        Unselectable => Some("UNSELECTABLE"),
        NoCollisions => Some("NO_COLLISIONS"),
        NoAttack => Some("NO_ATTACK"),
        AirborneTarget => Some("AIRBORNE_TARGET"),
        Parachuting => Some("PARACHUTING"),
        Repulsor => Some("REPULSOR"),
        Hijacked => Some("HIJACKED"),
        Aflame => Some("AFLAME"),
        Burned => Some("BURNED"),
        Wet => Some("WET"),
        IsFiringWeapon => Some("IS_FIRING_WEAPON"),
        Braking => Some("IS_BRAKING"),
        Stealthed => Some("STEALTHED"),
        Detected => Some("DETECTED"),
        CanStealth => Some("CAN_STEALTH"),
        Sold => Some("SOLD"),
        UndergoingRepair => Some("UNDERGOING_REPAIR"),
        Reconstructing => Some("RECONSTRUCTING"),
        Masked => Some("MASKED"),
        IsAttacking => Some("IS_ATTACKING"),
        IsUsingAbility => Some("USING_ABILITY"),
        IsAimingWeapon => Some("IS_AIMING_WEAPON"),
        NoAttackFromAi => Some("NO_ATTACK_FROM_AI"),
        IgnoringStealth => Some("IGNORING_STEALTH"),
        IsCarBomb => Some("IS_CARBOMB"),
        DeckHeightOffset => Some("DECK_HEIGHT_OFFSET"),
        Rider1 => Some("STATUS_RIDER1"),
        Rider2 => Some("STATUS_RIDER2"),
        Rider3 => Some("STATUS_RIDER3"),
        Rider4 => Some("STATUS_RIDER4"),
        Rider5 => Some("STATUS_RIDER5"),
        Rider6 => Some("STATUS_RIDER6"),
        Rider7 => Some("STATUS_RIDER7"),
        Rider8 => Some("STATUS_RIDER8"),
        FaerieFire => Some("FAERIE_FIRE"),
        MissileKillingSelf => Some("KILLING_SELF"),
        ReassignParking => Some("REASSIGN_PARKING"),
        BoobyTrapped => Some("BOOBY_TRAPPED"),
        Immobile => Some("IMMOBILE"),
        Disguised => Some("DISGUISED"),
        Deployed => Some("DEPLOYED"),
    }
}

/// C++ STATUS damage duration: PrimaryDamage is msec → logic frames @ 30 FPS.
pub fn host_status_damage_frames_from_primary_damage(primary_damage_msec: f32) -> u32 {
    ((primary_damage_msec.max(0.0) * 30.0) / 1000.0).ceil() as u32
}
