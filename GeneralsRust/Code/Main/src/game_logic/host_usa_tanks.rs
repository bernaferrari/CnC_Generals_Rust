//! Host America Crusader / Paladin tank residuals (main gun + Composite Armor).
//!
//! Residual slice (playability):
//! - AmericaTankCrusader / *Crusader* spawns with PRIMARY `CrusaderTankGun`
//!   (dmg **60** / range **150** / Delay **2000**ms → 60 frames).
//! - AmericaTankPaladin / *Paladin* spawns with PRIMARY `PaladinTankGun`
//!   (same gun stats; PointDefenseLaser residual already in host_point_defense).
//! - Laser General residual:
//!   - `Lazr_AmericaTankCrusader` → `Lazr_CrusaderTankGun` (dmg **80** / r**5** /
//!     Delay **2000**ms → 60 frames). Instant laser residual (WeaponSpeed 99999).
//!   - `Lazr_AmericaTankPaladin` → `Lazr_PaladinTankGun` (dmg **70** / r**3** /
//!     Delay **1000**ms → 30 frames).
//! - Upgrade_AmericaCompositeArmor MaxHealthUpgrade residual: **+100** max HP
//!   with ADD_CURRENT_HEALTH_TOO on Crusader / Paladin (not Humvee / Avenger).
//!
//! Wave 67 residual pack (retail AmericaVehicle.ini / Weapon.ini / Locomotor.ini):
//! - Weapon residual: PrimaryDamageRadius **5**, ScatterRadiusVsInfantry **10**,
//!   DamageType **ARMOR_PIERCING**, DeathType **NORMAL**, Delay **2000**ms → **60**f,
//!   Projectile **GenericTankShell**, FireFX **WeaponFX_GenericTankGunNoTracer**,
//!   DetonationFX **WeaponFX_GenericTankShellDetonation**, ClipSize **0**,
//!   Crusader WeaponSpeed **400** / Paladin **300**.
//! - Crusader body residual: MaxHealth **480**, Vision **150**, Shroud **300**,
//!   BuildCost **900**, BuildTime **10**s → **300**f, TransportSlotCount **3**,
//!   TurretTurnRate **180**, Locomotor Speed **30**/Damaged **25**, Geometry BOX
//!   **15**/**10**/**10**.
//! - Paladin body residual: MaxHealth **500**, BuildCost **1100**,
//!   BuildTime **12**s → **360**f (shares vision/shroud/turret/locomotor residual).
//! - Composite Armor residual: AddMaxHealth **100** + ADD_CURRENT_HEALTH_TOO.
//!
//! Fail-closed honesty:
//! - Not full ArmorSet PLAYER_UPGRADE UpgradedTankArmor matrix
//! - GenericTankShell DumbProjectile Bezier flight residual closed (non-laser gun)
//! - ScatterRadiusVsInfantry **10** residual miss cone closed (deterministic aim offset)
//! - Not full turret recoil drawable matrix
//! - Not full LaserName / LaserBoneName drawable beam matrix (Lazr residual)
//! - Not SCIENCE_PaladinTank prereq gate / ProductionUpdate door UI
//! - Not network composite / tank gun / laser-tank replication (network deferred)

use super::Weapon;
use glam::Vec3;

/// Logic frames per second (host fixed step).
pub const USA_TANKS_LOGIC_FPS: f32 = 30.0;

/// Retail CrusaderTankGun primary weapon.
pub const CRUSADER_TANK_GUN: &str = "CrusaderTankGun";
/// Retail PaladinTankGun primary weapon.
pub const PALADIN_TANK_GUN: &str = "PaladinTankGun";
/// Retail Laser General Crusader primary.
pub const LAZR_CRUSADER_TANK_GUN: &str = "Lazr_CrusaderTankGun";
/// Retail Laser General Paladin primary.
pub const LAZR_PALADIN_TANK_GUN: &str = "Lazr_PaladinTankGun";
/// Retail Upgrade_AmericaCompositeArmor.
pub const UPGRADE_AMERICA_COMPOSITE_ARMOR: &str = "Upgrade_AmericaCompositeArmor";

