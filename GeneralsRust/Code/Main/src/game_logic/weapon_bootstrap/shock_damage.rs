use super::*;

/// C++ Weapon.ini ShockWaveAmount residual (Regular).
///
/// Fail-closed: impulse residual on splash victims only — not full PhysicsBehavior
/// / ground-scrape matrix.
/// C++ Object::attemptDamage shockwave force residual (Object.cpp:1811-1822).
///
/// `source` is the FIRER (`victimPos - sourcePos` in Weapon.cpp:1385-1430),
/// not the blast epicenter. Distance saturates at `radius` so a Tomahawk
/// detonating hundreds of units from the launcher still applies
/// `ShockWaveTaperOff` (retail 0.75) at ground zero.
///
/// Returns None only when amount/radius are empty. Damage-ring iteration
/// (not this helper) decides who is eligible.
pub fn compute_shock_wave_force(
    source: glam::Vec3,
    victim_pos: glam::Vec3,
    amount: f32,
    radius: f32,
    taper_off: f32,
) -> Option<glam::Vec3> {
    if amount <= 0.0 || radius <= 0.0 {
        return None;
    }
    let v = victim_pos - source;
    let dist = v.length();
    if dist < 1e-4 {
        // C++ zero-vector guard: straight up.
        return Some(glam::Vec3::new(0.0, amount, 0.0));
    }
    let distance_from_center = (dist / radius).min(1.0);
    let taper = taper_off.clamp(0.0, 1.0);
    let distance_taper = distance_from_center * (1.0 - taper);
    let shock_taper_mult = 1.0 - distance_taper;
    let dir = v / dist;
    let mut force = dir * (amount * shock_taper_mult);
    // C++: z (host Y) = length of the scaled vector for dramatic lift.
    force.y = force.length();
    Some(force)
}

pub fn host_shock_wave_amount_for_weapon_name(name: &str) -> f32 {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store
            .find_weapon_template(name)
            .map(|wt| wt.shock_wave_amount.max(0.0))
    })
    .ok()
    .flatten();
    if let Some(v) = from_store {
        if v > 0.0 {
            return v;
        }
    }
    seed_shock_wave_amount_for(name)
}

/// C++ Weapon.ini ShockWaveRadius residual (Regular).
pub fn host_shock_wave_radius_for_weapon_name(name: &str) -> f32 {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store
            .find_weapon_template(name)
            .map(|wt| wt.shock_wave_radius.max(0.0))
    })
    .ok()
    .flatten();
    if let Some(v) = from_store {
        if v > 0.0 {
            return v;
        }
    }
    seed_shock_wave_radius_for(name)
}

/// C++ Weapon.ini ShockWaveTaperOff residual (Regular).
pub fn host_shock_wave_taper_for_weapon_name(name: &str) -> f32 {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store
            .find_weapon_template(name)
            .map(|wt| wt.shock_wave_taper_off.max(0.0))
    })
    .ok()
    .flatten();
    if let Some(v) = from_store {
        if v > 0.0 {
            return v;
        }
    }
    seed_shock_wave_taper_for(name)
}

pub(super) fn seed_shock_wave_amount_for(name: &str) -> f32 {
    let n = name.to_ascii_lowercase();
    if n.contains("moab") {
        return 250.0;
    }
    if n.contains("nuke") || n.contains("nuclear") {
        return 150.0;
    }
    if n.contains("scudstorm") || (n.contains("scud") && n.contains("damage")) {
        return 75.0;
    }
    if n.contains("demotrap")
        || n.contains("terrorist")
        || n.contains("suicide")
        || n.contains("carbomb")
    {
        return 50.0;
    }
    if n.contains("tomahawk") {
        return 50.0;
    }
    if n.contains("inferno") || n.contains("napalm") {
        return 30.0;
    }
    if n.contains("tankgun") || n.contains("cannon") {
        return 10.0;
    }
    0.0
}

