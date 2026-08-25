//! Host China Battlemaster tank residual (main gun + Uranium Shells + horde/nationalism ROF).
//!
//! Residual slice (playability):
//! - `ChinaTankBattleMaster` / variants spawn with PRIMARY `BattleMasterTankGun`
//!   (dmg **60** / radius **5** / range **150** / Delay **2000**ms → 60 frames).
//! - Uranium Shells PLAYER_UPGRADE residual (`Upgrade_ChinaUraniumShells`):
//!   WeaponBonus DAMAGE **125%** → PrimaryDamage **75**.
//! - Horde residual (`HordeUpdate` ExactMatch VEHICLE allies, Radius **75**, Count **5**):
//!   WeaponBonus HORDE RATE_OF_FIRE **150%** → delay floor(60/1.5)=**40** frames.
//! - Nationalism residual (`Upgrade_Nationalism` while in horde):
//!   additional RATE_OF_FIRE **125%** (stacks with horde) → delay floor(60/(1.5*1.25))=**32** frames.
//! - Small PrimaryDamageRadius **5** splash residual on fire.
//!
//! Wave 67 residual pack (retail ChinaVehicle.ini / Weapon.ini / Locomotor.ini):
//! - Weapon residual: DamageType **ARMOR_PIERCING**, DeathType **NORMAL**,
//!   ScatterRadiusVsInfantry **10**, Projectile **BattleMasterTankShell**,
//!   FireFX **WeaponFX_GenericTankGunNoTracerSmall**, Delay **2000**ms → **60**f,
//!   ClipSize **0**, DetonationFX **WeaponFX_GenericTankShellDetonation**.
//! - Body residual: MaxHealth **400**, Vision **150**, Shroud **300**,
//!   BuildCost **800**, BuildTime **10**s → **300**f, TransportSlotCount **3**,
//!   TurretTurnRate **120**, Locomotor Speed **25**/Damaged **25**, Geometry BOX
//!   **13**/**9**/**10**.
//! - Horde residual: ExactMatch **Yes**, KindOf **VEHICLE**, Radius **75**, Count **5**,
//!   UpdateRate **1000**ms → **30**f; Nuclear Tanks death weapon residual honesty.
//!
//! Horde residual:
//! - RubOffRadius honorary membership (default **20**)
//! - Terrain-decal type / vehicle size / fade-in-out
//! - ExactMatch vehicle horde is not Battlemaster-only (Dragon / Inferno /
//!   Gattling / Overlord chassis share HordeUpdate)
//! - Fanaticism weapon-bonus nests under Nationalism (C++ evaluateMoraleBonus)

//! - BattleMasterTankShell DumbProjectile Bezier flight residual closed
//! - ScatterRadiusVsInfantry **10** residual miss cone closed (deterministic aim offset)
//! - Not full Nuclear Tanks death weapon / locomotor upgrade residual
//! - SCIENCE_BattlemasterTraining ELITE spawn residual closed in host_unit_training
//! - Not network uranium / horde replication (network deferred)

use super::Weapon;
use glam::Vec3;

/// Logic frames per second (host fixed step).
pub const BATTLE_MASTER_LOGIC_FPS: f32 = 30.0;

/// Retail primary weapon.
pub const BATTLE_MASTER_TANK_GUN: &str = "BattleMasterTankGun";
/// Retail Upgrade_ChinaUraniumShells (WeaponBonusUpgrade → PLAYER_UPGRADE).
pub const UPGRADE_CHINA_URANIUM_SHELLS: &str = "Upgrade_ChinaUraniumShells";
/// Retail Upgrade_Fanaticism (Infantry General). Nested under Nationalism in C++.
pub const UPGRADE_FANATICISM: &str = "Upgrade_Fanaticism";
/// Retail Upgrade_Nationalism (player science/upgrade; stacks with HORDE).
pub const UPGRADE_NATIONALISM: &str = "Upgrade_Nationalism";

/// C++ `TheGameLogic->getDrawIconUI()` — scripts hide horde rings with icon chrome.
pub fn leftover_horde_draw_icon_ui() -> bool {
    gamelogic::helpers::TheGameLogic::get_draw_icon_ui()
}

/// Retail Upgrade_ChinaNuclearTanks residual.
pub const UPGRADE_CHINA_NUCLEAR_TANKS: &str = "Upgrade_ChinaNuclearTanks";
/// Retail NuclearTankDeathWeapon residual.
pub const NUCLEAR_TANK_DEATH_WEAPON: &str = "NuclearTankDeathWeapon";

/// Retail PrimaryDamage base.
pub const BATTLE_MASTER_DAMAGE: f32 = 60.0;
/// Retail PrimaryDamageRadius residual splash.
pub const BATTLE_MASTER_SPLASH_RADIUS: f32 = 5.0;
/// Retail AttackRange.
pub const BATTLE_MASTER_RANGE: f32 = 150.0;
/// Retail DelayBetweenShots residual (msec).
pub const BATTLE_MASTER_BASE_DELAY_MS: u32 = 2_000;
/// Retail DelayBetweenShots 2000ms → 60 frames @ 30 FPS.
pub const BATTLE_MASTER_BASE_DELAY_FRAMES: u32 = 60;
/// Retail WeaponSpeed residual (shell flight residual; host hits still residual-instant).
pub const BATTLE_MASTER_PROJECTILE_SPEED: f32 = 400.0;
/// Retail ScatterRadiusVsInfantry residual.
pub const BATTLE_MASTER_SCATTER_VS_INFANTRY: f32 = 10.0;
/// Retail DamageType residual.
pub const BATTLE_MASTER_DAMAGE_TYPE: &str = "ARMOR_PIERCING";
/// Retail DeathType residual.
pub const BATTLE_MASTER_DEATH_TYPE: &str = "NORMAL";
/// Retail ProjectileObject residual.
pub const BATTLE_MASTER_PROJECTILE: &str = "BattleMasterTankShell";
/// Retail BattleMasterTankShell MaxHealth residual.
pub const BM_SHELL_MAX_HEALTH: f32 = 100.0;
/// DumbProjectileBehavior FirstHeight residual.
pub const BM_SHELL_FIRST_HEIGHT: f32 = 10.0;
/// DumbProjectileBehavior SecondHeight residual.
pub const BM_SHELL_SECOND_HEIGHT: f32 = 10.0;
/// FirstPercentIndent residual (50%).
pub const BM_SHELL_FIRST_PERCENT_INDENT: f32 = 0.50;
/// SecondPercentIndent residual (90%).
pub const BM_SHELL_SECOND_PERCENT_INDENT: f32 = 0.90;
/// Retail FireFX residual.
pub const BATTLE_MASTER_FIRE_FX: &str = "WeaponFX_GenericTankGunNoTracerSmall";
/// Retail ProjectileDetonationFX residual.
pub const BATTLE_MASTER_DETONATION_FX: &str = "WeaponFX_GenericTankShellDetonation";
/// Retail ClipSize residual (0 == infinite).
pub const BATTLE_MASTER_CLIP_SIZE: u32 = 0;

/// Uranium PLAYER_UPGRADE WeaponBonus DAMAGE 125%.
pub const BATTLE_MASTER_URANIUM_DAMAGE_MULT: f32 = 1.25;
/// HORDE WeaponBonus RATE_OF_FIRE 150%.
pub const BATTLE_MASTER_HORDE_ROF_MULT: f32 = 1.5;
/// NATIONALISM WeaponBonus RATE_OF_FIRE 125% (stacks with horde when both active).
pub const BATTLE_MASTER_NATIONALISM_ROF_MULT: f32 = 1.25;