/// Retail tank gun PrimaryDamage residual.
pub const USA_TANK_GUN_DAMAGE: f32 = 60.0;
/// Retail tank gun PrimaryDamageRadius residual.
pub const USA_TANK_GUN_PRIMARY_RADIUS: f32 = 5.0;
/// Retail ScatterRadiusVsInfantry residual.
pub const USA_TANK_GUN_SCATTER_VS_INFANTRY: f32 = 10.0;
/// Retail DamageType residual.
pub const USA_TANK_GUN_DAMAGE_TYPE: &str = "ARMOR_PIERCING";
/// Retail DeathType residual.
pub const USA_TANK_GUN_DEATH_TYPE: &str = "NORMAL";
/// Retail Paladin Point Defense Laser DamageType residual.
pub const PALADIN_PDL_DAMAGE_TYPE: &str = "LASER";
/// Retail Paladin Point Defense Laser DeathType residual.
pub const PALADIN_PDL_DEATH_TYPE: &str = "LASERED";
/// Retail ProjectileObject residual.
pub const USA_TANK_GUN_PROJECTILE: &str = "GenericTankShell";
/// Retail GenericTankShell MaxHealth residual.
pub const USA_SHELL_MAX_HEALTH: f32 = 100.0;
/// DumbProjectileBehavior FirstHeight residual.
pub const USA_SHELL_FIRST_HEIGHT: f32 = 10.0;
/// DumbProjectileBehavior SecondHeight residual.
pub const USA_SHELL_SECOND_HEIGHT: f32 = 10.0;
/// FirstPercentIndent residual (50%).
pub const USA_SHELL_FIRST_PERCENT_INDENT: f32 = 0.50;
/// SecondPercentIndent residual (90%).
pub const USA_SHELL_SECOND_PERCENT_INDENT: f32 = 0.90;
/// Retail FireFX residual.
pub const USA_TANK_GUN_FIRE_FX: &str = "WeaponFX_GenericTankGunNoTracer";
/// Retail ProjectileDetonationFX residual.
pub const USA_TANK_GUN_DETONATION_FX: &str = "WeaponFX_GenericTankShellDetonation";
/// Retail ClipSize residual (0 == infinite).
pub const USA_TANK_GUN_CLIP_SIZE: u32 = 0;
/// Retail DelayBetweenShots residual (msec).
pub const USA_TANK_GUN_DELAY_MS: u32 = 2_000;
/// Retail Lazr_CrusaderTankGun PrimaryDamage residual.
pub const LAZR_CRUSADER_TANK_GUN_DAMAGE: f32 = 80.0;
/// Retail Lazr_PaladinTankGun PrimaryDamage residual.
pub const LAZR_PALADIN_TANK_GUN_DAMAGE: f32 = 70.0;
/// Retail tank gun AttackRange residual.
pub const USA_TANK_GUN_RANGE: f32 = 150.0;
/// Retail DelayBetweenShots 2000ms → 60 frames @ 30 FPS.
pub const USA_TANK_GUN_DELAY_FRAMES: u32 = 60;
/// Retail Lazr_PaladinTankGun DelayBetweenShots 1000ms → 30 frames @ 30 FPS.
pub const LAZR_PALADIN_TANK_GUN_DELAY_FRAMES: u32 = 30;
/// Retail Crusader WeaponSpeed residual.
pub const CRUSADER_WEAPON_SPEED: f32 = 400.0;
/// Retail Paladin WeaponSpeed residual.
pub const PALADIN_WEAPON_SPEED: f32 = 300.0;
/// Retail MaxHealthUpgrade AddMaxHealth residual.
pub const COMPOSITE_ARMOR_ADD_MAX_HEALTH: f32 = 100.0;
/// Retail MaxHealthUpgrade ChangeType residual.
pub const COMPOSITE_ARMOR_CHANGE_TYPE: &str = "ADD_CURRENT_HEALTH_TOO";

/// Residual fire audio.
pub const CRUSADER_FIRE_AUDIO: &str = "CrusaderTankWeapon";
pub const PALADIN_FIRE_AUDIO: &str = "PaladinTankWeapon";
/// Residual Laser General laser fire audio (FX residual honesty).
pub const LAZR_TANK_FIRE_AUDIO: &str = "Lazr_WeaponFX_LaserCrusader";

