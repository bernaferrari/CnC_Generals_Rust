use super::*;

/// C++ Weapon.ini FireFX residual name (Regular veterancy slot).
pub fn host_fire_fx_for_weapon_name(name: &str) -> String {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store.find_weapon_template(name).and_then(|wt| {
            wt.fire_fx[0]
                .as_ref()
                .map(|fx| {
                    let n = fx.name().trim();
                    n.to_string()
                })
                .filter(|s| !s.is_empty())
        })
    })
    .ok()
    .flatten();
    if let Some(s) = from_store {
        return s;
    }
    seed_fire_fx_for(name)
}

/// C++ Weapon.ini ProjectileDetonationFX residual name (Regular slot).
pub fn host_detonation_fx_for_weapon_name(name: &str) -> String {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store.find_weapon_template(name).and_then(|wt| {
            wt.projectile_detonate_fx[0]
                .as_ref()
                .map(|fx| {
                    let n = fx.name().trim();
                    n.to_string()
                })
                .filter(|s| !s.is_empty())
        })
    })
    .ok()
    .flatten();
    if let Some(s) = from_store {
        return s;
    }
    seed_detonation_fx_for(name)
}

///
/// Fail-closed: name residual only; not full drawable bone matrix lookup.
pub fn host_laser_bone_name_for_weapon_name(name: &str) -> String {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store.find_weapon_template(name).and_then(|wt| {
            let n = wt.laser_bone_name.trim();
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
    seed_laser_bone_name_for(name)
}

/// Resolve LaserBoneName for a host unit weapon slot.
pub fn host_laser_bone_name_for_unit_slot(
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
    wname
        .map(host_laser_bone_name_for_weapon_name)
        .unwrap_or_default()
}

pub(super) fn seed_laser_bone_name_for(name: &str) -> String {
    let n = name.to_ascii_lowercase();
    // Only meaningful when LaserName peels non-empty.
    if seed_laser_name_for(name).is_empty() {
        return String::new();
    }
    if n.contains("microwave") {
        return "WEAPON02".into();
    }
    if n.contains("ecm") || n.contains("frequencyjammer") || n.contains("jammer") {
        return "WEAPONA01".into();
    }
    if n.contains("avenger") && n.contains("target") {
        return "TurretFX03".into();
    }
    if n.contains("avenger") && (n.contains("pointdefense") || n.contains("pdl")) {
        return "LazerSpot01".into();
    }
    if n.contains("avenger") {
        return "TurretFX01".into();
    }
    if n.contains("lazr") {
        if n.contains("patriot") {
            return "WEAPONA01".into();
        }
        return "TurretMS01".into();
    }
    if n.contains("pointdefense")
        || n.contains("point_defense")
        || n.contains("pdl")
        || n.contains("paladin")
    {
        return "LASER".into();
    }
    if n.contains("supw") {
        return "MUZZLE01".into();
    }
    if n.contains("airf") {
        return "WeaponA01".into();
    }
    // Generic laser weapon residual bone.
    "LASER".into()
}

/// C++ Weapon.ini LaserName residual (Regular) — laser drawable / beam template.
///
/// Fail-closed: name + host residual beam spawn only; not full ThingFactory
/// laser object / LaserUpdate bone attach matrix.
pub fn host_laser_name_for_weapon_name(name: &str) -> String {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store.find_weapon_template(name).and_then(|wt| {
            let n = wt.laser_name.trim();
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
    seed_laser_name_for(name)
}

/// Resolve LaserName for a host unit weapon slot.
pub fn host_laser_name_for_unit_slot(
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
    wname
        .map(host_laser_name_for_weapon_name)
        .unwrap_or_default()
}

pub(super) fn seed_laser_name_for(name: &str) -> String {
    let n = name.to_ascii_lowercase();
    if n.contains("microwave") {
        return "MicrowaveDisableStream".into();
    }
    if n.contains("ecm") || n.contains("frequencyjammer") || n.contains("jammer") {
        return "ECMDisableStream".into();
    }
    if n.contains("avenger") && n.contains("target") {
        return "AvengerTargetingLaserBeam".into();
    }
    if n.contains("avenger")
        && (n.contains("pointdefense") || n.contains("point_defense") || n.contains("pdl"))
    {
        return "AvengerPointDefenseLaserBeam".into();
    }
    if n.contains("avenger") {
        return "AvengerLaserBeam".into();
    }
    if n.contains("lazr") && n.contains("crusader") {
        return "Lazr_CrusaderLaserBeam".into();
    }
    if n.contains("lazr") && n.contains("patriot") {
        return "Lazr_PatriotLaserBeam".into();
    }
    if n.contains("lazr") && (n.contains("paladin") || n.contains("tank")) {
        return "Lazr_PaladinLaserBeam".into();
    }
    if n.contains("airf") && n.contains("pointdefense") {
        return "AirF_PointDefenseLaserBeam".into();
    }
    if n.contains("supw") && n.contains("pointdefense") {
        return "SupW_PointDefenseDroneLaserBeam".into();
    }
    if n.contains("pointdefense") || n.contains("point_defense") || n.contains("pdl") {
        return "PointDefenseLaserBeam".into();
    }
    if n.contains("paladin") && n.contains("laser") {
        return "PointDefenseLaserBeam".into();
    }
    String::new()
}

/// C++ Weapon.ini ProjectileExhaust residual particle-system name (Regular).
///
/// Fail-closed: name residual for in-flight trail — not full client PSys attach
/// / VeterancyProjectileExhaust HEROIC matrix.
pub fn host_projectile_exhaust_for_weapon_name(name: &str) -> String {
    seed_projectile_exhaust_for(name)
}

/// Resolve ProjectileExhaust for a host unit weapon slot.
pub fn host_projectile_exhaust_for_unit_slot(
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
    wname
        .map(host_projectile_exhaust_for_weapon_name)
        .unwrap_or_default()
}

pub(super) fn seed_projectile_exhaust_for(name: &str) -> String {
    let n = name.to_ascii_lowercase();
    // Hitscan / beam / gun residual: no projectile exhaust.
    if n.contains("laser")
        || n.contains("machinegun")
        || n.contains("chaingun")
        || n.contains("gattling")
        || n.contains("minigun")
        || n.contains("flashbang")
        || n.contains("combatrifle")
        || n.contains("tankgun")
        || (n.contains("cannon") && !n.contains("nuke"))
    {
        return String::new();
    }
    if n.contains("tow") {
        return "TowMissileExhaust".into();
    }
    if n.contains("neutron") {
        return "NeutronMissileExhaust".into();
    }
    if n.contains("scud") {
        return "ScudMissileExhaust".into();
    }
    if n.contains("comanche") {
        return "MissileExhaust".into();
    }
    if n.contains("missiledefender")
        || n.contains("missile_defender")
        || (n.contains("humvee") && n.contains("missile"))
    {
        return "MissileDefenderMissileExhaust".into();
    }
    if n.contains("stinger") || n.contains("patriot") {
        return "MissileExhaust".into();
    }
    if n.contains("tankhunter") || n.contains("rpg") || n.contains("tunneldefender") {
        return "MissileExhaust".into();
    }
    if n.contains("missile") || n.contains("rocket") || n.contains("tomahawk") {
        return "MissileExhaust".into();
    }
    String::new()
}

/// C++ Weapon.ini FireOCL residual name (Regular slot).
///
/// Fail-closed: name residual only — not full ObjectCreationList create_at_position
/// / nugget spawn parity.
pub fn host_fire_ocl_for_weapon_name(name: &str) -> String {
    seed_fire_ocl_for(name)
}

/// C++ Weapon.ini ProjectileDetonationOCL residual name (Regular slot).
///
/// Fail-closed: name residual only — not full OCL object spawn at impact.
pub fn host_detonation_ocl_for_weapon_name(name: &str) -> String {
    seed_detonation_ocl_for(name)
}

/// Resolve FireOCL + ProjectileDetonationOCL for a host unit weapon slot.
pub fn host_weapon_ocl_for_unit_slot(
    template_name: &str,
    primary_weapon_name: Option<&str>,
    secondary_weapon_name: Option<&str>,
    slot: u8,
) -> (String, String) {
    let wname = if slot == 1 {
        secondary_weapon_name
            .or_else(|| secondary_weapon_name_for_unit(template_name))
            .or(primary_weapon_name)
            .or_else(|| primary_weapon_name_for_unit(template_name))
    } else {
        primary_weapon_name.or_else(|| primary_weapon_name_for_unit(template_name))
    };
    match wname {
        Some(n) => (
            host_fire_ocl_for_weapon_name(n),
            host_detonation_ocl_for_weapon_name(n),
        ),
        None => (String::new(), String::new()),
    }
}

pub(super) fn seed_fire_ocl_for(name: &str) -> String {
    let n = name.to_ascii_lowercase();
    // Retail Weapon.ini FireOCL peels (common ZH combat / death weapons).
    if n.contains("anthraxbomb") && n.contains("gamma") {
        return "OCL_PoisonFieldAnthraxGammaBomb".into();
    }
    if n.contains("anthraxbomb") {
        return "OCL_PoisonFieldAnthraxBomb".into();
    }
    if n.contains("scudstorm") || (n.contains("scud") && n.contains("damage")) {
        if n.contains("upgrade") {
            return "OCL_PoisonFieldUpgradedLarge".into();
        }
        return "OCL_PoisonFieldLarge".into();
    }
    if n.contains("dirtynuke") {
        return "OCL_DirtyNuke".into();
    }
    if n.contains("blacknapalm") && n.contains("bomb") {
        return "OCL_BlackNapalmFirestormSmall".into();
    }
    if n.contains("napalmbomb") || (n.contains("napalm") && n.contains("bomb")) {
        return "OCL_FirestormSmall".into();
    }
    if n.contains("mig") && n.contains("firestorm") {
        return "OCL_MiGFirestorm".into();
    }
    if n.contains("nucleartank") || (n.contains("nuclear") && n.contains("death")) {
        return "OCL_RadiationFieldSmall".into();
    }
    if n.contains("nukecannon") || (n.contains("nuclear") && n.contains("cannon")) {
        return "OCL_RadiationFieldMedium".into();
    }
    if n.contains("demotrap") || n.contains("terrorist") || n.contains("carbomb") {
        if n.contains("gamma") {
            return "OCL_PoisonFieldGammaSmall".into();
        }
        if n.contains("anthrax") || n.contains("upgrade") || n.contains("beta") {
            return "OCL_PoisonFieldUpgradedSmall".into();
        }
        // Standard / chem suicide residual peels to small poison field.
        if n.contains("toxin")
            || n.contains("poison")
            || n.contains("chem")
            || n.contains("suicide")
            || n.contains("terrorist")
            || n.contains("demotrap")
            || n.contains("carbomb")
        {
            return "OCL_PoisonFieldSmall".into();
        }
    }
    if n.contains("toxin") || n.contains("contaminat") {
        if n.contains("gamma") {
            return "OCL_PoisonFieldGammaMedium".into();
        }
        if n.contains("upgrade") || n.contains("anthrax") || n.contains("beta") {
            return "OCL_PoisonFieldUpgradedMedium".into();
        }
        return "OCL_PoisonFieldMedium".into();
    }
    if n.contains("inferno") {
        if n.contains("black") || n.contains("upgrade") {
            return "OCL_FireFieldUpgradedSmall".into();
        }
        return "OCL_FireFieldSmall".into();
    }
    String::new()
}

pub(super) fn seed_detonation_ocl_for(name: &str) -> String {
    let n = name.to_ascii_lowercase();
    // Retail Weapon.ini ProjectileDetonationOCL peels.
    if n.contains("firewall") {
        if n.contains("upgrade") || n.contains("black") {
            return "OCL_FireWallSegmentUpgraded".into();
        }
        return "OCL_FireWallSegment".into();
    }
    if n.contains("nukecannon") || (n.contains("nuclear") && n.contains("shell")) {
        if n.contains("medium") {
            return "OCL_RadiationFieldMedium".into();
        }
        return "OCL_RadiationFieldSmall".into();
    }
    if n.contains("inferno") {
        if n.contains("black") || n.contains("upgrade") {
            return "OCL_FireFieldUpgradedSmall".into();
        }
        return "OCL_FireFieldSmall".into();
    }
    if n.contains("toxin") || n.contains("scud") || n.contains("poison") || n.contains("anthrax") {
        if n.contains("gamma") {
            return "OCL_PoisonFieldGammaMedium".into();
        }
        if n.contains("upgrade") || n.contains("beta") || n.contains("anthrax") {
            return "OCL_PoisonFieldUpgradedMedium".into();
        }
        return "OCL_PoisonFieldMedium".into();
    }
    String::new()
}

/// Resolve FireFX + DetonationFX for a host unit firing a weapon slot.
pub fn host_weapon_fx_for_unit_slot(
    template_name: &str,
    primary_weapon_name: Option<&str>,
    secondary_weapon_name: Option<&str>,
    slot: u8,
) -> (String, String) {
    let wname = if slot == 1 {
        secondary_weapon_name
            .or_else(|| secondary_weapon_name_for_unit(template_name))
            .or(primary_weapon_name)
            .or_else(|| primary_weapon_name_for_unit(template_name))
    } else {
        primary_weapon_name.or_else(|| primary_weapon_name_for_unit(template_name))
    };
    match wname {
        Some(n) => (
            host_fire_fx_for_weapon_name(n),
            host_detonation_fx_for_weapon_name(n),
        ),
        None => (String::new(), String::new()),
    }
}

pub(super) fn seed_fire_fx_for(name: &str) -> String {
    let n = name.to_ascii_lowercase();
    if n.contains("laser") || n.contains("pointdefense") {
        return "WeaponFX_PaladinPointDefenseLaser".into();
    }
    if n.contains("tankgun")
        || n.contains("crusader")
        || n.contains("paladin")
        || n.contains("battlemaster")
        || n.contains("scorpion")
        || n.contains("marauder")
    {
        return "WeaponFX_GenericTankGunNoTracer".into();
    }
    if n.contains("ranger") && n.contains("flash") {
        return "WeaponFX_RangerFlashBang".into();
    }
    if n.contains("machinegun")
        || n.contains("combatrifle")
        || n.contains("ranger")
        || n.contains("redguard")
        || n.contains("rebel")
    {
        return "WeaponFX_GenericMachineGunFire".into();
    }
    if n.contains("missile")
        || n.contains("stinger")
        || n.contains("tomahawk")
        || n.contains("rpg")
        || n.contains("tankhunter")
    {
        return "WeaponFX_GenericMissileLaunch".into();
    }
    if n.contains("flame") || n.contains("dragon") || n.contains("inferno") {
        return "WeaponFX_DragonTankFlameWeapon".into();
    }
    if n.contains("gattling") || n.contains("minigun") {
        return "WeaponFX_GattlingTankGun".into();
    }
    if n.contains("nuke") {
        return "WeaponFX_NukeCannonMuzzleFlash".into();
    }
    if n.contains("aurora") || n.contains("bomb") {
        return "WeaponFX_AuroraBomb".into();
    }
    if n.contains("patriot") {
        return "WeaponFX_PatriotBattery".into();
    }
    String::new()
}

pub(super) fn seed_detonation_fx_for(name: &str) -> String {
    let n = name.to_ascii_lowercase();
    if n.contains("jet") || n.contains("raptor") || n.contains("mig") || n.contains("stealth") {
        return "WeaponFX_JetMissileDetonation".into();
    }
    if n.contains("rpg")
        || n.contains("buggy")
        || n.contains("tankhunter")
        || n.contains("tomahawk")
    {
        return "WeaponFX_RocketBuggyMissileDetonation".into();
    }
    if n.contains("scud") {
        return "WeaponFX_ScudLauncherDetonation".into();
    }
    if n.contains("nuke") || n.contains("neutron") {
        return "WeaponFX_NukeCannon".into();
    }
    if n.contains("aurora") || n.contains("bomb") {
        return "WeaponFX_AuroraBombDetonation".into();
    }
    if n.contains("missile") || n.contains("stinger") || n.contains("patriot") {
        return "WeaponFX_GenericMissileDetonation".into();
    }
    String::new()
}
