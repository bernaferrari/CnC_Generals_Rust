use super::*;

/// Convert the host world's rank enum into the canonical GameLogic enum used
/// by the WeaponStore. Keep this exhaustive instead of casting ordinal values.
fn game_logic_veterancy(
    level: crate::game_logic::VeterancyLevel,
) -> gamelogic::common::VeterancyLevel {
    match level {
        crate::game_logic::VeterancyLevel::Rookie => gamelogic::common::VeterancyLevel::Regular,
        crate::game_logic::VeterancyLevel::Veteran => gamelogic::common::VeterancyLevel::Veteran,
        crate::game_logic::VeterancyLevel::Elite => gamelogic::common::VeterancyLevel::Elite,
        crate::game_logic::VeterancyLevel::Heroic => gamelogic::common::VeterancyLevel::Heroic,
    }
}

/// C++ Weapon.ini `FireFX` for the firing object's actual veterancy level.
///
/// This intentionally has no name-based fallback: an unloaded or absent
/// Weapon.ini reference is no effect, matching C++'s null `FXList*` rather
/// than a guessed visual.
pub fn host_fire_fx_for_weapon_name_at_veterancy(
    name: &str,
    level: crate::game_logic::VeterancyLevel,
) -> String {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let level = game_logic_veterancy(level);
    with_weapon_store(|store| {
        store.find_weapon_template(name).and_then(|wt| {
            wt.get_fire_fx(level)
                .map(|fx| fx.name().trim().to_string())
                .filter(|name| !name.is_empty())
        })
    })
    .ok()
    .flatten()
    .unwrap_or_default()
}

/// C++ Weapon.ini `FireFX` for the regular (non-veteran) slot.
pub fn host_fire_fx_for_weapon_name(name: &str) -> String {
    host_fire_fx_for_weapon_name_at_veterancy(name, crate::game_logic::VeterancyLevel::Rookie)
}

/// C++ Weapon.ini `ProjectileDetonationFX` for the firing object's actual
/// veterancy level. Missing data stays absent rather than using a name guess.
pub fn host_detonation_fx_for_weapon_name_at_veterancy(
    name: &str,
    level: crate::game_logic::VeterancyLevel,
) -> String {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let level = game_logic_veterancy(level);
    with_weapon_store(|store| {
        store.find_weapon_template(name).and_then(|wt| {
            wt.get_projectile_detonate_fx(level)
                .map(|fx| fx.name().trim().to_string())
                .filter(|name| !name.is_empty())
        })
    })
    .ok()
    .flatten()
    .unwrap_or_default()
}

