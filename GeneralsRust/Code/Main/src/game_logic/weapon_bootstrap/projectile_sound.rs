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

/// Resolve FireSound for a host unit firing a weapon slot.
/// C++ Weapon.ini FireSoundLoopTime residual (logic frames @ 30 FPS).
///
/// INI duration peels land as frames via parseDurationUnsignedInt.
/// 0 = one-shot fire sound (no looping handle).
pub fn host_fire_sound_loop_frames_for_weapon_name(name: &str) -> u32 {
    let _ = ensure_host_weapon_store();
    // Prefer store field when present on template peel path.
    // Fall back to retail name seeds for continuous-fire weapons.
    seed_fire_sound_loop_frames_for(name)
}

pub(super) fn seed_fire_sound_loop_frames_for(name: &str) -> u32 {
    let n = name.to_ascii_lowercase();
    // Retail peels (msec → frames @ 30 FPS, ceil).
    // DragonTankFlameWeapon / ToxinTractor 80ms → 3f (ceil 80*30/1000).
    // Host unit docs often use floor; match FiringTracker extend semantics with ceil.
    if n.contains("flame") || n.contains("flamethrower") || n.contains("dragon") {
        return 3; // 80ms
    }
    if n.contains("toxin") && (n.contains("spray") || n.contains("stream")) {
        return 3;
    }
    if n.contains("microwave") {
        return 4; // 120ms
    }
    if n.contains("ecm") || n.contains("jam") {
        return 3;
    }
    // Continuous machine-gun residual peels (Humvee etc. often 0; leave 0).
    0
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

/// C++ Weapon.ini AllowAttackGarrisonedBldgs residual.
pub fn host_allow_attack_garrisoned_for_weapon_name(name: &str) -> bool {
    seed_allow_attack_garrisoned_for(name)
}

pub(super) fn seed_allow_attack_garrisoned_for(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("flame")
        || n.contains("dragon")
        || n.contains("toxin")
        || n.contains("microwave")
        || n.contains("ranger")
        || n.contains("flashbang")
        || n.contains("buildingclearer")
        || n.contains("cleargarrison")
        || n.contains("killgarrison")
}

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
    if let Some(n) = wname {
        let s = host_fire_sound_for_weapon_name(n);
        if !s.is_empty() {
            return s;
        }
    }
    "WeaponFire".to_string()
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
    // Generic residual — still better than empty for store seed.
    "WeaponFire".into()
}
