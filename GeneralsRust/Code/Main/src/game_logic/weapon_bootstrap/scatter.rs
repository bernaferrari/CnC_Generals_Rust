use super::*;

/// C++ Weapon.ini ScatterRadius residual (base).
pub fn host_scatter_radius_for_weapon_name(name: &str) -> f32 {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store
            .find_weapon_template(name)
            .map(|wt| wt.scatter_radius.max(0.0))
    })
    .ok()
    .flatten();
    if let Some(v) = from_store {
        if v > 0.0 {
            return v;
        }
    }
    seed_scatter_radius_for(name)
}

/// C++ Weapon.ini ScatterRadiusVsInfantry residual (added for infantry targets).
pub fn host_scatter_radius_vs_infantry_for_weapon_name(name: &str) -> f32 {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store.find_weapon_template(name).map(|wt| {
            // field may be infantry_inaccuracy_dist on template
            wt.infantry_inaccuracy_dist.max(0.0)
        })
    })
    .ok()
    .flatten();
    if let Some(v) = from_store {
        if v > 0.0 {
            return v;
        }
    }
    seed_scatter_radius_vs_infantry_for(name)
}

/// Effective scatter radius residual (C++ effective_scatter_radius).
pub fn host_effective_scatter_radius(name: &str, target_is_infantry: bool) -> f32 {
    let base = host_scatter_radius_for_weapon_name(name);
    if !target_is_infantry {
        return base;
    }
    let vs = host_scatter_radius_vs_infantry_for_weapon_name(name);
    if vs > 0.0 {
        base + vs
    } else {
        base
    }
}

pub(super) fn seed_scatter_radius_for(name: &str) -> f32 {
    let n = name.to_ascii_lowercase();
    // Most combat shells use ScatterRadius 0 + ScatterRadiusVsInfantry only.
    if n.contains("scudstorm") && n.contains("weapon") && !n.contains("damage") {
        return 0.0;
    }
    // Flashbang residual ScatterRadius **4** (applies to all targets).
    if n.contains("flashbang") || n.contains("flash_bang") {
        return 4.0;
    }
    // Strategy Center artillery residual ScatterRadius **15**.
    if n.contains("strategycenter") || n.contains("strategy_center") {
        return 15.0;
    }
    0.0
}

pub(super) fn seed_scatter_radius_vs_infantry_for(name: &str) -> f32 {
    let n = name.to_ascii_lowercase();
    if n.contains("laser") || n.contains("machinegun") || n.contains("minigun") {
        return 0.0;
    }
    // Fire Base howitzer residual ScatterRadiusVsInfantry **15**.
    if n.contains("firebase") || (n.contains("howitzer") && !n.contains("neutron")) {
        return 15.0;
    }
    // Strategy Center gun residual ScatterRadiusVsInfantry **15**.
    if n.contains("strategycenter") || n.contains("strategy_center") {
        return 15.0;
    }
    // SupW EMPPatriotEffectSpheroid residual ScatterRadiusVsInfantry **10**.
    if n.contains("empblast") || n.contains("emppatriot") {
        return 10.0;
    }
    // Neutron shell residual ScatterRadiusVsInfantry **10** (before NukeCannon primary 30).
    if n.contains("neutron") {
        return 10.0;
    }
    // Nuke Cannon residual ScatterRadiusVsInfantry **30**.
    if n.contains("nukecannon") || n.contains("nuke_cannon") {
        return 30.0;
    }
    // SCUD Launcher residual ScatterRadiusVsInfantry **30** (not ScudStorm).
    if (n.contains("scudlauncher") || n.contains("scud_launcher") || n.contains("scudmissile"))
        && !n.contains("scudstorm")
    {
        return 30.0;
    }
    // Inferno Cannon residual ScatterRadiusVsInfantry **30**.
    if n.contains("infernocannon") || n.contains("inferno_cannon") || n.contains("infernotank") {
        return 30.0;
    }
    // Tomahawk residual ScatterRadiusVsInfantry **20** (not the common 10).
    if n.contains("tomahawk") {
        return 20.0;
    }
    // Rocket Buggy residual ScatterRadiusVsInfantry **20** (BuggyRocketWeapon).
    if n.contains("buggyrocket") || n.contains("rocketbuggy") || n.contains("buggy_rocket") {
        return 20.0;
    }
    // Common ZH residual: tank/missile shells vs infantry inaccuracy ~10.
    if n.contains("tankgun")
        || n.contains("tankshell")
        || n.contains("crusader")
        || n.contains("paladin")
        || n.contains("battlemaster")
        || n.contains("scorpion")
        || n.contains("marauder")
        || n.contains("overlord")
        || n.contains("raptor")
        || n.contains("missile")
        || n.contains("rpg")
        || n.contains("tankhunter")
        || n.contains("tunneldefender")
        || n.contains("rocketweapon")
        || n.contains("technical")
        || n.contains("humvee")
        || n.contains("hellfire")
        || n.contains("stinger")
    {
        return 10.0;
    }
    0.0
}

/// Deterministic scatter offset residual (C++ random radius + angle).
///
/// Uses pure ADC stream seeded by shooter^target so re-fire re-query is stable
/// within the same pairing (fail-closed vs live GameLogic global stream order).
/// Default geometry hit radius residual when object selection radius is unset.
pub const DEFAULT_SCATTER_HIT_RADIUS: f32 = 5.0;

/// C++ ScatterRadius residual for instant-hit weapons (no projectile flight).
///
/// Aim is offset by `scatter_aim_offset`; if the 2D offset exceeds the target's
/// hit radius the intended target is missed (damage skipped / applied at ground).
pub fn scatter_misses_intended_target(
    scatter_radius: f32,
    seed: u32,
    target_hit_radius: f32,
) -> bool {
    if scatter_radius <= 0.0 {
        return false;
    }
    let offset = scatter_aim_offset(seed, scatter_radius);
    let miss_dist = (offset.x * offset.x + offset.z * offset.z).sqrt();
    let hit_r = target_hit_radius.max(1.0);
    miss_dist > hit_r
}

/// Deterministic scatter seed residual from shooter/target/frame.
pub fn scatter_seed_for_shot(shooter: u32, target: u32, frame: u32) -> u32 {
    shooter
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add(target.wrapping_mul(0x85EB_CA6B))
        .wrapping_add(frame.wrapping_mul(0xC2B2_AE35))
}
/// 2D scatter aim offset residual (Y cleared — ground-plane miss).
pub fn scatter_impact_offset(seed: u32, scatter_radius: f32) -> glam::Vec3 {
    let mut o = scatter_aim_offset(seed, scatter_radius);
    o.y = 0.0;
    o
}

pub fn scatter_aim_offset(seed: u32, scatter_radius: f32) -> glam::Vec3 {
    use crate::game_logic::host_rng_residual::pure_logic_random_real;
    if scatter_radius <= 0.0 {
        return glam::Vec3::ZERO;
    }
    let r = pure_logic_random_real(seed, 0, 0.0, scatter_radius);
    let ang = pure_logic_random_real(seed, 1, 0.0, std::f32::consts::TAU);
    // Gameplay XZ plane residual (Y up).
    glam::Vec3::new(r * ang.cos(), 0.0, r * ang.sin())
}

