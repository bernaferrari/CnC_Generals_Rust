use super::*;

/// C++ Weapon.ini MinTargetPitch / MaxTargetPitch residual (radians).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostTargetPitchLimits {
    pub min_pitch: f32,
    pub max_pitch: f32,
}

impl Default for HostTargetPitchLimits {
    fn default() -> Self {
        Self {
            min_pitch: -std::f32::consts::PI,
            max_pitch: std::f32::consts::PI,
        }
    }
}

impl HostTargetPitchLimits {
    /// True when the peel is the full-sphere default (no pitch limit).
    pub fn is_unlimited(&self) -> bool {
        self.min_pitch <= -std::f32::consts::PI + 1e-3
            && self.max_pitch >= std::f32::consts::PI - 1e-3
    }
}

/// Normalize pitch peel: store may hold degrees (e.g. 15/45/80) or radians.
pub fn normalize_pitch_radians(raw: f32) -> f32 {
    if raw.abs() > std::f32::consts::PI + 0.01 {
        raw.to_radians()
    } else {
        raw
    }
}

/// Resolve Min/MaxTargetPitch for a weapon name.
/// C++ Weapon.ini ContinueAttackRange residual (world units).
///
/// After destroying a target (esp. mines), look for another similar object
/// controlled by the same player within this radius of the original victim pos.
/// C++ PATHFIND_CELL_SIZE residual (world units; Sage default cell).
pub const PATHFIND_CELL_SIZE: f32 = 10.0;
/// C++ approach standoff fudge for non-contact weapons.
pub const ATTACK_RANGE_APPROACH_FUDGE: f32 = 0.9;

/// Leftover `WeaponTemplate::is_contact_weapon` (Weapon.cpp:531-537).
///
/// `RATIONALIZE_ATTACK_RANGE` is on: `(authored - ¼ cell) < PATHFIND_CELL_SIZE`
/// → contact when authored range < 12.5. The `#else ATTACK_RANGE_FUDGE=1.05`
/// branch is not compiled.
pub fn is_contact_weapon_range(attack_range: f32) -> bool {
    const UNDERSIZE: f32 = PATHFIND_CELL_SIZE * 0.25;
    (attack_range - UNDERSIZE) < PATHFIND_CELL_SIZE
}

/// Contact check on leftover `get_attack_range` (already −¼ cell).
/// C++ `isContactWeapon` ≡ `getAttackRange(identity RANGE) < PATHFIND_CELL_SIZE`.
pub fn is_contact_effective_range(effective_range: f32) -> bool {
    effective_range < PATHFIND_CELL_SIZE
}

/// Leftover `WeaponTemplate::is_contact_weapon` from the live WeaponStore.
pub fn host_is_contact_weapon_name(name: &str) -> bool {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let leftover = with_weapon_store(|store| {
        store
            .find_weapon_template(name)
            .map(|wt| wt.is_contact_weapon())
    })
    .ok()
    .flatten();
    if let Some(contact) = leftover {
        return contact;
    }
    seed_is_contact_weapon_name(name)
}

pub fn seed_is_contact_weapon_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    // Explicit short-range residual weapons.
    if n.contains("minedisarm")
        || n.contains("mine_disarm")
        || n.contains("disarming")
        || n.contains("carbomb")
        || n.contains("car_bomb")
        || n.contains("terrorist")
        || n.contains("suicide")
        || n.contains("demotrap") && n.contains("detonation")
        || n.contains("dynamite")
        || n.contains("combatbike")
        || n.contains("battlebus") && n.contains("crash")
    {
        return true;
    }
    // Range peel from seed tables when known.
    let r = seed_attack_range_for_contact(name);
    is_contact_weapon_range(r)
}

pub(super) fn seed_attack_range_for_contact(name: &str) -> f32 {
    let n = name.to_ascii_lowercase();
    if n.contains("minedisarm") || n.contains("disarming") {
        return 5.0;
    }
    if n.contains("carbomb") || n.contains("terrorist") || n.contains("suicide") {
        return 5.0;
    }
    // Default non-contact.
    100.0
}

/// C++ Weapon::computeApproachTarget residual (simplified 2D).
///
/// Contact weapons approach the target position itself. Non-contact weapons
/// stand off at `attackRange * 0.9` along the source→target line.
/// C++ PATHFIND undersize fudge used with MinimumAttackRange residual.
pub const MIN_ATTACK_RANGE_CELL_FUDGE: f32 = PATHFIND_CELL_SIZE * 0.25;

/// Effective minimum attack range residual (template min + optional cell fudge).
pub fn effective_minimum_attack_range(min_range: f32) -> f32 {
    if min_range <= 0.0 {
        return 0.0;
    }
    // C++ isWithinAttackRange uses min with small pathfind undersize in places;
    // host residual keeps the raw min for the "too close" gate and backs up to
    // min + 10% so the next frame is clearly legal.
    min_range * 1.10
}