/// Retail HordeUpdate Radius (exact-match allies counted within this).
pub const BATTLE_MASTER_HORDE_RADIUS: f32 = 75.0;
/// Retail HordeUpdate Count (includes self via C++ minCount-1 others).
pub const BATTLE_MASTER_HORDE_COUNT: u32 = 5;
/// Retail HordeUpdate UpdateRate residual (msec).
pub const BATTLE_MASTER_HORDE_UPDATE_MS: u32 = 1_000;
/// Retail HordeUpdate UpdateRate 1000ms → 30 frames @ 30 FPS.
pub const BATTLE_MASTER_HORDE_UPDATE_FRAMES: u32 = 30;
/// Retail HordeUpdate ExactMatch residual.
pub const BATTLE_MASTER_HORDE_EXACT_MATCH: bool = true;
/// Retail HordeUpdate KindOf residual.
pub const BATTLE_MASTER_HORDE_KIND_OF: &str = "VEHICLE";
/// C++ HordeUpdateModuleData default `m_rubOffRadius` (INI `RubOffRadius` override).
pub const BATTLE_MASTER_HORDE_RUB_OFF_RADIUS: f32 = 20.0;
/// C++ vehicle terrain-decal size = `3.5 * majorRadius`.
pub const VEHICLE_HORDE_DECAL_SIZE_MULT: f32 = 3.5;
/// C++ `setTerrainDecalFadeTarget(1.0f, 0.03f)` on join.
pub const HORDE_DECAL_FADE_IN_RATE: f32 = 0.03;
/// C++ `setTerrainDecalFadeTarget(0.0f, -0.03f)` on leave.
pub const HORDE_DECAL_FADE_OUT_RATE: f32 = -0.03;

/// Leftover `TerrainDecalType` ordinals (wave 111 `TERRAIN_DECAL_TYPE_NAMES`).
pub const TERRAIN_DECAL_HORDE: u8 = 1;
pub const TERRAIN_DECAL_HORDE_WITH_NATIONALISM: u8 = 2;
pub const TERRAIN_DECAL_HORDE_VEHICLE: u8 = 3;
pub const TERRAIN_DECAL_HORDE_WITH_FANATICISM: u8 = 6;
pub const TERRAIN_DECAL_CRATE: u8 = 5;
pub const TERRAIN_DECAL_CHEMSUIT: u8 = 7;
pub const TERRAIN_DECAL_NONE: u8 = 8;
pub const TERRAIN_DECAL_SHADOW_TEXTURE: u8 = 9;

/// C++ `TerrainDecalTextureName` table (W3DModelDraw.cpp).
pub fn terrain_decal_texture_name(decal_type: u8) -> &'static str {
    match decal_type {
        1 => "EXHorde",
        2 => "EXHorde_UP",
        3 => "EXHordeB",
        TERRAIN_DECAL_CRATE => "EXJunkCrate",
        6 => "EXHordeC_UP",
        TERRAIN_DECAL_CHEMSUIT => "EXChemSuit",
        TERRAIN_DECAL_SHADOW_TEXTURE => "shadow",
        _ => "",
    }
}

/// C++ CreateCrateDie crate glow: `2.5 * majorRadius`.
pub const CRATE_DECAL_SIZE_MULT: f32 = 2.5;
/// C++ CreateCrateDie fade-in `(1.0, 0.03)`.
pub const CRATE_DECAL_FADE_IN_RATE: f32 = 0.03;

/// Residual fire audio.
pub const BATTLE_MASTER_FIRE_AUDIO: &str = "BattlemasterTankWeapon";

// --- Body residual (ChinaTankBattleMaster) ---

/// Retail MaxHealth residual.
pub const BATTLE_MASTER_MAX_HEALTH: f32 = 400.0;
/// Retail VisionRange residual.
pub const BATTLE_MASTER_VISION_RANGE: f32 = 150.0;
/// Retail ShroudClearingRange residual.
pub const BATTLE_MASTER_SHROUD_CLEARING_RANGE: f32 = 300.0;
/// Retail BuildCost residual.
pub const BATTLE_MASTER_BUILD_COST: u32 = 800;
/// Retail BuildTime residual (seconds).
pub const BATTLE_MASTER_BUILD_TIME_SEC: f32 = 10.0;
/// BuildTime 10s → 300 frames @ 30 FPS.
pub const BATTLE_MASTER_BUILD_TIME_FRAMES: u32 = 300;
/// Retail TransportSlotCount residual.
pub const BATTLE_MASTER_TRANSPORT_SLOT_COUNT: u32 = 3;
/// Retail TurretTurnRate residual (deg/sec).
pub const BATTLE_MASTER_TURRET_TURN_RATE: f32 = 120.0;
/// Retail BattleMasterLocomotor Speed residual.
pub const BATTLE_MASTER_LOCOMOTOR_SPEED: f32 = 25.0;
/// Retail BattleMasterLocomotor SpeedDamaged residual.
pub const BATTLE_MASTER_LOCOMOTOR_SPEED_DAMAGED: f32 = 25.0;
/// Retail NuclearBattleMasterLocomotor Speed residual.
pub const BATTLE_MASTER_NUCLEAR_LOCOMOTOR_SPEED: f32 = 35.0;
/// Retail Geometry BOX MajorRadius residual.
pub const BATTLE_MASTER_GEOMETRY_MAJOR: f32 = 13.0;
/// Retail Geometry BOX MinorRadius residual.
pub const BATTLE_MASTER_GEOMETRY_MINOR: f32 = 9.0;
/// Retail GeometryHeight residual.
pub const BATTLE_MASTER_GEOMETRY_HEIGHT: f32 = 10.0;
/// Retail ExperienceValue residual.
pub const BATTLE_MASTER_EXPERIENCE_VALUE: [u32; 4] = [100, 100, 200, 400];
/// Retail ExperienceRequired residual.
pub const BATTLE_MASTER_EXPERIENCE_REQUIRED: [u32; 4] = [0, 200, 300, 600];

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn battlemaster_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * BATTLE_MASTER_LOGIC_FPS / 1000.0).round() as u32
}

/// Whether template is a residual Battlemaster tank chassis.
///
/// Fail-closed: name residual. Excludes weapons/shells/debris/science tokens.
pub fn is_battlemaster_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n.contains("weapon")
        || n.contains("shell")
        || n.contains("projectile")
        || n.contains("missile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("science")
        || n.contains("training")
        || n.contains("gun")
        || n.contains("crate")
        || n.contains("locomotor")
    {
        return false;
    }
    n.contains("battlemaster")
        || n.contains("battlemastertank")
        || n.contains("tankbattlemaster")
        || n == "china_battlemastertank"
        || n == "china_battletank"
        || n == "testbattlemaster"
}

/// Whether Uranium Shells upgrade tag is present.
pub fn has_uranium_shells_upgrade(applied_upgrades: &std::collections::HashSet<String>) -> bool {
    applied_upgrades.iter().any(|u| {
        let l = u.to_ascii_lowercase();
        l.contains("uraniumshell") || l == "upgrade_chinauraniumshells"
    })
}

/// Whether Nationalism upgrade tag is present on the unit residual.
pub fn has_nationalism_upgrade(applied_upgrades: &std::collections::HashSet<String>) -> bool {
    applied_upgrades.iter().any(|u| {
        let l = u.to_ascii_lowercase();
        l.contains("nationalism") || l == "upgrade_nationalism" || l.contains("chinanationalism")
    })
}

/// Whether Fanaticism upgrade tag is present (does not imply Nationalism).
pub fn has_fanaticism_upgrade(applied_upgrades: &std::collections::HashSet<String>) -> bool {
    applied_upgrades.iter().any(|u| {
        let l = u.to_ascii_lowercase();
        l.contains("fanaticism")
    })
}

/// C++ `evaluateMoraleBonus` nesting: FANATICISM only while NATIONALISM is set.
pub fn leftover_horde_fanaticism_bonus(
    has_nationalism: bool,
    has_fanaticism_upgrade: bool,
) -> bool {
    has_nationalism && has_fanaticism_upgrade
}

/// Apply Uranium residual damage mult when upgrade present.
pub fn battlemaster_damage_with_uranium(base_damage: f32, has_uranium: bool) -> f32 {
    if has_uranium {
        base_damage * BATTLE_MASTER_URANIUM_DAMAGE_MULT
    } else {
        base_damage
    }
}