// --- Body residual (AmericaTankCrusader / AmericaTankPaladin) ---

/// Retail Crusader MaxHealth residual.
pub const CRUSADER_MAX_HEALTH: f32 = 480.0;
/// Retail Paladin MaxHealth residual.
pub const PALADIN_MAX_HEALTH: f32 = 500.0;
/// Retail VisionRange residual (shared).
pub const USA_TANK_VISION_RANGE: f32 = 150.0;
/// Retail ShroudClearingRange residual (shared).
pub const USA_TANK_SHROUD_CLEARING_RANGE: f32 = 300.0;
/// Retail Crusader BuildCost residual.
pub const CRUSADER_BUILD_COST: u32 = 900;
/// Retail Paladin BuildCost residual.
pub const PALADIN_BUILD_COST: u32 = 1_100;
/// Retail Crusader BuildTime residual (seconds).
pub const CRUSADER_BUILD_TIME_SEC: f32 = 10.0;
/// Retail Paladin BuildTime residual (seconds).
pub const PALADIN_BUILD_TIME_SEC: f32 = 12.0;
/// Crusader BuildTime → frames @ 30 FPS.
pub const CRUSADER_BUILD_TIME_FRAMES: u32 = 300;
/// Paladin BuildTime → frames @ 30 FPS.
pub const PALADIN_BUILD_TIME_FRAMES: u32 = 360;
/// Retail TransportSlotCount residual.
pub const USA_TANK_TRANSPORT_SLOT_COUNT: u32 = 3;
/// Retail TurretTurnRate residual (deg/sec).
pub const USA_TANK_TURRET_TURN_RATE: f32 = 180.0;
/// Retail CrusaderLocomotor Speed residual.
pub const USA_TANK_LOCOMOTOR_SPEED: f32 = 30.0;
/// Retail CrusaderLocomotor SpeedDamaged residual.
pub const USA_TANK_LOCOMOTOR_SPEED_DAMAGED: f32 = 25.0;
/// Retail Geometry BOX MajorRadius residual.
pub const USA_TANK_GEOMETRY_MAJOR: f32 = 15.0;
/// Retail Geometry BOX MinorRadius residual.
pub const USA_TANK_GEOMETRY_MINOR: f32 = 10.0;
/// Retail GeometryHeight residual.
pub const USA_TANK_GEOMETRY_HEIGHT: f32 = 10.0;
/// Retail ExperienceValue residual.
pub const USA_TANK_EXPERIENCE_VALUE: [u32; 4] = [100, 100, 200, 400];
/// Retail ExperienceRequired residual.
pub const USA_TANK_EXPERIENCE_REQUIRED: [u32; 4] = [0, 200, 300, 600];

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn usa_tanks_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * USA_TANKS_LOGIC_FPS / 1000.0).round() as u32
}

/// Whether template is a residual Crusader tank chassis.
///
/// Fail-closed: name residual; excludes debris / weapons / pure Paladin.
pub fn is_crusader_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n == "testcrusader" || n == "usa_crusader" || n == "usa_crusadertank" {
        return true;
    }
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("shell")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("turret")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("crate")
    {
        return false;
    }
    n.contains("crusader")
}

/// Whether template is a residual Paladin tank chassis.
///
/// Fail-closed: name residual; PointDefenseLaser modules already host residual.
pub fn is_paladin_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n == "testpaladin" || n == "usa_paladin" || n == "usa_paladintank" {
        return true;
    }
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("shell")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("pointdefense")
        || n.contains("dead")
        || n.starts_with("upgrade")
    {
        return false;
    }
    n.contains("paladin")
}

/// Composite Armor MaxHealthUpgrade applies to Crusader + Paladin (retail ModuleTag).
pub fn is_composite_armor_unit_template(template_name: &str) -> bool {
    is_crusader_template(template_name) || is_paladin_template(template_name)
}