pub(super) fn seed_shock_wave_radius_for(name: &str) -> f32 {
    let n = name.to_ascii_lowercase();
    if n.contains("moab") {
        return 100.0;
    }
    if n.contains("nuke") || n.contains("nuclear") {
        return 80.0;
    }
    if n.contains("scudstorm") || (n.contains("scud") && n.contains("damage")) {
        return 50.0;
    }
    if n.contains("demotrap")
        || n.contains("terrorist")
        || n.contains("suicide")
        || n.contains("carbomb")
    {
        return 40.0;
    }
    if n.contains("tomahawk") {
        return 40.0;
    }
    if n.contains("inferno") || n.contains("napalm") {
        return 30.0;
    }
    if n.contains("tankgun") || n.contains("cannon") {
        return 15.0;
    }
    0.0
}

pub(super) fn seed_shock_wave_taper_for(name: &str) -> f32 {
    // Retail-ish default taper when amount peels non-empty.
    if seed_shock_wave_amount_for(name) > 0.0 {
        return 0.75;
    }
    0.0
}

/// C++ Weapon.ini SecondaryDamage residual amount (Regular).
///
/// Fail-closed: name peel / store lookup only — applied as outer splash ring
/// on projectile impact (not full Affects mask / shockwave matrix).
pub fn host_secondary_damage_for_weapon_name(name: &str) -> f32 {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store
            .find_weapon_template(name)
            .map(|wt| wt.secondary_damage.max(0.0))
    })
    .ok()
    .flatten();
    if let Some(v) = from_store {
        if v > 0.0 {
            return v;
        }
    }
    seed_secondary_damage_for(name)
}

/// C++ Weapon.ini SecondaryDamageRadius residual (Regular).
/// C++ Weapon.ini PrimaryDamageRadius residual.
pub fn host_primary_damage_radius_for_weapon_name(name: &str) -> f32 {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store
            .find_weapon_template(name)
            .map(|wt| wt.primary_damage_radius.max(0.0))
    })
    .ok()
    .flatten();
    if let Some(v) = from_store {
        if v > 0.0 {
            return v;
        }
    }
    seed_primary_damage_radius_for(name)
}

pub(super) fn seed_primary_damage_radius_for(name: &str) -> f32 {
    let n = name.to_ascii_lowercase();
    if n.contains("howitzer") || n.contains("firebase") || n.contains("artillery") {
        return 25.0;
    }
    if n.contains("scud") || n.contains("nuke") || n.contains("tomahawk") {
        return 30.0;
    }
    if n.contains("bomb") || n.contains("demo") {
        return 20.0;
    }
    0.0
}

pub fn host_secondary_damage_radius_for_weapon_name(name: &str) -> f32 {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store
            .find_weapon_template(name)
            .map(|wt| wt.secondary_damage_radius.max(0.0))
    })
    .ok()
    .flatten();
    if let Some(v) = from_store {
        if v > 0.0 {
            return v;
        }
    }
    seed_secondary_damage_radius_for(name)
}

pub(super) fn seed_secondary_damage_for(name: &str) -> f32 {
    let n = name.to_ascii_lowercase();
    if n.contains("scudstorm") || (n.contains("scud") && n.contains("damage")) {
        return if n.contains("upgrade") { 200.0 } else { 150.0 };
    }
    if n.contains("demotrap") || n.contains("terrorist") || n.contains("suicide") {
        return 50.0;
    }
    if n.contains("napalm") && n.contains("bomb") {
        return 25.0;
    }
    if n.contains("inferno") {
        return 10.0;
    }
    if n.contains("scorpion") {
        return 25.0;
    }
    if n.contains("strategycenter") {
        return 10.0;
    }
    0.0
}

pub(super) fn seed_secondary_damage_radius_for(name: &str) -> f32 {
    let n = name.to_ascii_lowercase();
    if n.contains("scudstorm") || (n.contains("scud") && n.contains("damage")) {
        return 200.0;
    }
    if n.contains("demotrap") || n.contains("terrorist") || n.contains("suicide") {
        return 25.0;
    }
    if n.contains("napalm") && n.contains("bomb") {
        return 30.0;
    }
    if n.contains("inferno") {
        return 20.0;
    }
    if n.contains("scorpion") {
        return 20.0;
    }
    if n.contains("strategycenter") {
        return 15.0;
    }
    0.0
}