/// Combined ROF multiplier residual (HORDE * NATIONALISM when both active).
///
/// Nationalism only applies while in horde (C++ AIUpdate evaluateMoraleBonus).
pub fn battlemaster_rof_multiplier(in_horde: bool, has_nationalism: bool) -> f32 {
    let mut rof = 1.0_f32;
    if in_horde {
        rof *= BATTLE_MASTER_HORDE_ROF_MULT;
        if has_nationalism {
            rof *= BATTLE_MASTER_NATIONALISM_ROF_MULT;
        }
    }
    rof
}

/// Delay frames residual: floor(base / ROF), min 1.
pub fn battlemaster_delay_frames(in_horde: bool, has_nationalism: bool) -> u32 {
    let base = BATTLE_MASTER_BASE_DELAY_FRAMES as f32;
    let rof = battlemaster_rof_multiplier(in_horde, has_nationalism);
    (base / rof).floor().max(1.0) as u32
}

/// Reload time seconds residual for delay frames @ 30 FPS.
pub fn delay_frames_to_reload_secs(delay_frames: u32) -> f32 {
    (delay_frames.max(1) as f32) / 30.0
}

/// (damage, range, delay_frames, splash_radius, projectile_speed) for bonuses.
pub fn battlemaster_weapon_stats(
    has_uranium: bool,
    in_horde: bool,
    has_nationalism: bool,
) -> (f32, f32, u32, f32, f32) {
    let dmg = battlemaster_damage_with_uranium(BATTLE_MASTER_DAMAGE, has_uranium);
    let delay = battlemaster_delay_frames(in_horde, has_nationalism);
    (
        dmg,
        BATTLE_MASTER_RANGE,
        delay,
        BATTLE_MASTER_SPLASH_RADIUS,
        BATTLE_MASTER_PROJECTILE_SPEED,
    )
}

/// Build residual Weapon for uranium + horde/nationalism ROF residual.
pub fn battlemaster_weapon(has_uranium: bool, in_horde: bool, has_nationalism: bool) -> Weapon {
    let (damage, range, delay, _splash, speed) =
        battlemaster_weapon_stats(has_uranium, in_horde, has_nationalism);
    Weapon {
        damage,
        range,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(delay),
        last_fire_time: 0.0,
        ammo: None,
        clip_size: 0,
        clip_reload_time: 0.0,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: speed,
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
        suspend_fx_frame: 0,
        reloading_clip: false,
        last_bonus_rof: 0.0,
    }
}

/// Splash residual damage at distance from impact.
///
/// Intended target takes full PrimaryDamage; others within PrimaryDamageRadius
/// take full PrimaryDamage residual (fail-closed vs continuous falloff).
pub fn battlemaster_splash_damage_at(
    is_intended_target: bool,
    distance_from_impact: f32,
    damage: f32,
) -> f32 {
    if is_intended_target {
        return damage;
    }
    if distance_from_impact <= BATTLE_MASTER_SPLASH_RADIUS {
        damage
    } else {
        0.0
    }
}

/// Legal residual splash target.
pub fn is_legal_battlemaster_splash_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind
}

/// Whether residual fire should apply Battlemaster residual path.

/// Cubic Bezier residual sample for BattleMasterTankShell DumbProjectileBehavior.
pub fn battlemaster_shell_bezier_point(from: Vec3, to: Vec3, t: f32) -> Vec3 {
    let t = t.clamp(0.0, 1.0);
    let delta = to - from;
    let p0 = from;
    let p3 = to;
    let p1 = from + delta * BM_SHELL_FIRST_PERCENT_INDENT + Vec3::Y * BM_SHELL_FIRST_HEIGHT;
    let p2 = from + delta * BM_SHELL_SECOND_PERCENT_INDENT + Vec3::Y * BM_SHELL_SECOND_HEIGHT;
    let u = 1.0 - t;
    let tt = t * t;
    let uu = u * u;
    let uuu = uu * u;
    let ttt = tt * t;
    p0 * uuu + p1 * (3.0 * uu * t) + p2 * (3.0 * u * tt) + p3 * ttt
}

/// Flight frame residual from ground distance @ WeaponSpeed 400.
pub fn battlemaster_shell_flight_frames(from: Vec3, to: Vec3) -> u32 {
    let dx = to.x - from.x;
    let dz = to.z - from.z;
    let dist = (dx * dx + dz * dz).sqrt().max(1.0);
    let frames = (dist / (BATTLE_MASTER_PROJECTILE_SPEED / BATTLE_MASTER_LOGIC_FPS)).ceil() as u32;
    frames.clamp(4, 180)
}

pub fn should_apply_battlemaster_residual(is_battlemaster: bool) -> bool {
    is_battlemaster
}

/// Horde residual: Count includes self (C++: others >= Count-1).
///
/// `nearby_same_type_allies` is the count of *other* exact-match allies within Radius.
pub fn is_in_horde(nearby_same_type_allies: u32) -> bool {
    // nearby others + self >= Count  ⇒ others >= Count - 1
    nearby_same_type_allies + 1 >= BATTLE_MASTER_HORDE_COUNT
}

/// 2D distance residual (XZ plane).
pub fn distance_2d(ax: f32, az: f32, bx: f32, bz: f32) -> f32 {
    let dx = ax - bx;
    let dz = az - bz;
    (dx * dx + dz * dz).sqrt()
}

/// Whether other unit counts toward self's horde residual (ExactMatch + AlliesOnly).
pub fn counts_toward_battlemaster_horde(
    self_alive: bool,
    other_alive: bool,
    same_team: bool,
    other_is_battlemaster: bool,
    distance: f32,
    radius: f32,
) -> bool {
    self_alive
        && other_alive
        && same_team
        && other_is_battlemaster
        && distance <= radius
        && distance >= 0.0
}

/// C++ `HordeUpdate` membership: Count-1 neighbors → true member; else honorary
/// if 2D-center distance to a true member ≤ `RubOffRadius`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LeftoverHordeMembership {
    pub in_horde: bool,
    pub true_member: bool,
}

/// C++ `iterateObjectsInRange(..., FROM_BOUNDINGSPHERE_3D)` input.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LeftoverHordeScanUnit {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub sphere_radius: f32,
    pub alive: bool,
}

impl LeftoverHordeScanUnit {
    pub fn xz(x: f32, z: f32, alive: bool) -> Self {
        Self {
            x,
            y: 0.0,
            z,
            sphere_radius: 0.0,
            alive,
        }
    }
}

/// C++ `FROM_BOUNDINGSPHERE_3D`: 3D center-to-center minus both sphere radii.
pub fn leftover_from_bounding_sphere_3d(
    a: &LeftoverHordeScanUnit,
    b: &LeftoverHordeScanUnit,
) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    let center = (dx * dx + dy * dy + dz * dz).sqrt();
    let combined = a.sphere_radius.max(0.0) + b.sphere_radius.max(0.0);
    (center - combined).max(0.0)
}

pub fn leftover_horde_major_radius(
    authored: bool,
    major_radius: f32,
    selection_radius: f32,
) -> f32 {
    if authored && major_radius > 0.0 {
        major_radius
    } else {
        selection_radius.max(1.0)
    }
}

pub fn leftover_horde_bounding_sphere_radius(
    authored: bool,
    bounding_sphere: f32,
    selection_radius: f32,
) -> f32 {
    if authored {
        bounding_sphere.max(0.0)
    } else {
        selection_radius.max(1.0)
    }
}

pub fn evaluate_leftover_horde_membership(
    nearby_within_radius: u32,
    min_count: u32,
    nearest_true_member_dist: Option<f32>,
    rub_off_radius: f32,
) -> LeftoverHordeMembership {
    if nearby_within_radius + 1 >= min_count {
        LeftoverHordeMembership {
            in_horde: true,
            true_member: true,
        }
    } else if nearest_true_member_dist
        .map(|d| d >= 0.0 && d <= rub_off_radius)
        .unwrap_or(false)
    {
        LeftoverHordeMembership {
            in_horde: true,
            true_member: false,
        }
    } else {
        LeftoverHordeMembership {
            in_horde: false,
            true_member: false,
        }
    }
}

