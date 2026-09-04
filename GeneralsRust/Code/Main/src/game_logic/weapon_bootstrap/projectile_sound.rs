use super::*;

/// C++ Weapon.ini ProjectileObject residual name for a store weapon template.
pub fn host_projectile_name_for_weapon_name(name: &str) -> String {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store.find_weapon_template(name).map(|wt| {
            let n = wt.projectile_name.trim();
            n.to_string()
        })
    })
    .ok()
    .flatten();
    if let Some(s) = from_store.filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("NONE")) {
        return s;
    }
    seed_projectile_name_for(name)
}

/// Resolve ProjectileObject for a host unit firing a weapon slot.
pub fn host_projectile_name_for_unit_slot(
    template_name: &str,
    primary_weapon_name: Option<&str>,
    secondary_weapon_name: Option<&str>,
    slot: u8,
) -> String {
    let wname = if slot == 1 {
        secondary_weapon_name
            .or_else(|| secondary_weapon_name_for_unit(template_name))
            .or(primary_weapon_name)
            .or_else(|| primary_weapon_name_for_unit(template_name))
    } else {
        primary_weapon_name.or_else(|| primary_weapon_name_for_unit(template_name))
    };
    match wname {
        Some(n) => host_projectile_name_for_weapon_name(n),
        None => String::new(),
    }
}

pub(super) fn seed_projectile_name_for(name: &str) -> String {
    let n = name.to_ascii_lowercase();
    // Hitscan / laser residual — no projectile object.
    if n.contains("laser")
        || n.contains("flame")
        || n.contains("gattling")
        || n.contains("minigun")
        || n.contains("machinegun")
        || n.contains("combatrifle")
        || n.contains("redguard")
        || (n.contains("ranger") && !n.contains("flash") && !n.contains("missile"))
    {
        return String::new();
    }
    if n.contains("tomahawk") {
        return "TomahawkMissile".into();
    }
    if n.contains("scud") {
        return "ScudMissile".into();
    }
    if n.contains("patriot") || n.contains("stinger") {
        return "PatriotMissile".into();
    }
    if n.contains("rpg") || n.contains("tankhunter") || n.contains("tunneldefender") {
        return "GenericTankShell".into(); // many residual peels use tank shell / rocket proxy
    }
    if n.contains("missile") || n.contains("defender") {
        return "GenericMissile".into();
    }
    if n.contains("tankgun")
        || n.contains("crusader")
        || n.contains("paladin")
        || n.contains("battlemaster")
        || n.contains("scorpion")
        || n.contains("marauder")
        || n.contains("firebase")
    {
        return "GenericTankShell".into();
    }
    if n.contains("nuke") || n.contains("neutron") {
        return "NukeCannonShell".into();
    }
    if n.contains("aurora") || n.contains("bomb") {
        return "AuroraBomb".into();
    }
    if n.contains("raptor") || n.contains("mig") || n.contains("stealth") || n.contains("jet") {
        return "JetMissile".into();
    }
    if n.contains("comanche") || n.contains("rocket") || n.contains("buggy") {
        return "RocketBuggyMissile".into();
    }
    if n.contains("flash") || n.contains("grenade") {
        return "FlashBangGrenade".into();
    }
    String::new()
}

pub fn host_fire_sound_for_weapon_name(name: &str) -> String {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store.find_weapon_template(name).and_then(|wt| {
            let n = wt.fire_sound.name().trim();
            if n.is_empty() {
                None
            } else {
                Some(n.to_string())
            }
        })
    })
    .ok()
    .flatten();
    if let Some(s) = from_store {
        return s;
    }
    // Store missing FireSound (pre-seed residual templates) → name peel.
    seed_fire_sound_for(name)
}

/// C++ FiringTracker::shotFired FireSoundLoopTime residual (logic frames).
///
/// Leftover `WeaponTemplate.fire_sound_loop_time` is the C++ field
/// (`Weapon.h:679` getFireSoundLoopTime). Missing template or authored 0
/// → one-shot fire sound (no looping handle).
pub fn host_fire_sound_loop_frames_for_weapon_name(name: &str) -> u32 {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    with_weapon_store(|store| {
        store
            .find_weapon_template(name)
            .map(|wt| wt.fire_sound_loop_time)
    })
    .ok()
    .flatten()
    .unwrap_or(0)
}