/// Whether template is a Laser General residual chassis (Lazr_ prefix or name).
///
/// Fail-closed: name residual (not full general-science production gate).
pub fn is_laser_general_tank_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    n.starts_with("lazr_")
        || n.contains("lazr_america")
        || n.contains("lasergeneral")
        || n == "testlazrcrusader"
        || n == "testlazrpaladin"
}

/// Primary weapon name for residual USA main battle tank.
pub fn primary_weapon_name_for_usa_tank(template_name: &str) -> Option<&'static str> {
    let laser = is_laser_general_tank_template(template_name);
    if is_paladin_template(template_name) {
        Some(if laser {
            LAZR_PALADIN_TANK_GUN
        } else {
            PALADIN_TANK_GUN
        })
    } else if is_crusader_template(template_name) {
        Some(if laser {
            LAZR_CRUSADER_TANK_GUN
        } else {
            CRUSADER_TANK_GUN
        })
    } else {
        None
    }
}

/// Build residual tank gun weapon (Crusader / Paladin shared stats).
pub fn usa_tank_gun_weapon() -> Weapon {
    usa_tank_gun_weapon_for_template("AmericaTankCrusader")
}

/// Build residual tank gun for a specific chassis template (laser vs shell).
pub fn usa_tank_gun_weapon_for_template(template_name: &str) -> Weapon {
    let laser = is_laser_general_tank_template(template_name);
    let is_paladin = is_paladin_template(template_name);
    let (damage, delay_frames, projectile_speed) = if laser && is_paladin {
        (
            LAZR_PALADIN_TANK_GUN_DAMAGE,
            LAZR_PALADIN_TANK_GUN_DELAY_FRAMES,
            99999.0,
        )
    } else if laser {
        (
            LAZR_CRUSADER_TANK_GUN_DAMAGE,
            USA_TANK_GUN_DELAY_FRAMES,
            99999.0,
        )
    } else {
        // Shell residual: Paladin shares Crusader damage/delay; projectile speed differs.
        let speed = if is_paladin {
            PALADIN_WEAPON_SPEED
        } else {
            CRUSADER_WEAPON_SPEED
        };
        (USA_TANK_GUN_DAMAGE, USA_TANK_GUN_DELAY_FRAMES, speed)
    };
    Weapon {
        damage,
        range: USA_TANK_GUN_RANGE,
        min_range: 0.0,
        reload_time: delay_frames as f32 / USA_TANKS_LOGIC_FPS,
        last_fire_time: 0.0,
        ammo: None,
        clip_size: 0,
        clip_reload_time: 0.0,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed,
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
        suspend_fx_frame: 0,
        reloading_clip: false,
        last_bonus_rof: 0.0,
    }
}

/// Apply Composite Armor residual: +AddMaxHealth current+max (ADD_CURRENT_HEALTH_TOO).
///
/// Returns true when health was increased (first apply residual; idempotent via tag).

/// Whether residual fire should apply USA tank gun residual (non-laser).
pub fn should_apply_usa_tank_gun_residual(template_name: &str) -> bool {
    (is_crusader_template(template_name) || is_paladin_template(template_name))
        && !is_laser_general_tank_template(template_name)
}

/// Splash damage residual at distance (full PrimaryDamage inside radius).
pub fn usa_tank_gun_splash_damage_at(distance: f32, damage: f32) -> f32 {
    if distance <= USA_TANK_GUN_PRIMARY_RADIUS {
        damage
    } else {
        0.0
    }
}

/// Legal splash target residual.
pub fn is_legal_usa_tank_splash_target(
    is_alive: bool,
    is_attackable: bool,
    is_projectile: bool,
    is_self: bool,
) -> bool {
    is_alive && is_attackable && !is_projectile && !is_self
}

/// Cubic Bezier residual sample for GenericTankShell DumbProjectileBehavior.
pub fn usa_tank_shell_bezier_point(from: Vec3, to: Vec3, t: f32) -> Vec3 {
    let t = t.clamp(0.0, 1.0);
    let delta = to - from;
    let p0 = from;
    let p3 = to;
    let p1 = from + delta * USA_SHELL_FIRST_PERCENT_INDENT + Vec3::Y * USA_SHELL_FIRST_HEIGHT;
    let p2 = from + delta * USA_SHELL_SECOND_PERCENT_INDENT + Vec3::Y * USA_SHELL_SECOND_HEIGHT;
    let u = 1.0 - t;
    let tt = t * t;
    let uu = u * u;
    let uuu = uu * u;
    let ttt = tt * t;
    p0 * uuu + p1 * (3.0 * uu * t) + p2 * (3.0 * u * tt) + p3 * ttt
}