/// Two-pass leftover blob: true members first, then RubOff honorary.
/// XZ-only wrapper (sphere 0) for tests; live host uses `_scan`.
pub fn evaluate_leftover_horde_blob<F>(
    units: &[(f32, f32, bool)],
    min_count: u32,
    radius: f32,
    rub_off_radius: f32,
    counts_toward: F,
) -> Vec<LeftoverHordeMembership>
where
    F: FnMut(usize, usize, f32) -> bool,
{
    let scan: Vec<LeftoverHordeScanUnit> = units
        .iter()
        .map(|(x, z, alive)| LeftoverHordeScanUnit::xz(*x, *z, *alive))
        .collect();
    evaluate_leftover_horde_blob_scan(&scan, min_count, radius, rub_off_radius, counts_toward)
}

/// C++ Count uses `FROM_BOUNDINGSPHERE_3D`; RubOff uses `FROM_CENTER_2D`
/// against the 3D-scan candidate set.
pub fn evaluate_leftover_horde_blob_scan<F>(
    units: &[LeftoverHordeScanUnit],
    min_count: u32,
    _radius: f32,
    rub_off_radius: f32,
    mut counts_toward: F,
) -> Vec<LeftoverHordeMembership>
where
    F: FnMut(usize, usize, f32) -> bool,
{
    let n = units.len();
    let mut nearby = vec![0u32; n];
    let mut true_member = vec![false; n];
    let mut out = vec![
        LeftoverHordeMembership {
            in_horde: false,
            true_member: false,
        };
        n
    ];
    for i in 0..n {
        if !units[i].alive {
            continue;
        }
        for j in 0..n {
            if i == j {
                continue;
            }
            let dist = leftover_from_bounding_sphere_3d(&units[i], &units[j]);
            if counts_toward(i, j, dist) {
                nearby[i] = nearby[i].saturating_add(1);
            }
        }
        if nearby[i] + 1 >= min_count {
            true_member[i] = true;
            out[i] = LeftoverHordeMembership {
                in_horde: true,
                true_member: true,
            };
        }
    }
    for i in 0..n {
        if out[i].true_member || !units[i].alive {
            continue;
        }
        for j in 0..n {
            if i == j || !true_member[j] {
                continue;
            }
            let scan_dist = leftover_from_bounding_sphere_3d(&units[i], &units[j]);
            if !counts_toward(i, j, scan_dist) {
                continue;
            }
            let dist_2d = distance_2d(units[i].x, units[i].z, units[j].x, units[j].z);
            if dist_2d <= rub_off_radius {
                out[i] = LeftoverHordeMembership {
                    in_horde: true,
                    true_member: false,
                };
                break;
            }
        }
    }
    out
}

fn leftover_vehicle_name_excluded(n: &str) -> bool {
    n.contains("weapon")
        || n.contains("shell")
        || n.contains("projectile")
        || n.contains("missile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("science")
        || n.contains("training")
        || n.contains("locomotor")
        || n.contains("crate")
}

fn is_dragon_horde_template(n: &str) -> bool {
    if n.contains("firewall") || n.contains("firestorm") || n.contains("segment") {
        return false;
    }
    n.contains("dragontank")
        || n.contains("tankdragon")
        || n == "china_dragontank"
        || n == "testdragontank"
        || (n.contains("dragon") && (n.contains("tank") || n.contains("vehicle")))
}

fn is_inferno_horde_template(n: &str) -> bool {
    if n.contains("firefield") || n.contains("fire_field") {
        return false;
    }
    n.contains("infernocannon")
        || n.contains("inferno_cannon")
        || n.contains("vehicleinferno")
        || n == "testinfernocannon"
        || n == "test_inferno_cannon"
}

fn is_gattling_tank_horde_template(n: &str) -> bool {
    if n.contains("overlord") || n.contains("helix") || n.contains("gun") {
        return false;
    }
    if n.contains("cannon") && !n.contains("tank") && !n.contains("vehicle") {
        return false;
    }
    n.contains("gattlingtank")
        || n.contains("gatlingtank")
        || n.contains("tankgattling")
        || n.contains("tankgatling")
        || n.contains("vehiclegattling")
        || n.contains("vehiclegatling")
        || n == "china_gattlingtank"
        || n == "testgattlingtank"
        || n == "testgatlingtank"
}

fn is_overlord_horde_template(n: &str) -> bool {
    if n.contains("gattling")
        || n.contains("gatling")
        || n.contains("propaganda")
        || n.contains("bunker")
        || n.contains("helix")
        || n.contains("command")
        || n.contains("minigun")
        || n.ends_with("gun")
        || n.contains("tankgun")
        || n.contains("exhaust")
    {
        return false;
    }
    n == "testoverlord"
        || n == "testemperor"
        || n == "china_overlordtank"
        || n == "china_overlord"
        || n == "tank_chinatankemperor"
        || n.contains("tankoverlord")
        || n.contains("overlordtank")
        || n.contains("tankemperor")
        || n.contains("emperortank")
        || (n.contains("overlord")
            && (n.contains("tank") || n.contains("vehicle") || n.contains("china")))
        || (n.contains("emperor")
            && (n.contains("tank") || n.contains("vehicle") || n.contains("china")))
}

/// C++ KINDOF_PORTABLE_STRUCTURE ride-on (Overlord/Helix bunker / gattling / speaker).
pub fn is_portable_structure_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    (n.contains("overlord") || n.contains("helix") || n.contains("emperor"))
        && (n.contains("bunker")
            || n.contains("gattling")
            || n.contains("gatling")
            || n.contains("propaganda"))
}

/// Retail China ExactMatch HordeUpdate vehicles, not Battlemaster-only.
pub fn is_china_vehicle_horde_unit(template_name: &str) -> bool {
    if is_battlemaster_template(template_name) {
        return true;
    }
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() || leftover_vehicle_name_excluded(&n) {
        return false;
    }
    if is_portable_structure_template(template_name) {
        return false;
    }
    is_dragon_horde_template(&n)
        || is_inferno_horde_template(&n)
        || is_gattling_tank_horde_template(&n)
        || is_overlord_horde_template(&n)
}

/// Any retail HordeUpdate chassis (infantry or ExactMatch vehicle).
/// C++ `evaluateMoraleBonus` runs on every unit with `getHordeUpdateInterface`.
pub fn is_china_horde_update_unit(template_name: &str) -> bool {
    is_china_vehicle_horde_unit(template_name) || leftover_china_infantry_horde_name(template_name)
}

/// C++ ExactMatch: same ThingTemplate identity (live: exact template name).
/// Does not lump Emperor+Overlord or name-variant chassis.
pub fn same_vehicle_horde_family(a: &str, b: &str) -> bool {
    !a.is_empty() && a == b
}

/// C++ HordeUpdate infantry/vehicle terrain-decal matrix.
pub fn leftover_horde_decal_type(
    is_infantry: bool,
    has_nationalism: bool,
    has_fanaticism: bool,
) -> u8 {
    if has_nationalism {
        if has_fanaticism {
            TERRAIN_DECAL_HORDE_WITH_FANATICISM
        } else {
            TERRAIN_DECAL_HORDE_WITH_NATIONALISM
        }
    } else if is_infantry {
        TERRAIN_DECAL_HORDE
    } else {
        TERRAIN_DECAL_HORDE_VEHICLE
    }
}

pub fn leftover_vehicle_horde_decal_size(major_radius: f32) -> f32 {
    VEHICLE_HORDE_DECAL_SIZE_MULT * major_radius.max(0.0)
}