/// C++ Weapon.ini PlayFXWhenStealthed residual.
///
/// When false (default), stealthed+undetected+non-disguised shooters suppress
/// FireFX (C++ Weapon::fireWeaponTemplate handleWeaponFireFX gate). Mines always
/// play FX. When true, FX plays even while stealthed.
pub fn host_play_fx_when_stealthed_for_weapon_name(name: &str) -> bool {
    use gamelogic::weapon::with_weapon_store;

    let _ = ensure_host_weapon_store();
    with_weapon_store(|store| {
        store
            .find_weapon_template(name)
            .map(|weapon| weapon.play_fx_when_stealthed)
    })
    .ok()
    .flatten()
    .unwrap_or(false)
}

/// C++ Weapon.ini AllowAttackGarrisonedBldgs / WeaponTemplate::m_allowAttackGarrisonedBldgs.
///
/// Leftover store is source of truth. Estimate-only (C++ estimateWeaponTemplateDamage
/// vs occupied garrisonable). Default false when the template is missing — do
/// not invent the flag from weapon-name substrings. Occupant-kill is
/// DamageType=KILL_GARRISONED, not this flag.
pub fn host_allow_attack_garrisoned_for_weapon_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    with_weapon_store(|store| {
        store
            .find_weapon_template(name)
            .map(|weapon| weapon.allow_attack_garrisoned_bldgs)
    })
    .ok()
    .flatten()
    .unwrap_or(false)
}

/// C++ Weapon.h:678 `getFireSound()` resolves the WeaponTemplate's authored
/// AudioEventRTS; an unknown/unauthored weapon has no FireSound, and
/// AudioManager::addAudioEvent returns AHSV_NoSound for the empty name
/// (GameAudio.cpp:384-386). So the no-weapon result is empty — never an
/// invented generic token ("WeaponFire" is not a retail AudioEvent).
pub fn host_fire_sound_for_unit_slot(
    template_name: &str,
    primary_weapon_name: Option<&str>,
    secondary_weapon_name: Option<&str>,
    slot: u8,
) -> String {
    let wname = if slot == 1 {
        secondary_weapon_name
            .or_else(|| secondary_weapon_name_for_unit(template_name))
            .or(primary_weapon_name)
            .or_else(|| primary_weapon_name_for_unit(template_name))
    } else {
        primary_weapon_name.or_else(|| primary_weapon_name_for_unit(template_name))
    };
    wname.map(host_fire_sound_for_weapon_name)
        .unwrap_or_default()
}

pub(super) fn seed_fire_sound_for(name: &str) -> String {
    let n = name.to_ascii_lowercase();
    // Retail residual peels used by host honesty packs / unit peels.
    if n.contains("rpg") || n.contains("tankhunter") || n.contains("tunneldefender") {
        return "RPGTrooperWeapon".into();
    }
    if n.contains("jarmen") || n.contains("snipe") || n.contains("pathfinder") {
        return "JarmenKellWeaponSnipe".into();
    }
    if n.contains("terrorist") || n.contains("suicide") || n.contains("carbomb") {
        return "CarBomberDie".into();
    }
    if n.contains("tomahawk") {
        return "TomahawkWeapon".into();
    }
    if n.contains("scud") {
        return "ScudLauncherWeapon".into();
    }
    if n.contains("patriot") {
        return "PatriotBatteryWeapon".into();
    }
    if n.contains("ranger") && n.contains("flash") {
        return "RangerFlashBang".into();
    }
    if n.contains("laser") || n.contains("pointdefense") || n.contains("point_defense") {
        return "LaserFire".into();
    }
    if n.contains("ranger") || n.contains("machinegun") || n.contains("combatrifle") {
        return "MachineGunFire".into();
    }
    if n.contains("tankgun")
        || n.contains("crusader")
        || n.contains("paladin")
        || n.contains("battlemaster")
    {
        return "TankGunFire".into();
    }
    if n.contains("missile") || n.contains("stinger") {
        return "MissileLaunch".into();
    }
    if n.contains("flame") || n.contains("dragon") || n.contains("inferno") {
        return "FlameWeaponFire".into();
    }
    if n.contains("gattling") || n.contains("gatling") || n.contains("minigun") {
        return "GattlingFire".into();
    }
    if n.contains("nuke") {
        return "NukeCannonFire".into();
    }
    if n.contains("aurora") || n.contains("bomb") {
        return "BombDrop".into();
    }
    // No authored FireSound peel — C++ plays nothing (AHSV_NoSound,
    // GameAudio.cpp:384-386); inventing a generic token only dead-ends as
    // THE_AUDIO ERR(no-info).
    String::new()
}