/// C++ Weapon.ini `ProjectileDetonationFX` for the regular slot.
pub fn host_detonation_fx_for_weapon_name(name: &str) -> String {
    host_detonation_fx_for_weapon_name_at_veterancy(name, crate::game_logic::VeterancyLevel::Rookie)
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

/// C++ Weapon.ini `ProjectileExhaust` for the firing object's actual
/// veterancy level. The name is retained separately from the resolved particle
/// template, so an unavailable client particle definition never turns into a
/// guessed generic exhaust.
pub fn host_projectile_exhaust_for_weapon_name_at_veterancy(
    name: &str,
    level: crate::game_logic::VeterancyLevel,
) -> String {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let level = game_logic_veterancy(level);
    with_weapon_store(|store| {
        store.find_weapon_template(name).and_then(|wt| {
            wt.get_projectile_exhaust_name(level)
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
        })
    })
    .ok()
    .flatten()
    .unwrap_or_default()
}

/// C++ Weapon.ini `ProjectileExhaust` for the regular slot.
pub fn host_projectile_exhaust_for_weapon_name(name: &str) -> String {
    host_projectile_exhaust_for_weapon_name_at_veterancy(
        name,
        crate::game_logic::VeterancyLevel::Rookie,
    )
}

/// Resolve ProjectileExhaust for a host unit weapon slot.
pub fn host_projectile_exhaust_for_unit_slot(
    template_name: &str,
    primary_weapon_name: Option<&str>,
    secondary_weapon_name: Option<&str>,
    slot: u8,
) -> String {
    host_projectile_exhaust_for_unit_slot_at_veterancy(
        template_name,
        primary_weapon_name,
        secondary_weapon_name,
        slot,
        crate::game_logic::VeterancyLevel::Rookie,
    )
}

/// Resolve `ProjectileExhaust` for a host unit weapon slot at its actual
/// veterancy level.
pub fn host_projectile_exhaust_for_unit_slot_at_veterancy(
    template_name: &str,
    primary_weapon_name: Option<&str>,
    secondary_weapon_name: Option<&str>,
    slot: u8,
    level: crate::game_logic::VeterancyLevel,
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
        .map(|name| host_projectile_exhaust_for_weapon_name_at_veterancy(name, level))
        .unwrap_or_default()
}

/// C++ Weapon.ini `FireOCL` for the firing object's actual veterancy level.
/// The value is a parsed reference, not a category-derived OCL guess.
pub fn host_fire_ocl_for_weapon_name_at_veterancy(
    name: &str,
    level: crate::game_logic::VeterancyLevel,
) -> String {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let level = game_logic_veterancy(level);
    with_weapon_store(|store| {
        store.find_weapon_template(name).and_then(|wt| {
            wt.get_fire_ocl_name(level)
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
        })
    })
    .ok()
    .flatten()
    .unwrap_or_default()
}

/// C++ Weapon.ini `FireOCL` for the regular slot.
pub fn host_fire_ocl_for_weapon_name(name: &str) -> String {
    host_fire_ocl_for_weapon_name_at_veterancy(name, crate::game_logic::VeterancyLevel::Rookie)
}

/// C++ Weapon.ini `ProjectileDetonationOCL` for the firing object's actual
/// veterancy level. Missing/unloaded data remains absent.
pub fn host_detonation_ocl_for_weapon_name_at_veterancy(
    name: &str,
    level: crate::game_logic::VeterancyLevel,
) -> String {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let level = game_logic_veterancy(level);
    with_weapon_store(|store| {
        store.find_weapon_template(name).and_then(|wt| {
            wt.get_projectile_detonation_ocl_name(level)
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
        })
    })
    .ok()
    .flatten()
    .unwrap_or_default()
}

/// C++ Weapon.ini `ProjectileDetonationOCL` for the regular slot.
pub fn host_detonation_ocl_for_weapon_name(name: &str) -> String {
    host_detonation_ocl_for_weapon_name_at_veterancy(
        name,
        crate::game_logic::VeterancyLevel::Rookie,
    )
}

/// Resolve FireOCL + ProjectileDetonationOCL for a host unit weapon slot.
pub fn host_weapon_ocl_for_unit_slot(
    template_name: &str,
    primary_weapon_name: Option<&str>,
    secondary_weapon_name: Option<&str>,
    slot: u8,
) -> (String, String) {
    host_weapon_ocl_for_unit_slot_at_veterancy(
        template_name,
        primary_weapon_name,
        secondary_weapon_name,
        slot,
        crate::game_logic::VeterancyLevel::Rookie,
    )
}

/// Resolve FireOCL + ProjectileDetonationOCL for a host unit weapon slot at
/// the firing object's actual veterancy level.
pub fn host_weapon_ocl_for_unit_slot_at_veterancy(
    template_name: &str,
    primary_weapon_name: Option<&str>,
    secondary_weapon_name: Option<&str>,
    slot: u8,
    level: crate::game_logic::VeterancyLevel,
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
            host_fire_ocl_for_weapon_name_at_veterancy(n, level),
            host_detonation_ocl_for_weapon_name_at_veterancy(n, level),
        ),
        None => (String::new(), String::new()),
    }
}

/// Resolve FireFX + DetonationFX for a host unit firing a weapon slot.
pub fn host_weapon_fx_for_unit_slot(
    template_name: &str,
    primary_weapon_name: Option<&str>,
    secondary_weapon_name: Option<&str>,
    slot: u8,
) -> (String, String) {
    host_weapon_fx_for_unit_slot_at_veterancy(
        template_name,
        primary_weapon_name,
        secondary_weapon_name,
        slot,
        crate::game_logic::VeterancyLevel::Rookie,
    )
}

/// Resolve FireFX + ProjectileDetonationFX for a host unit weapon slot at the
/// firing object's actual veterancy level.
pub fn host_weapon_fx_for_unit_slot_at_veterancy(
    template_name: &str,
    primary_weapon_name: Option<&str>,
    secondary_weapon_name: Option<&str>,
    slot: u8,
    level: crate::game_logic::VeterancyLevel,
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
            host_fire_fx_for_weapon_name_at_veterancy(n, level),
            host_detonation_fx_for_weapon_name_at_veterancy(n, level),
        ),
        None => (String::new(), String::new()),
    }
}
