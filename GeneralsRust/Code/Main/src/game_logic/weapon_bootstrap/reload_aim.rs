use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostReloadType {
    Auto,
    Manual,
    ReturnToBase,
}

pub fn host_reload_type_for_weapon_name(name: &str) -> HostReloadType {
    use gamelogic::weapon::{WeaponReloadType as GlReload, with_weapon_store};
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store
            .find_weapon_template(name)
            .map(|wt| match wt.reload_type {
                GlReload::AutoReload => HostReloadType::Auto,
                GlReload::ReturnToBaseToReload => HostReloadType::ReturnToBase,
                _ => HostReloadType::Manual,
            })
    })
    .ok()
    .flatten();
    if let Some(t) = from_store {
        return t;
    }
    seed_reload_type_for(name)
}

pub(super) fn seed_reload_type_for(name: &str) -> HostReloadType {
    let n = name.to_ascii_lowercase();
    if n.contains("raptor")
        || (n.contains("mig") && n.contains("missile"))
        || n.contains("jetmissile")
        || n.contains("stealthfighter")
        || (n.contains("aurora") && (n.contains("bomb") || n.contains("weapon")))
    {
        return HostReloadType::ReturnToBase;
    }
    if n.contains("terrorist") || n.contains("carbomb") || n.contains("suicide") {
        return HostReloadType::Manual;
    }
    HostReloadType::Auto
}

/// C++ Weapon.ini PreAttackType residual (WeaponPrefireType).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostPrefireType {
    /// Delay before every shot.
    PerShot,
    /// Delay once per engagement / new target.
    PerAttack,
    /// Delay once at the start of each clip.
    PerClip,
}

impl HostPrefireType {
    pub fn from_ini(s: &str) -> Self {
        match s.trim().to_ascii_uppercase().as_str() {
            "PER_SHOT" => Self::PerShot,
            "PER_CLIP" => Self::PerClip,
            "PER_ATTACK" => Self::PerAttack,
            _ => Self::PerShot, // C++ default PrefirePerShot
        }
    }
}

/// Resolve PreAttackType for a weapon name.
pub fn host_prefire_type_for_weapon_name(name: &str) -> HostPrefireType {
    use gamelogic::weapon::{WeaponPrefireType, with_weapon_store};
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store
            .find_weapon_template(name)
            .map(|wt| match wt.prefire_type {
                WeaponPrefireType::PrefirePerShot => HostPrefireType::PerShot,
                WeaponPrefireType::PrefirePerAttack => HostPrefireType::PerAttack,
                WeaponPrefireType::PrefirePerClip => HostPrefireType::PerClip,
            })
    })
    .ok()
    .flatten();
    if let Some(t) = from_store {
        return t;
    }
    seed_prefire_type_for(name)
}

/// Deterministic DelayBetweenShots yardstick in host seconds (frames / 30).
///
/// C++ stores `m_whenWeCanFireAgain` from a single `getDelayBetweenShots`
/// draw. Live ready-checks recompute `now - last_fire >= interval`, so this
/// must not roll — fire stamps the draw onto `last_fire_time`.
pub fn host_delay_between_shots_secs_nominal(name: &str) -> Option<f32> {
    leftover_delay_between_shots_secs(name, 1.0, false)
}

/// C++ `WeaponTemplate::getDelayBetweenShots` as host seconds (frames / 30).
/// Inclusive GameLogicRandomValue when Min != Max. RATE_OF_FIRE is applied by
/// leftover (`REAL_TO_INT_FLOOR(delay / bonusROF)`).
pub fn host_delay_between_shots_secs(name: &str) -> Option<f32> {
    leftover_delay_between_shots_secs(name, 1.0, true)
}

/// Per-shot leftover `get_delay_between_shots` in host seconds, with RATE_OF_FIRE.
pub fn host_delay_between_shots_secs_with_rof(name: &str, rof: f32) -> Option<f32> {
    leftover_delay_between_shots_secs(name, rof, true)
}

/// Ready-check leftover yardstick: min==max branch + RATE_OF_FIRE floor, no roll.
pub fn host_delay_between_shots_secs_nominal_with_rof(name: &str, rof: f32) -> Option<f32> {
    leftover_delay_between_shots_secs(name, rof, false)
}