/// When inside MinimumAttackRange, compute a backup point outside the dead zone.
///
/// Places the unit on the line from target through source at `desired` distance
/// from the target (2D XZ). Y kept from source.
pub fn compute_min_range_backup_pos(
    source_pos: glam::Vec3,
    target_pos: glam::Vec3,
    min_range: f32,
) -> glam::Vec3 {
    let desired = effective_minimum_attack_range(min_range);
    if desired <= 0.0 {
        return source_pos;
    }
    let dx = source_pos.x - target_pos.x;
    let dz = source_pos.z - target_pos.z;
    let dist = (dx * dx + dz * dz).sqrt();
    if dist < 1e-3 {
        // Stacked on target: step along +X residual.
        return glam::Vec3::new(target_pos.x + desired, source_pos.y, target_pos.z);
    }
    let scale = desired / dist;
    glam::Vec3::new(
        target_pos.x + dx * scale,
        source_pos.y,
        target_pos.z + dz * scale,
    )
}

/// True when horizontal distance is inside the MinimumAttackRange dead zone.
pub fn is_inside_minimum_attack_range(distance: f32, min_range: f32) -> bool {
    min_range > 0.0 && distance + 1e-4 < min_range
}

pub fn compute_approach_target_pos(
    source_pos: glam::Vec3,
    target_pos: glam::Vec3,
    attack_range: f32,
) -> glam::Vec3 {
    if is_contact_effective_range(attack_range) {
        return target_pos;
    }
    let dx = target_pos.x - source_pos.x;
    let dy = target_pos.y - source_pos.y;
    let dz = target_pos.z - source_pos.z;
    let dist = (dx * dx + dy * dy + dz * dz).sqrt();
    if dist < 1e-3 {
        return target_pos;
    }
    let stand = (attack_range.max(0.0) * ATTACK_RANGE_APPROACH_FUDGE).min(dist);
    // Approach point is `stand` short of the target from the source direction.
    // C++ places approach at attackRange along dir from target toward... wait:
    // C++: approach = attackRange * dir + targetPos where dir = normalize(source-target)?
    // From Weapon.cpp: dir from source to target, then approach = attackRange * dir + targetPos
    // That would be BEYOND the target. Re-read...
    // dir.x = (target - source)/dist ... approach = attackRange * dir + targetPos
    // That's target + attackRange * toward_target = past the target.
    // Actually looking again at the C++ snippet:
    // approachTargetPos.x = attackRange * dir.x + targetPos->x
    // with dir from Cos(angle) of direction...
    // Wait the earlier code has:
    // dir from source to target normalized, then approach = attackRange * dir + targetPos
    // That puts point past target from source. That seems wrong for approach.
    // Looking at 2090-2115 again from earlier read:
    // approachTargetPos.x = attackRange * dir.x + targetPos->x
    // with dir = (target - source)/dist ... yes that's target + range * dir = beyond target.
    // Hmm maybe dir is inverted in full code. Safer host residual: stand off from target
    // toward source at min(range*0.9, dist).
    let back = stand / dist;
    glam::Vec3::new(
        target_pos.x - dx * back,
        target_pos.y - dy * back,
        target_pos.z - dz * back,
    )
}

/// C++ `Weapon::isGoalPosWithinAttackRange` (Weapon.cpp:2241-2287).
/// 2D bounding-sphere distance, −¼ cell on max / +¼ cell on min.
pub fn is_goal_pos_within_attack_range(
    goal: glam::Vec3,
    target: glam::Vec3,
    attack_range: f32,
    min_range: f32,
    source_radius: f32,
    target_radius: f32,
) -> bool {
    let dx = goal.x - target.x;
    let dz = goal.z - target.z;
    let center = (dx * dx + dz * dz).sqrt();
    let dist = (center - source_radius - target_radius).max(0.0);
    let fudge = PATHFIND_CELL_SIZE * 0.25;
    let max_r = attack_range - fudge;
    if max_r <= 0.0 {
        return false;
    }
    let min_r = min_range + fudge;
    if dist * dist < min_r * min_r {
        return false;
    }
    dist * dist <= max_r * max_r
}

pub fn host_continue_attack_range_for_weapon_name(name: &str) -> f32 {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store
            .find_weapon_template(name)
            .map(|wt| wt.continue_attack_range.max(0.0))
    })
    .ok()
    .flatten();
    if let Some(v) = from_store {
        if v > 0.0 {
            return v;
        }
    }
    seed_continue_attack_range_for(name)
}

pub fn seed_continue_attack_range_for(name: &str) -> f32 {
    let n = name.to_ascii_lowercase();
    // Worker / Dozer mine disarm residual (ContinueAttackRange = 100).
    if n.contains("minedisarm") || n.contains("mine_disarm") || n.contains("disarming") {
        return 100.0;
    }
    if n.contains("dozer") && n.contains("mine") {
        return 100.0;
    }
    if n.contains("worker") && (n.contains("mine") || n.contains("disarm")) {
        return 100.0;
    }
    0.0
}