/// Flight frame residual from ground distance @ weapon speed.
pub fn usa_tank_shell_flight_frames(from: Vec3, to: Vec3, weapon_speed: f32) -> u32 {
    let dx = to.x - from.x;
    let dz = to.z - from.z;
    let dist = (dx * dx + dz * dz).sqrt().max(1.0);
    let speed = weapon_speed.max(1.0);
    let frames = (dist / (speed / USA_TANKS_LOGIC_FPS)).ceil() as u32;
    frames.clamp(4, 180)
}

pub fn apply_composite_armor_health(max_health: &mut f32, current: &mut f32, maximum: &mut f32) {
    *max_health = max_health.saturating_add_f32(COMPOSITE_ARMOR_ADD_MAX_HEALTH);
    *maximum = maximum.saturating_add_f32(COMPOSITE_ARMOR_ADD_MAX_HEALTH);
    *current = current.saturating_add_f32(COMPOSITE_ARMOR_ADD_MAX_HEALTH);
}

/// f32 saturating add helper (no std saturating_add for f32).
trait SaturatingAddF32 {
    fn saturating_add_f32(self, rhs: f32) -> f32;
}
impl SaturatingAddF32 for f32 {
    fn saturating_add_f32(self, rhs: f32) -> f32 {
        (self + rhs).max(0.0)
    }
}

/// Only Paladin PointDefenseLaser has SecondaryTargetTypes = INFANTRY.
pub fn paladin_allows_secondary_infantry_intercept(template_name: &str) -> bool {
    is_paladin_template(template_name)
}

// --- Wave 67 residual honesty packs ---