/// C++ `W3DModelDraw::setTerrainDecal` infantry size from ShadowSize (never invent 40wu).
pub fn leftover_infantry_horde_decal_size(shadow_size_x: f32, shadow_size_y: f32) -> f32 {
    if shadow_size_x > 0.0 {
        shadow_size_x
    } else {
        shadow_size_y.max(0.0)
    }
}

/// C++ `W3DProjectedShadowManager::addDecal`: when ShadowSize is 0, size is
/// object-space bbox `Extent * 2`. Infantry EXHorde uses this fallback.
pub fn leftover_infantry_horde_decal_size_or_bbox(
    shadow_size_x: f32,
    shadow_size_y: f32,
    bbox_extent_x: f32,
    bbox_extent_y: f32,
) -> f32 {
    let size = leftover_infantry_horde_decal_size(shadow_size_x, shadow_size_y);
    if size > 0.0 {
        return size;
    }
    let sx = if bbox_extent_x > 0.0 {
        bbox_extent_x * 2.0
    } else {
        0.0
    };
    let sy = if bbox_extent_y > 0.0 {
        bbox_extent_y * 2.0
    } else {
        0.0
    };
    if sx > 0.0 { sx } else { sy }
}

/// C++ `HordeUpdate.cpp:253` vehicle membership gate.
pub fn leftover_vehicle_horde_membership_due(
    current_frame: u32,
    last_horde_refresh_frame: u32,
    update_rate: u32,
) -> bool {
    current_frame > last_horde_refresh_frame.saturating_add(update_rate)
}

/// First live scan is a wake (tests + spawn). Later infantry sleeps UpdateRate;
/// vehicles recheck membership only when `frame > last + UpdateRate`.
pub fn leftover_horde_take_wake(
    initialized: bool,
    is_infantry: bool,
    current_frame: u32,
    last_refresh: u32,
    next_wake: u32,
    update_rate: u32,
) -> (bool, bool, u32, u32) {
    if !initialized {
        return (
            true,
            true,
            current_frame,
            current_frame.saturating_add(update_rate.max(1)),
        );
    }
    if is_infantry {
        if current_frame >= next_wake {
            (
                true,
                true,
                current_frame,
                current_frame.saturating_add(update_rate.max(1)),
            )
        } else {
            (false, true, last_refresh, next_wake)
        }
    } else if leftover_vehicle_horde_membership_due(current_frame, last_refresh, update_rate) {
        (true, true, current_frame, next_wake)
    } else {
        (false, true, last_refresh, next_wake)
    }
}

/// C++ `HordeUpdate.cpp:146-147` constructor first-wake delay.
pub fn leftover_horde_first_wake_delay(update_rate: u32) -> u32 {
    let delay = update_rate.max(1) as i32;
    gamelogic::helpers::get_game_logic_random_value(1, delay).max(1) as u32
}

/// Resolve Object INI ShadowSize without inventing a 40wu fallback.
pub fn leftover_template_shadow_size(
    template_name: &str,
    authored_x: f32,
    authored_y: f32,
) -> (f32, f32) {
    if authored_x > 0.0 || authored_y > 0.0 {
        return (authored_x.max(0.0), authored_y.max(0.0));
    }
    if let Some(guard) = game_engine::common::thing::thing_factory::try_get_thing_factory() {
        if let Some(factory) = guard.as_ref() {
            if let Some(tmpl) = factory.find_template(template_name, false) {
                let sx = tmpl.get_shadow_size_x();
                let sy = tmpl.get_shadow_size_y();
                if sx > 0.0 || sy > 0.0 {
                    return (sx, sy);
                }
            }
        }
    }
    if let Some(mgr) = crate::assets::get_asset_manager() {
        if let Ok(m) = mgr.lock() {
            if let Some(def) = m.get_object_definition(template_name) {
                let parse = |key: &str| {
                    def.attributes
                        .iter()
                        .find_map(|(k, v)| {
                            k.eq_ignore_ascii_case(key)
                                .then(|| v.parse::<f32>().ok())
                                .flatten()
                        })
                        .unwrap_or(0.0)
                };
                let sx = parse("ShadowSizeX");
                let sy = parse("ShadowSizeY");
                if sx > 0.0 || sy > 0.0 {
                    return (sx, sy);
                }
            }
        }
    }
    (0.0, 0.0)
}

/// Resolve Object INI `Shadow` bits. Authored live bits win; else leftover factory / INI.
pub fn leftover_template_shadow_type(template_name: &str, authored: u32) -> u32 {
    if authored != 0 {
        return authored;
    }
    if let Some(guard) = game_engine::common::thing::thing_factory::try_get_thing_factory() {
        if let Some(factory) = guard.as_ref() {
            if let Some(tmpl) = factory.find_template(template_name, false) {
                let bits = tmpl.get_shadow_type().bits() as u32;
                if bits != 0 {
                    return bits;
                }
            }
        }
    }
    if let Some(mgr) = crate::assets::get_asset_manager() {
        if let Ok(m) = mgr.lock() {
            if let Some(def) = m.get_object_definition(template_name) {
                if let Some(raw) = def
                    .attributes
                    .iter()
                    .find_map(|(k, v)| k.eq_ignore_ascii_case("Shadow").then_some(v.as_str()))
                {
                    return crate::game_logic::host_enum_table_residual::parse_shadow_type_bits(
                        raw,
                    );
                }
            }
        }
    }
    0
}

/// Resolve Object INI ShadowOffset without inventing a shift.
pub fn leftover_template_shadow_offset(
    template_name: &str,
    authored_x: f32,
    authored_y: f32,
) -> (f32, f32) {
    if authored_x != 0.0 || authored_y != 0.0 {
        return (authored_x, authored_y);
    }
    if let Some(guard) = game_engine::common::thing::thing_factory::try_get_thing_factory() {
        if let Some(factory) = guard.as_ref() {
            if let Some(tmpl) = factory.find_template(template_name, false) {
                let ox = tmpl.get_shadow_offset_x();
                let oy = tmpl.get_shadow_offset_y();
                if ox != 0.0 || oy != 0.0 {
                    return (ox, oy);
                }
            }
        }
    }
    if let Some(mgr) = crate::assets::get_asset_manager() {
        if let Ok(m) = mgr.lock() {
            if let Some(def) = m.get_object_definition(template_name) {
                let parse = |key: &str| {
                    def.attributes
                        .iter()
                        .find_map(|(k, v)| {
                            k.eq_ignore_ascii_case(key)
                                .then(|| v.parse::<f32>().ok())
                                .flatten()
                        })
                        .unwrap_or(0.0)
                };
                return (parse("ShadowOffsetX"), parse("ShadowOffsetY"));
            }
        }
    }
    (0.0, 0.0)
}

/// C++ join/leave fade. `None` when membership did not change.
pub fn leftover_horde_decal_fade(was_in_horde: bool, now_in_horde: bool) -> Option<(f32, f32)> {
    if !was_in_horde && now_in_horde {
        Some((1.0, HORDE_DECAL_FADE_IN_RATE))
    } else if was_in_horde && !now_in_horde {
        Some((0.0, HORDE_DECAL_FADE_OUT_RATE))
    } else {
        None
    }
}

/// C++ INI `FlagSubObjectNames` leftover meshes (HordeFlag.W3D / FLAG).
pub const HORDE_FLAG_SUB_OBJECT_NAMES: &[&str] = &["FLAG", "HordeFlag", "HordeFlag2"];

fn leftover_china_infantry_horde_name(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.contains("weapon")
        || n.contains("missile")
        || n.contains("projectile")
        || n.contains("locomotor")
    {
        return false;
    }
    n.contains("redguard")
        || n.contains("red_guard")
        || n.contains("tankhunter")
        || n.contains("tank_hunter")
        || n.contains("minigunner")
        || n.contains("mini_gunner")
}

pub fn leftover_unit_has_horde_flag_subobjects(template_name: &str) -> bool {
    is_china_vehicle_horde_unit(template_name) || leftover_china_infantry_horde_name(template_name)
}