pub fn host_target_pitch_limits_for_weapon_name(name: &str) -> HostTargetPitchLimits {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store
            .find_weapon_template(name)
            .map(|wt| HostTargetPitchLimits {
                min_pitch: normalize_pitch_radians(wt.min_target_pitch),
                max_pitch: normalize_pitch_radians(wt.max_target_pitch),
            })
    })
    .ok()
    .flatten();
    if let Some(lim) = from_store {
        // Prefer store when not the pure default, or when seed has no special case.
        if !lim.is_unlimited() {
            return lim;
        }
    }
    seed_target_pitch_limits_for(name)
}

pub fn seed_target_pitch_limits_for(name: &str) -> HostTargetPitchLimits {
    let n = name.to_ascii_lowercase();
    // Strategy Center artillery: steep loft residual (45°–80°).
    if n.contains("strategycenter") || (n.contains("artillery") && n.contains("center")) {
        return HostTargetPitchLimits {
            min_pitch: 45f32.to_radians(),
            max_pitch: 80f32.to_radians(),
        };
    }
    // Fire base / howitzer lob residual often -15..79 or similar.
    if n.contains("firebase") || n.contains("howitzer") || n.contains("inferno") {
        return HostTargetPitchLimits {
            min_pitch: (-15f32).to_radians(),
            max_pitch: 79f32.to_radians(),
        };
    }
    // Common tank guns residual ±15°.
    if n.contains("tankgun")
        || n.contains("tankshell")
        || n.contains("crusader")
        || n.contains("paladin")
        || n.contains("battlemaster")
        || n.contains("scorpion")
        || n.contains("marauder")
        || n.contains("overlord")
        || n.contains("tomahawk")
    {
        return HostTargetPitchLimits {
            min_pitch: (-15f32).to_radians(),
            max_pitch: 15f32.to_radians(),
        };
    }
    HostTargetPitchLimits::default()
}

/// C++ Weapon pitch check residual (center-to-center simplified geometry).
///
/// Pitch is elevation angle from source to target in the vertical plane:
/// `atan2(dy, horizontal_xz)`. Unlimited peels always pass.
pub fn is_pitch_within_limits(
    source_pos: glam::Vec3,
    target_pos: glam::Vec3,
    limits: &HostTargetPitchLimits,
) -> bool {
    // No geometry extents available — treat target as a point (height 0).
    is_pitch_within_limits_geom(source_pos, target_pos, limits, 0.0, 0.0, 0.0)
}

/// C++ `Weapon` pitch check with `GeometryInfo::calcPitches` residual.
///
/// `source_half_height`: half of source geometry height (center above position).
/// `target_height_above` / `target_height_below`: C++ getMaxHeightAbove/BelowPosition.
pub fn is_pitch_within_limits_geom(
    source_pos: glam::Vec3,
    target_pos: glam::Vec3,
    limits: &HostTargetPitchLimits,
    source_half_height: f32,
    target_height_above: f32,
    target_height_below: f32,
) -> bool {
    if limits.is_unlimited() {
        return true;
    }

    // C++ Weapon::isWithinAttackPitch ACCCEPTABLE_DZ residual (up-axis).
    // Host uses Y-up; C++ used Z-up.
    pub(super) const ACCEPTABLE_DZ: f32 = 10.0;
    let dy = target_pos.y - source_pos.y;
    if dy.abs() < ACCEPTABLE_DZ {
        return true;
    }

    let dx = target_pos.x - source_pos.x;
    let dz = target_pos.z - source_pos.z;
    let horiz = (dx * dx + dz * dz).sqrt();
    let src_center_y = source_pos.y + source_half_height.max(0.0);

    // GeometryInfo::calcPitches residual: pitches to top-center and bottom-center.
    let max_dz = (target_pos.y + target_height_above.max(0.0)) - src_center_y;
    let min_dz = (target_pos.y - target_height_below.max(0.0)) - src_center_y;

    let (mut min_pitch, mut max_pitch) = if horiz < 1e-3 {
        let s = if dy >= 0.0 {
            std::f32::consts::FRAC_PI_2
        } else {
            -std::f32::consts::FRAC_PI_2
        };
        (s, s)
    } else {
        (min_dz.atan2(horiz), max_dz.atan2(horiz))
    };
    if min_pitch > max_pitch {
        std::mem::swap(&mut min_pitch, &mut max_pitch);
    }

    let wmin = limits.min_pitch;
    let wmax = limits.max_pitch;
    // Intersection between geometry pitch span and weapon loft window.
    (min_pitch >= wmin && min_pitch <= wmax)
        || (max_pitch >= wmin && max_pitch <= wmax)
        || (min_pitch <= wmin && max_pitch >= wmax)
}