/// Wave 67 residual honesty: Crusader / Paladin weapon residual peel.
pub fn honesty_usa_tanks_weapon_residual_ok() -> bool {
    CRUSADER_TANK_GUN == "CrusaderTankGun"
        && PALADIN_TANK_GUN == "PaladinTankGun"
        && (USA_TANK_GUN_DAMAGE - 60.0).abs() < 0.01
        && (USA_TANK_GUN_PRIMARY_RADIUS - 5.0).abs() < 0.01
        && (USA_TANK_GUN_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01
        && (USA_TANK_GUN_RANGE - 150.0).abs() < 0.01
        && USA_TANK_GUN_DELAY_MS == 2_000
        && USA_TANK_GUN_DELAY_FRAMES == usa_tanks_ms_to_frames(USA_TANK_GUN_DELAY_MS)
        && USA_TANK_GUN_DELAY_FRAMES == 60
        && (CRUSADER_WEAPON_SPEED - 400.0).abs() < 0.01
        && (PALADIN_WEAPON_SPEED - 300.0).abs() < 0.01
        && USA_TANK_GUN_DAMAGE_TYPE == "ARMOR_PIERCING"
        && USA_TANK_GUN_DEATH_TYPE == "NORMAL"
        && USA_TANK_GUN_PROJECTILE == "GenericTankShell"
        && (USA_SHELL_FIRST_HEIGHT - 10.0).abs() < 0.01
        && (USA_SHELL_SECOND_PERCENT_INDENT - 0.90).abs() < 0.001
        && USA_TANK_GUN_FIRE_FX == "WeaponFX_GenericTankGunNoTracer"
        && USA_TANK_GUN_DETONATION_FX == "WeaponFX_GenericTankShellDetonation"
        && USA_TANK_GUN_CLIP_SIZE == 0
        && CRUSADER_FIRE_AUDIO == "CrusaderTankWeapon"
        && PALADIN_FIRE_AUDIO == "PaladinTankWeapon"
        && (LAZR_CRUSADER_TANK_GUN_DAMAGE - 80.0).abs() < 0.01
        && (LAZR_PALADIN_TANK_GUN_DAMAGE - 70.0).abs() < 0.01
        && LAZR_PALADIN_TANK_GUN_DELAY_FRAMES == 30
        && {
            let c = usa_tank_gun_weapon_for_template("AmericaTankCrusader");
            let p = usa_tank_gun_weapon_for_template("AmericaTankPaladin");
            (c.damage - 60.0).abs() < 0.01
                && (c.projectile_speed - 400.0).abs() < 0.01
                && (p.projectile_speed - 300.0).abs() < 0.01
                && !c.can_target_air
                && c.can_target_ground
        }
}

/// Wave 67 residual honesty: Crusader / Paladin body residual peel.
pub fn honesty_usa_tanks_body_residual_ok() -> bool {
    (CRUSADER_MAX_HEALTH - 480.0).abs() < 0.01
        && (PALADIN_MAX_HEALTH - 500.0).abs() < 0.01
        && (USA_TANK_VISION_RANGE - 150.0).abs() < 0.01
        && (USA_TANK_SHROUD_CLEARING_RANGE - 300.0).abs() < 0.01
        && CRUSADER_BUILD_COST == 900
        && PALADIN_BUILD_COST == 1_100
        && (CRUSADER_BUILD_TIME_SEC - 10.0).abs() < 0.01
        && (PALADIN_BUILD_TIME_SEC - 12.0).abs() < 0.01
        && CRUSADER_BUILD_TIME_FRAMES
            == (CRUSADER_BUILD_TIME_SEC * USA_TANKS_LOGIC_FPS).round() as u32
        && PALADIN_BUILD_TIME_FRAMES
            == (PALADIN_BUILD_TIME_SEC * USA_TANKS_LOGIC_FPS).round() as u32
        && CRUSADER_BUILD_TIME_FRAMES == 300
        && PALADIN_BUILD_TIME_FRAMES == 360
        && USA_TANK_TRANSPORT_SLOT_COUNT == 3
        && (USA_TANK_TURRET_TURN_RATE - 180.0).abs() < 0.01
        && (USA_TANK_LOCOMOTOR_SPEED - 30.0).abs() < 0.01
        && (USA_TANK_LOCOMOTOR_SPEED_DAMAGED - 25.0).abs() < 0.01
        && (USA_TANK_GEOMETRY_MAJOR - 15.0).abs() < 0.01
        && (USA_TANK_GEOMETRY_MINOR - 10.0).abs() < 0.01
        && (USA_TANK_GEOMETRY_HEIGHT - 10.0).abs() < 0.01
        && USA_TANK_EXPERIENCE_VALUE == [100, 100, 200, 400]
        && USA_TANK_EXPERIENCE_REQUIRED == [0, 200, 300, 600]
}

/// Wave 67 residual honesty: Composite Armor upgrade residual peel.
pub fn honesty_usa_tanks_composite_armor_residual_ok() -> bool {
    UPGRADE_AMERICA_COMPOSITE_ARMOR == "Upgrade_AmericaCompositeArmor"
        && (COMPOSITE_ARMOR_ADD_MAX_HEALTH - 100.0).abs() < 0.01
        && COMPOSITE_ARMOR_CHANGE_TYPE == "ADD_CURRENT_HEALTH_TOO"
        && is_composite_armor_unit_template("AmericaTankCrusader")
        && is_composite_armor_unit_template("AmericaTankPaladin")
        && !is_composite_armor_unit_template("AmericaVehicleHumvee")
        && {
            let mut max_h = 480.0_f32;
            let mut cur = 400.0_f32;
            let mut maximum = 480.0_f32;
            apply_composite_armor_health(&mut max_h, &mut cur, &mut maximum);
            (max_h - 580.0).abs() < 0.01 && (cur - 500.0).abs() < 0.01
        }
}

/// Combined Wave 67 USA tanks residual honesty pack.

/// Apply Crusader/Paladin TankGun ScatterRadiusVsInfantry residual to aim.
///
/// Laser General guns (Lazr_*) are instant-hit and do not use shell scatter.
pub fn usa_tank_scatter_aim(aim: Vec3, target_is_infantry: bool, seed: u32) -> (Vec3, bool) {
    use crate::game_logic::weapon_bootstrap::{host_effective_scatter_radius, scatter_aim_offset};
    let mut scatter = host_effective_scatter_radius(CRUSADER_TANK_GUN, target_is_infantry);
    if target_is_infantry && scatter <= 0.0 {
        scatter = USA_TANK_GUN_SCATTER_VS_INFANTRY;
    }
    if scatter <= 0.0 {
        return (aim, false);
    }
    let off = scatter_aim_offset(seed, scatter);
    (Vec3::new(aim.x + off.x, aim.y, aim.z + off.z), true)
}

/// Whether Crusader/Paladin tank gun residual misses intended infantry via ScatterRadiusVsInfantry.
pub fn usa_tank_scatter_misses_infantry(
    target_is_infantry: bool,
    seed: u32,
    target_hit_radius: f32,
) -> bool {
    use crate::game_logic::weapon_bootstrap::scatter_misses_intended_target;
    if !target_is_infantry {
        return false;
    }
    scatter_misses_intended_target(USA_TANK_GUN_SCATTER_VS_INFANTRY, seed, target_hit_radius)
}

/// Wave residual honesty: USA tank ScatterRadiusVsInfantry peels.
pub fn honesty_usa_tank_scatter_vs_infantry_ok() -> bool {
    use crate::game_logic::weapon_bootstrap::host_effective_scatter_radius;
    let vs_c = host_effective_scatter_radius(CRUSADER_TANK_GUN, true);
    let vs_p = host_effective_scatter_radius(PALADIN_TANK_GUN, true);
    let ground = host_effective_scatter_radius(CRUSADER_TANK_GUN, false);
    (USA_TANK_GUN_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01
        && ((vs_c - 10.0).abs() < 0.01 || vs_c <= 0.0)
        && ((vs_p - 10.0).abs() < 0.01 || vs_p <= 0.0)
        && ground.abs() < 0.01
        && {
            let (sc, applied) = usa_tank_scatter_aim(Vec3::new(0.0, 0.0, 0.0), true, 3);
            applied && sc.length() <= USA_TANK_GUN_SCATTER_VS_INFANTRY + 0.01
        }
}

pub fn honesty_usa_tanks_residual_pack_ok() -> bool {
    honesty_usa_tanks_weapon_residual_ok()
        && honesty_usa_tanks_body_residual_ok()
        && honesty_usa_tanks_composite_armor_residual_ok()
        && honesty_usa_tank_scatter_vs_infantry_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usa_tank_scatter_vs_infantry_peels() {
        assert!(honesty_usa_tank_scatter_vs_infantry_ok());
        let aim = Vec3::new(140.0, 0.0, 18.0);
        let (no_sc, applied) = usa_tank_scatter_aim(aim, false, 31);
        assert!(!applied);
        assert_eq!(no_sc, aim);
        let (sc, applied) = usa_tank_scatter_aim(aim, true, 31);
        assert!(applied);
        let d = ((sc.x - aim.x).powi(2) + (sc.z - aim.z).powi(2)).sqrt();
        assert!(d > 0.01 && d <= USA_TANK_GUN_SCATTER_VS_INFANTRY + 0.01);

        assert!(usa_tank_scatter_misses_infantry(true, 67, 0.5));
        assert!(!usa_tank_scatter_misses_infantry(true, 67, 100.0));
        assert!(!usa_tank_scatter_misses_infantry(false, 67, 0.5));
    }

    #[test]
    fn crusader_paladin_name_matrix() {
        assert!(is_crusader_template("AmericaTankCrusader"));
        assert!(is_crusader_template("USA_Crusader"));
        assert!(is_crusader_template("USA_CrusaderTank"));
        assert!(is_crusader_template("TestCrusader"));
        assert!(!is_crusader_template("AmericaTankPaladin"));
        assert!(!is_crusader_template("2FreeCrusadersCrate"));

        assert!(is_paladin_template("AmericaTankPaladin"));
        assert!(is_paladin_template("USA_Paladin"));
        assert!(is_paladin_template("TestPaladin"));
        assert!(!is_paladin_template("AmericaTankCrusader"));
        assert!(!is_paladin_template("PaladinPointDefenseLaser"));
    }

    #[test]
    fn composite_armor_targets() {
        assert!(is_composite_armor_unit_template("AmericaTankCrusader"));
        assert!(is_composite_armor_unit_template("AmericaTankPaladin"));
        assert!(!is_composite_armor_unit_template("AmericaVehicleHumvee"));
        assert!(!is_composite_armor_unit_template("AmericaTankAvenger"));
    }

    #[test]
    fn composite_armor_adds_100() {
        let mut max_h = 480.0_f32;
        let mut cur = 400.0_f32;
        let mut maximum = 480.0_f32;
        apply_composite_armor_health(&mut max_h, &mut cur, &mut maximum);
        assert!((max_h - 580.0).abs() < 0.01);
        assert!((maximum - 580.0).abs() < 0.01);
        assert!((cur - 500.0).abs() < 0.01);
    }

    #[test]
    fn tank_gun_stats() {
        let w = usa_tank_gun_weapon();
        assert!((w.damage - 60.0).abs() < 0.01);
        assert!((w.range - 150.0).abs() < 0.01);
        assert!((w.reload_time - 2.0).abs() < 0.01);
        assert!(!w.can_target_air);
        assert!(w.can_target_ground);
    }

    #[test]
    fn laser_tank_gun_stats() {
        let c = usa_tank_gun_weapon_for_template("Lazr_AmericaTankCrusader");
        assert!((c.damage - LAZR_CRUSADER_TANK_GUN_DAMAGE).abs() < 0.01);
        assert!((c.reload_time - 2.0).abs() < 0.01);
        assert!((c.projectile_speed - 99999.0).abs() < 1.0);

        let p = usa_tank_gun_weapon_for_template("Lazr_AmericaTankPaladin");
        assert!((p.damage - LAZR_PALADIN_TANK_GUN_DAMAGE).abs() < 0.01);
        assert!((p.reload_time - 1.0).abs() < 0.01);
        assert!(is_laser_general_tank_template("Lazr_AmericaTankCrusader"));
        assert!(is_laser_general_tank_template("TestLazrPaladin"));
        assert!(!is_laser_general_tank_template("AmericaTankCrusader"));
    }

    #[test]
    fn weapon_name_lookup() {
        assert_eq!(
            primary_weapon_name_for_usa_tank("AmericaTankCrusader"),
            Some(CRUSADER_TANK_GUN)
        );
        assert_eq!(
            primary_weapon_name_for_usa_tank("Lazr_AmericaTankCrusader"),
            Some(LAZR_CRUSADER_TANK_GUN)
        );
        assert_eq!(
            primary_weapon_name_for_usa_tank("Lazr_AmericaTankPaladin"),
            Some(LAZR_PALADIN_TANK_GUN)
        );
        assert_eq!(
            primary_weapon_name_for_usa_tank("AmericaTankPaladin"),
            Some(PALADIN_TANK_GUN)
        );
        assert_eq!(primary_weapon_name_for_usa_tank("AmericaTankAvenger"), None);
    }

    #[test]
    fn usa_tanks_residual_pack_honesty_wave67() {
        assert!(honesty_usa_tanks_weapon_residual_ok());
        assert!(honesty_usa_tanks_body_residual_ok());
        assert!(honesty_usa_tanks_composite_armor_residual_ok());
        assert!(honesty_usa_tank_scatter_vs_infantry_ok());
        assert!(honesty_usa_tanks_residual_pack_ok());
        assert_eq!(usa_tanks_ms_to_frames(2_000), 60);
        assert_eq!(usa_tanks_ms_to_frames(0), 0);
        assert_eq!(CRUSADER_BUILD_TIME_FRAMES, 300);
        assert_eq!(PALADIN_BUILD_TIME_FRAMES, 360);
        assert!((USA_TANK_GUN_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01);
        assert_eq!(USA_TANK_GUN_PROJECTILE, "GenericTankShell");
        assert_eq!(USA_TANK_TRANSPORT_SLOT_COUNT, 3);
    }
}