/// C++ WeaponTemplate::getDelayBetweenShots frame draw before RATE_OF_FIRE.
/// Inclusive GameLogicRandomValue when Min != Max.
pub fn delay_between_shots_frames(min_frames: i32, max_frames: i32) -> i32 {
    if min_frames == max_frames {
        min_frames
    } else {
        gamelogic::helpers::get_game_logic_random_value(min_frames, max_frames)
    }
}

/// Per-shot DelayBetweenShots in host seconds (frames / 30), rolled when Min != Max.
pub fn host_delay_between_shots_secs_rolled(name: &str) -> Option<f32> {
    host_delay_between_shots_secs(name)
}

fn leftover_delay_between_shots_secs(name: &str, rof: f32, roll: bool) -> Option<f32> {
    use gamelogic::weapon::{WeaponBonus, WeaponBonusField, with_weapon_store};
    let _ = ensure_host_weapon_store();
    with_weapon_store(|store| {
        store.find_weapon_template(name).and_then(|wt| {
            let mut bonus = WeaponBonus::new();
            bonus.set_field(WeaponBonusField::RateOfFire, rof.max(0.01));
            let frames = if roll || wt.min_delay_between_shots == wt.max_delay_between_shots {
                wt.get_delay_between_shots(&bonus)
            } else {
                let mut yardstick = WeaponTemplate::new(wt.name.clone());
                yardstick.min_delay_between_shots = wt.min_delay_between_shots;
                yardstick.max_delay_between_shots = wt.min_delay_between_shots;
                yardstick.get_delay_between_shots(&bonus)
            };
            (frames > 0).then_some(frames as f32 / 30.0)
        })
    })
    .ok()
    .flatten()
}

/// C++ Weapon.ini PrimaryDamage residual amount (Regular).
pub fn host_primary_damage_for_weapon_name(name: &str) -> Option<f32> {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    with_weapon_store(|store| {
        store
            .find_weapon_template(name)
            .map(|wt| wt.primary_damage)
            .filter(|damage| *damage > 0.0)
    })
    .ok()
    .flatten()
}

pub(super) fn seed_prefire_type_for(name: &str) -> HostPrefireType {
    let n = name.to_ascii_lowercase();
    // Gattling / continuous-fire residual often PER_SHOT with short delay.
    if n.contains("gattling") || n.contains("minigun") || n.contains("vulcan") {
        return HostPrefireType::PerShot;
    }
    // Scud / artillery clip residual.
    if n.contains("scudstorm") || (n.contains("howitzer") && n.contains("spectre")) {
        return HostPrefireType::PerClip;
    }
    // Melee / knife / sniper residual: once per attack.
    if n.contains("knife")
        || n.contains("melee")
        || n.contains("burton")
        || n.contains("sniper")
        || n.contains("jarmen")
        || n.contains("pathfinder")
        || n.contains("ranger")
        || n.contains("tunneldefender")
    {
        return HostPrefireType::PerAttack;
    }
    // C++ default is PER_SHOT.
    HostPrefireType::PerShot
}

/// C++ Weapon.ini AcceptableAimDelta residual (radians).
///
/// C++ `INI::parseAngleReal` stores radians. Host peels convert degree-like
/// store values (`> PI`) via `to_radians`. Floor matches AIStates REL_THRESH
/// (~2°) when the peel is smaller.
pub const AIM_DELTA_REL_THRESH_RAD: f32 = 0.035;

/// Normalize a raw AcceptableAimDelta peel into radians with REL_THRESH floor.
pub fn normalize_aim_delta_radians(raw: f32) -> f32 {
    let rad = if raw > std::f32::consts::PI + 0.01 {
        // Degree-like residual from plain f32 INI peels (e.g. 20 / 180).
        raw.to_radians()
    } else {
        raw.max(0.0)
    };
    if rad < AIM_DELTA_REL_THRESH_RAD {
        AIM_DELTA_REL_THRESH_RAD
    } else {
        rad
    }
}