pub fn hide_leftover_horde_flag_subobjects(
    vis: &mut crate::game_logic::host_sub_objects_upgrade::HostSubObjectVisibility,
) {
    vis.apply_show_hide(&[], HORDE_FLAG_SUB_OBJECT_NAMES);
}

pub fn leftover_horde_flag_visibility_for_template(
    template_name: &str,
) -> crate::game_logic::host_sub_objects_upgrade::HostSubObjectVisibility {
    let mut vis = crate::game_logic::host_sub_objects_upgrade::HostSubObjectVisibility::default();
    if leftover_unit_has_horde_flag_subobjects(template_name) {
        hide_leftover_horde_flag_subobjects(&mut vis);
    }
    vis
}

// --- Wave 67 residual honesty packs ---

/// Wave 67 residual honesty: Battlemaster weapon residual peel.
pub fn honesty_battlemaster_weapon_residual_ok() -> bool {
    BATTLE_MASTER_TANK_GUN == "BattleMasterTankGun"
        && (BATTLE_MASTER_DAMAGE - 60.0).abs() < 0.01
        && (BATTLE_MASTER_SPLASH_RADIUS - 5.0).abs() < 0.01
        && (BATTLE_MASTER_RANGE - 150.0).abs() < 0.01
        && BATTLE_MASTER_BASE_DELAY_MS == 2_000
        && BATTLE_MASTER_BASE_DELAY_FRAMES == battlemaster_ms_to_frames(BATTLE_MASTER_BASE_DELAY_MS)
        && BATTLE_MASTER_BASE_DELAY_FRAMES == 60
        && (BATTLE_MASTER_PROJECTILE_SPEED - 400.0).abs() < 0.01
        && (BATTLE_MASTER_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01
        && BATTLE_MASTER_DAMAGE_TYPE == "ARMOR_PIERCING"
        && BATTLE_MASTER_DEATH_TYPE == "NORMAL"
        && BATTLE_MASTER_PROJECTILE == "BattleMasterTankShell"
        && (BM_SHELL_FIRST_HEIGHT - 10.0).abs() < 0.01
        && (BM_SHELL_SECOND_PERCENT_INDENT - 0.90).abs() < 0.001
        && BATTLE_MASTER_FIRE_FX == "WeaponFX_GenericTankGunNoTracerSmall"
        && BATTLE_MASTER_DETONATION_FX == "WeaponFX_GenericTankShellDetonation"
        && BATTLE_MASTER_CLIP_SIZE == 0
        && BATTLE_MASTER_FIRE_AUDIO == "BattlemasterTankWeapon"
        && (BATTLE_MASTER_URANIUM_DAMAGE_MULT - 1.25).abs() < 0.001
        && UPGRADE_CHINA_URANIUM_SHELLS == "Upgrade_ChinaUraniumShells"
        && {
            let w = battlemaster_weapon(true, false, false);
            (w.damage - 75.0).abs() < 0.01 && !w.can_target_air && w.can_target_ground
        }
}

/// Wave 67 residual honesty: horde / nationalism residual peel.
pub fn honesty_battlemaster_horde_residual_ok() -> bool {
    (BATTLE_MASTER_HORDE_RADIUS - 75.0).abs() < 0.01
        && BATTLE_MASTER_HORDE_COUNT == 5
        && BATTLE_MASTER_HORDE_UPDATE_MS == 1_000
        && BATTLE_MASTER_HORDE_UPDATE_FRAMES
            == battlemaster_ms_to_frames(BATTLE_MASTER_HORDE_UPDATE_MS)
        && BATTLE_MASTER_HORDE_EXACT_MATCH
        && BATTLE_MASTER_HORDE_KIND_OF == "VEHICLE"
        && (BATTLE_MASTER_HORDE_RUB_OFF_RADIUS - 20.0).abs() < 0.01
        && (BATTLE_MASTER_HORDE_ROF_MULT - 1.5).abs() < 0.001
        && (BATTLE_MASTER_NATIONALISM_ROF_MULT - 1.25).abs() < 0.001
        && UPGRADE_NATIONALISM == "Upgrade_Nationalism"
        && battlemaster_delay_frames(true, false) == 40
        && battlemaster_delay_frames(true, true) == 32
        && is_in_horde(4)
        && !is_in_horde(3)
        && evaluate_leftover_horde_membership(3, 5, Some(15.0), 20.0).in_horde
        && !evaluate_leftover_horde_membership(3, 5, Some(15.0), 20.0).true_member
        && !evaluate_leftover_horde_membership(3, 5, Some(25.0), 20.0).in_horde
        && is_china_vehicle_horde_unit("ChinaTankDragon")
        && is_china_vehicle_horde_unit("ChinaVehicleInfernoCannon")
        && is_china_vehicle_horde_unit("ChinaTankGattling")
        && is_china_vehicle_horde_unit("ChinaTankOverlord")
        && is_china_horde_update_unit("ChinaTankDragon")
        && is_china_horde_update_unit("ChinaVehicleInfernoCannon")
        && is_china_horde_update_unit("ChinaTankGattling")
        && is_china_horde_update_unit("ChinaTankOverlord")
        && is_china_horde_update_unit("ChinaInfantryRedguard")
        && !is_china_vehicle_horde_unit("OverlordGattlingCannon")
        && leftover_horde_decal_type(false, false, false) == TERRAIN_DECAL_HORDE_VEHICLE
        && leftover_horde_decal_type(true, false, true) == TERRAIN_DECAL_HORDE
        && leftover_horde_decal_type(true, true, true) == TERRAIN_DECAL_HORDE_WITH_FANATICISM
        && leftover_infantry_horde_decal_size(0.0, 0.0) == 0.0
        && (leftover_infantry_horde_decal_size_or_bbox(0.0, 0.0, 4.0, 3.0) - 8.0).abs() < 0.01
        && leftover_vehicle_horde_membership_due(31, 0, 30)
        && (leftover_vehicle_horde_decal_size(13.0) - 45.5).abs() < 0.01
        && same_vehicle_horde_family("ChinaTankBattleMaster", "ChinaTankBattleMaster")
        && !same_vehicle_horde_family("ChinaTankOverlord", "ChinaTankEmperor")
        && !same_vehicle_horde_family("ChinaTankBattleMaster", "Infa_ChinaTankBattleMaster")
}

/// Wave 67 residual honesty: Battlemaster body residual peel.
pub fn honesty_battlemaster_body_residual_ok() -> bool {
    (BATTLE_MASTER_MAX_HEALTH - 400.0).abs() < 0.01
        && (BATTLE_MASTER_VISION_RANGE - 150.0).abs() < 0.01
        && (BATTLE_MASTER_SHROUD_CLEARING_RANGE - 300.0).abs() < 0.01
        && BATTLE_MASTER_BUILD_COST == 800
        && (BATTLE_MASTER_BUILD_TIME_SEC - 10.0).abs() < 0.01
        && BATTLE_MASTER_BUILD_TIME_FRAMES
            == (BATTLE_MASTER_BUILD_TIME_SEC * BATTLE_MASTER_LOGIC_FPS).round() as u32
        && BATTLE_MASTER_BUILD_TIME_FRAMES == 300
        && BATTLE_MASTER_TRANSPORT_SLOT_COUNT == 3
        && (BATTLE_MASTER_TURRET_TURN_RATE - 120.0).abs() < 0.01
        && (BATTLE_MASTER_LOCOMOTOR_SPEED - 25.0).abs() < 0.01
        && (BATTLE_MASTER_LOCOMOTOR_SPEED_DAMAGED - 25.0).abs() < 0.01
        && (BATTLE_MASTER_NUCLEAR_LOCOMOTOR_SPEED - 35.0).abs() < 0.01
        && (BATTLE_MASTER_GEOMETRY_MAJOR - 13.0).abs() < 0.01
        && (BATTLE_MASTER_GEOMETRY_MINOR - 9.0).abs() < 0.01
        && (BATTLE_MASTER_GEOMETRY_HEIGHT - 10.0).abs() < 0.01
        && BATTLE_MASTER_EXPERIENCE_VALUE == [100, 100, 200, 400]
        && BATTLE_MASTER_EXPERIENCE_REQUIRED == [0, 200, 300, 600]
        && UPGRADE_CHINA_NUCLEAR_TANKS == "Upgrade_ChinaNuclearTanks"
        && NUCLEAR_TANK_DEATH_WEAPON == "NuclearTankDeathWeapon"
}

/// Combined Wave 67 Battlemaster residual honesty pack.

/// Apply BattleMasterTankGun ScatterRadiusVsInfantry residual to aim.
pub fn battlemaster_scatter_aim(aim: Vec3, target_is_infantry: bool, seed: u32) -> (Vec3, bool) {
    use crate::game_logic::weapon_bootstrap::{host_effective_scatter_radius, scatter_aim_offset};
    let mut scatter = host_effective_scatter_radius(BATTLE_MASTER_TANK_GUN, target_is_infantry);
    if target_is_infantry && scatter <= 0.0 {
        scatter = BATTLE_MASTER_SCATTER_VS_INFANTRY;
    }
    if scatter <= 0.0 {
        return (aim, false);
    }
    let off = scatter_aim_offset(seed, scatter);
    (Vec3::new(aim.x + off.x, aim.y, aim.z + off.z), true)
}

/// Whether BattleMasterTankGun residual misses intended infantry via ScatterRadiusVsInfantry.
pub fn battlemaster_scatter_misses_infantry(
    target_is_infantry: bool,
    seed: u32,
    target_hit_radius: f32,
) -> bool {
    use crate::game_logic::weapon_bootstrap::scatter_misses_intended_target;
    if !target_is_infantry {
        return false;
    }
    scatter_misses_intended_target(BATTLE_MASTER_SCATTER_VS_INFANTRY, seed, target_hit_radius)
}

/// Wave residual honesty: Battlemaster ScatterRadiusVsInfantry peels.
pub fn honesty_battlemaster_scatter_vs_infantry_ok() -> bool {
    use crate::game_logic::weapon_bootstrap::host_effective_scatter_radius;
    let vs = host_effective_scatter_radius(BATTLE_MASTER_TANK_GUN, true);
    let ground = host_effective_scatter_radius(BATTLE_MASTER_TANK_GUN, false);
    (BATTLE_MASTER_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01
        && ((vs - 10.0).abs() < 0.01 || vs <= 0.0)
        && ground.abs() < 0.01
        && {
            let (sc, applied) = battlemaster_scatter_aim(Vec3::new(0.0, 0.0, 0.0), true, 3);
            applied && sc.length() <= BATTLE_MASTER_SCATTER_VS_INFANTRY + 0.01
        }
}

pub fn honesty_battlemaster_residual_pack_ok() -> bool {
    honesty_battlemaster_weapon_residual_ok()
        && honesty_battlemaster_horde_residual_ok()
        && honesty_battlemaster_body_residual_ok()
        && honesty_battlemaster_scatter_vs_infantry_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn battlemaster_scatter_vs_infantry_peels() {
        assert!(honesty_battlemaster_scatter_vs_infantry_ok());
        let aim = Vec3::new(140.0, 0.0, 18.0);
        let (no_sc, applied) = battlemaster_scatter_aim(aim, false, 31);
        assert!(!applied);
        assert_eq!(no_sc, aim);
        let (sc, applied) = battlemaster_scatter_aim(aim, true, 31);
        assert!(applied);
        let d = ((sc.x - aim.x).powi(2) + (sc.z - aim.z).powi(2)).sqrt();
        assert!(d > 0.01 && d <= BATTLE_MASTER_SCATTER_VS_INFANTRY + 0.01);

        assert!(battlemaster_scatter_misses_infantry(true, 37, 0.5));
        assert!(!battlemaster_scatter_misses_infantry(true, 37, 100.0));
        assert!(!battlemaster_scatter_misses_infantry(false, 37, 0.5));
    }
    use std::collections::HashSet;

    #[test]
    fn battlemaster_name_matrix() {
        assert!(is_battlemaster_template("ChinaTankBattleMaster"));
        assert!(is_battlemaster_template("China_BattlemasterTank"));
        assert!(is_battlemaster_template("China_BattleTank"));
        assert!(is_battlemaster_template("TestBattlemaster"));
        assert!(is_battlemaster_template("Tank_ChinaTankBattleMaster"));
        assert!(is_battlemaster_template("Nuke_ChinaTankBattleMaster"));
        assert!(!is_battlemaster_template("BattleMasterTankGun"));
        assert!(!is_battlemaster_template("BattleMasterTankShell"));
        assert!(!is_battlemaster_template("SCIENCE_BattlemasterTraining"));
        assert!(!is_battlemaster_template("Upgrade_ChinaUraniumShells"));
        assert!(!is_battlemaster_template("ChinaTankDragon"));
        assert!(!is_battlemaster_template("ChinaTankOverlord"));
        assert!(!is_battlemaster_template("BattleMasterLocomotor"));
    }

    #[test]
    fn base_gun_stats() {
        let (d, r, f, s, sp) = battlemaster_weapon_stats(false, false, false);
        assert!((d - 60.0).abs() < 0.01);
        assert!((r - 150.0).abs() < 0.01);
        assert_eq!(f, 60);
        assert!((s - 5.0).abs() < 0.01);
        assert!((sp - 400.0).abs() < 0.01);
        let w = battlemaster_weapon(false, false, false);
        assert!((w.damage - 60.0).abs() < 0.01);
        assert!((w.reload_time - 2.0).abs() < 0.01);
        assert!(!w.can_target_air);
        assert!(w.can_target_ground);
    }

    #[test]
    fn uranium_damage_125_percent() {
        let (d, _, f, _, _) = battlemaster_weapon_stats(true, false, false);
        assert!((d - 75.0).abs() < 0.01);
        assert_eq!(f, 60); // uranium is damage, not ROF
        assert!((battlemaster_damage_with_uranium(60.0, true) - 75.0).abs() < 0.01);

        let mut tags = HashSet::new();
        tags.insert(UPGRADE_CHINA_URANIUM_SHELLS.to_string());
        assert!(has_uranium_shells_upgrade(&tags));
        let w_base = battlemaster_weapon(false, false, false);
        let w_horde = battlemaster_weapon(false, true, false);
        let w_both = battlemaster_weapon(false, true, true);
        assert!(w_horde.reload_time < w_base.reload_time - 0.05);
        assert!(w_both.reload_time < w_horde.reload_time - 0.01);
        let w_full = battlemaster_weapon(true, true, true);
        assert!((w_full.damage - 75.0).abs() < 0.01);
        assert!((w_full.reload_time - (32.0 / 30.0)).abs() < 0.02);
    }

    #[test]
    fn horde_decal_matrix() {
        assert_eq!(
            leftover_horde_decal_type(true, false, false),
            TERRAIN_DECAL_HORDE
        );
        assert_eq!(
            leftover_horde_decal_type(false, false, false),
            TERRAIN_DECAL_HORDE_VEHICLE
        );
        assert_eq!(
            leftover_horde_decal_type(false, true, false),
            TERRAIN_DECAL_HORDE_WITH_NATIONALISM
        );
        assert_eq!(
            leftover_horde_decal_type(true, false, true),
            TERRAIN_DECAL_HORDE
        );
        assert_eq!(
            leftover_horde_decal_type(true, true, true),
            TERRAIN_DECAL_HORDE_WITH_FANATICISM
        );
        assert_eq!(leftover_infantry_horde_decal_size(14.0, 12.0), 14.0);
        assert_eq!(leftover_infantry_horde_decal_size(0.0, 0.0), 0.0);
        assert!(!leftover_vehicle_horde_membership_due(0, 0, 30));
        assert!(leftover_vehicle_horde_membership_due(31, 0, 30));
        assert_eq!(leftover_horde_decal_fade(false, true), Some((1.0, 0.03)));
        assert_eq!(leftover_horde_decal_fade(true, false), Some((0.0, -0.03)));
        assert_eq!(leftover_horde_decal_fade(true, true), None);
    }

    #[test]
    fn horde_count_includes_self() {
        // 4 others + self = 5 → in horde
        assert!(is_in_horde(4));
        assert!(!is_in_horde(3));
        assert!(!is_in_horde(0));
        assert!(is_in_horde(5));
    }

    #[test]
    fn splash_radius_5() {
        assert!((battlemaster_splash_damage_at(true, 100.0, 60.0) - 60.0).abs() < 0.01);
        assert!((battlemaster_splash_damage_at(false, 4.0, 60.0) - 60.0).abs() < 0.01);
        assert!((battlemaster_splash_damage_at(false, 5.0, 75.0) - 75.0).abs() < 0.01);
        assert!((battlemaster_splash_damage_at(false, 5.1, 60.0)).abs() < 0.01);
    }

    #[test]
    fn nationalism_tag_detect() {
        let mut tags = HashSet::new();
        tags.insert(UPGRADE_NATIONALISM.to_string());
        assert!(has_nationalism_upgrade(&tags));
        tags.clear();
        tags.insert("Upgrade_ChinaNationalism".to_string());
        assert!(has_nationalism_upgrade(&tags));
    }

    #[test]
    fn counts_toward_horde_filters() {
        assert!(counts_toward_battlemaster_horde(
            true, true, true, true, 50.0, 75.0
        ));
        assert!(!counts_toward_battlemaster_horde(
            true, true, false, true, 50.0, 75.0
        ));
        assert!(!counts_toward_battlemaster_horde(
            true, true, true, true, 80.0, 75.0
        ));
        assert!(!counts_toward_battlemaster_horde(
            true, false, true, true, 10.0, 75.0
        ));
    }

    #[test]
    fn vehicle_horde_not_battlemaster_only() {
        assert!(is_china_vehicle_horde_unit("ChinaTankBattleMaster"));
        assert!(is_china_vehicle_horde_unit("ChinaTankDragon"));
        assert!(is_china_vehicle_horde_unit("ChinaVehicleInfernoCannon"));
        assert!(is_china_vehicle_horde_unit("ChinaTankGattling"));
        assert!(is_china_vehicle_horde_unit("ChinaTankOverlord"));
        assert!(!is_china_vehicle_horde_unit("OverlordGattlingCannon"));
        assert!(!is_china_vehicle_horde_unit("ChinaInfantryRedguard"));
        assert!(!same_vehicle_horde_family(
            "ChinaTankDragon",
            "Nuke_ChinaTankDragon"
        ));
        assert!(!same_vehicle_horde_family(
            "ChinaTankDragon",
            "ChinaTankBattleMaster"
        ));
        assert!(!same_vehicle_horde_family(
            "ChinaTankOverlord",
            "ChinaTankEmperor"
        ));
        assert!(!same_vehicle_horde_family(
            "ChinaTankBattleMaster",
            "Infa_ChinaTankBattleMaster"
        ));
        assert!(same_vehicle_horde_family(
            "ChinaTankBattleMaster",
            "ChinaTankBattleMaster"
        ));
    }

    #[test]
    fn rub_off_honorary_membership() {
        let units = [(0.0, 0.0, true), (10.0, 0.0, true), (15.0, 0.0, true)];
        // Three units all within radius of each other but Count=5 → no true members.
        let none = evaluate_leftover_horde_blob(&units, 5, 75.0, 20.0, |_, _, dist| dist <= 75.0);
        assert!(none.iter().all(|m| !m.in_horde && !m.true_member));

        // Five clustered + one 15 units away (inside RubOff of a true member).
        let blob = [
            (0.0, 0.0, true),
            (5.0, 0.0, true),
            (10.0, 0.0, true),
            (0.0, 5.0, true),
            (5.0, 5.0, true),
            (18.0, 0.0, true),
        ];
        let mem = evaluate_leftover_horde_blob(&blob, 5, 75.0, 20.0, |_, _, dist| dist <= 75.0);
        assert!(mem[0].true_member);
        assert!(mem[5].in_horde && !mem[5].true_member);

        let edge = evaluate_leftover_horde_membership(1, 5, Some(25.0), 20.0);
        assert!(!edge.in_horde);
    }

    #[test]
    fn horde_decal_matrix_core_transitions() {
        assert_eq!(
            leftover_horde_decal_type(true, false, false),
            TERRAIN_DECAL_HORDE
        );
        assert_eq!(
            leftover_horde_decal_type(false, false, false),
            TERRAIN_DECAL_HORDE_VEHICLE
        );
        assert_eq!(
            leftover_horde_decal_type(false, true, false),
            TERRAIN_DECAL_HORDE_WITH_NATIONALISM
        );
        assert_eq!(leftover_horde_decal_fade(false, true), Some((1.0, 0.03)));
        assert_eq!(leftover_horde_decal_fade(true, false), Some((0.0, -0.03)));
        assert_eq!(leftover_horde_decal_fade(true, true), None);
    }

    #[test]
    fn bounding_sphere_3d_count_includes_slack() {
        // Centers 80 apart: 2D fails Radius 75; sphere 13+13 makes boundary 54.
        let line: Vec<LeftoverHordeScanUnit> = [0.0, 20.0, 40.0, 60.0, 80.0]
            .into_iter()
            .map(|x| LeftoverHordeScanUnit {
                x,
                y: 0.0,
                z: 0.0,
                sphere_radius: 13.0,
                alive: true,
            })
            .collect();
        let with_sphere =
            evaluate_leftover_horde_blob_scan(&line, 5, 75.0, 20.0, |_, _, dist| dist <= 75.0);
        assert!(with_sphere.iter().all(|m| m.true_member));

        let no_slack: Vec<LeftoverHordeScanUnit> = [0.0, 20.0, 40.0, 60.0, 80.0]
            .into_iter()
            .map(|x| LeftoverHordeScanUnit::xz(x, 0.0, true))
            .collect();
        let without =
            evaluate_leftover_horde_blob_scan(&no_slack, 5, 75.0, 20.0, |_, _, dist| dist <= 75.0);
        assert!(!without[0].true_member);
        assert!((leftover_from_bounding_sphere_3d(&line[0], &line[4]) - 54.0).abs() < 0.01);
    }

    #[test]
    fn horde_flag_subobjects_hidden() {
        let vis = leftover_horde_flag_visibility_for_template("ChinaTankBattleMaster");
        assert!(vis.is_hidden("FLAG"));
        assert!(vis.is_hidden("HordeFlag"));
        let empty = leftover_horde_flag_visibility_for_template("AmericaTankCrusader");
        assert!(!empty.is_hidden("FLAG"));
        assert!((leftover_horde_major_radius(true, 13.0, 15.81) - 13.0).abs() < 0.01);
        assert!((leftover_vehicle_horde_decal_size(13.0) - 45.5).abs() < 0.01);
    }

    #[test]
    fn battlemaster_residual_pack_honesty_wave67() {
        assert!(honesty_battlemaster_weapon_residual_ok());
        assert!(honesty_battlemaster_horde_residual_ok());
        assert!(honesty_battlemaster_body_residual_ok());
        assert!(honesty_battlemaster_scatter_vs_infantry_ok());
        assert!(honesty_battlemaster_residual_pack_ok());
        assert_eq!(battlemaster_ms_to_frames(2_000), 60);
        assert_eq!(BATTLE_MASTER_BUILD_TIME_FRAMES, 300);
        assert_eq!(BATTLE_MASTER_PROJECTILE, "BattleMasterTankShell");
        assert!(BATTLE_MASTER_HORDE_EXACT_MATCH);
        assert!((BATTLE_MASTER_NUCLEAR_LOCOMOTOR_SPEED - 35.0).abs() < 0.01);
    }
}