/// Resolve AcceptableAimDelta for a weapon name (radians, REL_THRESH floor).
pub fn host_aim_delta_for_weapon_name(name: &str) -> f32 {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store =
        with_weapon_store(|store| store.find_weapon_template(name).map(|wt| wt.aim_delta))
            .ok()
            .flatten();
    if let Some(v) = from_store {
        // Store default 0 still gets REL_THRESH floor.
        return normalize_aim_delta_radians(v);
    }
    normalize_aim_delta_radians(seed_aim_delta_degrees_for(name))
}

pub(super) fn seed_aim_delta_degrees_for(name: &str) -> f32 {
    let n = name.to_ascii_lowercase();
    // Omni-fire residual (no turn required).
    if n.contains("stinger")
        || n.contains("drone")
        || n.contains("sentry")
        || n.contains("gunship")
        || n.contains("spectre")
        || n.contains("pointDefense")
        || n.contains("pointdefense")
        || (n.contains("laser") && n.contains("defense"))
        || n.contains("scudstorm")
        || n.contains("neutronmine")
        || n.contains("demotrap")
    {
        return 180.0;
    }
    // Aircraft / bomb residual.
    if n.contains("aurora") || n.contains("bomb") {
        return 45.0;
    }
    // Common vehicle guns residual.
    if n.contains("tankgun")
        || n.contains("tankshell")
        || n.contains("crusader")
        || n.contains("paladin")
        || n.contains("battlemaster")
        || n.contains("scorpion")
        || n.contains("marauder")
        || n.contains("overlord")
        || n.contains("humvee")
        || n.contains("technical")
        || n.contains("quadcannon")
        || n.contains("tomahawk")
        || n.contains("inferno")
        || n.contains("howitzer")
        || n.contains("firebase")
    {
        return 20.0;
    }
    // Infantry residual.
    if n.contains("machinegun")
        || n.contains("ranger")
        || n.contains("rebel")
        || n.contains("rpg")
        || n.contains("missile")
        || n.contains("ak47")
        || n.contains("pathfinder")
        || n.contains("jarmen")
        || n.contains("burton")
    {
        return 30.0;
    }
    0.0
}

/// Shortest signed angle difference in radians, range (-PI, PI].
pub fn shortest_angle_delta(from: f32, to: f32) -> f32 {
    let mut d = to - from;
    while d > std::f32::consts::PI {
        d -= std::f32::consts::TAU;
    }
    while d <= -std::f32::consts::PI {
        d += std::f32::consts::TAU;
    }
    d
}

/// C++ PartitionManager::getRelativeAngle2D residual (source facing → target).
///
/// Host XZ plane; orientation is yaw about Y (0 = +X residual matches movement).
pub fn relative_angle_2d(
    source_pos: glam::Vec3,
    source_orientation: f32,
    target_pos: glam::Vec3,
) -> f32 {
    let dx = target_pos.x - source_pos.x;
    let dz = target_pos.z - source_pos.z;
    if dx * dx + dz * dz < 1e-8 {
        return 0.0;
    }
    // Match Object movement orientation: (-dz).atan2(dx).
    let desired = (-dz).atan2(dx);
    shortest_angle_delta(source_orientation, desired)
}

/// True when |relative angle| is within aim_delta (radians).
pub fn is_within_aim_delta(rel_angle: f32, aim_delta_rad: f32) -> bool {
    rel_angle.abs() <= aim_delta_rad.max(AIM_DELTA_REL_THRESH_RAD)
}

/// C++ Weapon.cpp:2645-2662 — next-shot frame is now + firing slot delay.
pub fn shared_next_ready_time(last_fire_time: f32, firing_interval: f32) -> f32 {
    last_fire_time + firing_interval.max(0.0)
}

/// Back-compute `last_fire_time` so the slot becomes ready at `next_ready`.
///
/// Live ready is `now - last_fire >= slot_interval`. Matching C++
/// `setPossibleNextShotFrame(m_whenWeCanFireAgain)` therefore stamps
/// `last_fire = next_ready - slot_interval`.
pub fn last_fire_time_matching_shared_ready(next_ready: f32, slot_interval: f32) -> f32 {
    next_ready - slot_interval.max(0.0)
}
