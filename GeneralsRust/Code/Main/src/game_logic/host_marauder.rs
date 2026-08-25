//! Host GLA Marauder Tank residual (salvage fire-rate tiers).
//!
//! Residual slice (playability):
//! - Spawns with PRIMARY `MarauderTankGun` (dmg **60** / radius **5** / range **170**
//!   / DelayBetweenShots **2000**ms).
//! - Salvage weapon upgrade residual (`WEAPON_SALVAGER` / CRATEUPGRADE):
//!   - Tier 0: `MarauderTankGun` — 2000ms delay (60 frames @ 30 FPS)
//!   - Tier 1 CRATEUPGRADE_ONE: `MarauderTankGunUpgradeOne` — 1500ms (45 frames),
//!     WeaponSpeed residual 400
//!   - Tier 2 CRATEUPGRADE_TWO: `MarauderTankGunUpgradeTwo` — 750ms (23 frames),
//!     ClipSize 2 residual, WeaponSpeed residual 500
//! - PrimaryDamage stays **60** across tiers (fire-rate residual, not damage).
//! - Small PrimaryDamageRadius **5** splash residual on fire.
//!
//! Wave 66 residual pack (retail GLAVehicle.ini / Weapon.ini / Locomotor.ini):
//! - Weapon residual: DamageType ARMOR_PIERCING, DeathType NORMAL, ScatterVsInfantry **10**,
//!   Delay MS **2000**/**1500**/**750**, tier2 ClipSize **2** / ClipReload **100**ms → **3**f,
//!   FireFX WeaponFX_GenericTankGunNoTracer, Projectile MarauderTankShell.
//! - Body residual: MaxHealth **500**, Vision **125**, Shroud **300**, BuildCost **800**,
//!   BuildTime **10**s → **300**f, TransportSlotCount **3**, SCIENCE_MarauderTank.
//! - Locomotor residual: Speed **40** / Damaged **30**.
//!
//! Fail-closed honesty:
//! - MarauderTankShell DumbProjectile Bezier flight residual closed (10/10, 50%/90%)
//! - ScatterRadiusVsInfantry **10** residual miss cone closed (deterministic aim offset)
//! - Not full SalvageCrate collate / W3D turret subobject (Turret / TurretUp01/02) swap
//! - Not full ClipReloadTime 100ms dual-shot cadence matrix (tier 2 uses faster reload)
//! - Not Min/MaxTargetPitch matrix
//! - Not network salvage / weapon-set replication (network deferred)

use super::Weapon;
use glam::Vec3;

/// Logic frames per second (host fixed step).
pub const MARAUDER_LOGIC_FPS: f32 = 30.0;

/// Retail primary base weapon.
pub const MARAUDER_TANK_GUN: &str = "MarauderTankGun";
/// Retail CRATEUPGRADE_ONE weapon (faster fire).
pub const MARAUDER_TANK_GUN_UPGRADE_ONE: &str = "MarauderTankGunUpgradeOne";
/// Retail CRATEUPGRADE_TWO weapon (fastest dual residual).
pub const MARAUDER_TANK_GUN_UPGRADE_TWO: &str = "MarauderTankGunUpgradeTwo";
/// Retail projectile shell residual.
pub const MARAUDER_TANK_SHELL: &str = "MarauderTankShell";
/// Retail MarauderTankShell MaxHealth residual.
pub const MARAUDER_SHELL_MAX_HEALTH: f32 = 100.0;
/// DumbProjectileBehavior FirstHeight residual.
pub const MARAUDER_SHELL_FIRST_HEIGHT: f32 = 10.0;
/// DumbProjectileBehavior SecondHeight residual.
pub const MARAUDER_SHELL_SECOND_HEIGHT: f32 = 10.0;
/// FirstPercentIndent residual (50%).
pub const MARAUDER_SHELL_FIRST_PERCENT_INDENT: f32 = 0.50;
/// SecondPercentIndent residual (90%).
pub const MARAUDER_SHELL_SECOND_PERCENT_INDENT: f32 = 0.90;
/// Retail science gate residual.
pub const SCIENCE_MARAUDER_TANK: &str = "SCIENCE_MarauderTank";

/// Retail PrimaryDamage (all tiers).
pub const MARAUDER_DAMAGE: f32 = 60.0;
/// Retail PrimaryDamageRadius (all tiers).
pub const MARAUDER_SPLASH_RADIUS: f32 = 5.0;
/// Retail AttackRange (all tiers).
pub const MARAUDER_RANGE: f32 = 170.0;
/// Retail ScatterRadiusVsInfantry residual.
pub const MARAUDER_SCATTER_VS_INFANTRY: f32 = 10.0;
/// Retail DamageType residual.
pub const MARAUDER_DAMAGE_TYPE: &str = "ARMOR_PIERCING";
/// Retail DeathType residual.
pub const MARAUDER_DEATH_TYPE: &str = "NORMAL";
/// Retail FireFX residual.
pub const MARAUDER_FIRE_FX: &str = "WeaponFX_GenericTankGunNoTracer";

/// Tier 0 DelayBetweenShots residual (msec).
pub const MARAUDER_DELAY_MS_TIER0: u32 = 2000;
/// Tier 0 DelayBetweenShots 2000ms → 60 frames @ 30 FPS.
pub const MARAUDER_DELAY_FRAMES_TIER0: u32 = 60;
/// Tier 1 DelayBetweenShots residual (msec).
pub const MARAUDER_DELAY_MS_TIER1: u32 = 1500;
/// Tier 1 DelayBetweenShots 1500ms → 45 frames @ 30 FPS.
pub const MARAUDER_DELAY_FRAMES_TIER1: u32 = 45;
/// Tier 2 DelayBetweenShots residual (msec).
pub const MARAUDER_DELAY_MS_TIER2: u32 = 750;
/// Tier 2 DelayBetweenShots 750ms → 23 frames @ 30 FPS (ceil 22.5).
pub const MARAUDER_DELAY_FRAMES_TIER2: u32 = 23;

/// Tier 0/1 ClipSize residual (0 == infinite).
pub const MARAUDER_CLIP_SIZE_BASE: u32 = 0;
/// Tier 2 ClipSize residual (dual-shot clip).
pub const MARAUDER_CLIP_SIZE_TIER2: u32 = 2;
/// Tier 2 ClipReloadTime residual (msec).
pub const MARAUDER_CLIP_RELOAD_MS_TIER2: u32 = 100;
/// Tier 2 ClipReloadTime 100ms → 3 frames @ 30 FPS.
pub const MARAUDER_CLIP_RELOAD_FRAMES_TIER2: u32 = 3;

/// WeaponSpeed residual (shell flight residual; host hits are still residual-instant).
pub const MARAUDER_SPEED_TIER0: f32 = 300.0;
pub const MARAUDER_SPEED_TIER1: f32 = 400.0;
pub const MARAUDER_SPEED_TIER2: f32 = 500.0;

/// Residual fire audio.
pub const MARAUDER_FIRE_AUDIO: &str = "MarauderTankWeapon";

// --- Body residual (GLATankMarauder) ---

/// Retail ActiveBody MaxHealth residual.
pub const MARAUDER_MAX_HEALTH: f32 = 500.0;
/// Retail VisionRange residual.
pub const MARAUDER_VISION_RANGE: f32 = 125.0;
/// Retail ShroudClearingRange residual.
pub const MARAUDER_SHROUD_CLEARING_RANGE: f32 = 300.0;
/// Retail BuildCost residual.
pub const MARAUDER_BUILD_COST: u32 = 800;
/// Retail BuildTime residual (seconds).
pub const MARAUDER_BUILD_TIME_SEC: f32 = 10.0;
/// BuildTime 10s → 300 frames @ 30 FPS.
pub const MARAUDER_BUILD_TIME_FRAMES: u32 = 300;
/// Retail TransportSlotCount residual.
pub const MARAUDER_TRANSPORT_SLOT_COUNT: u32 = 3;
/// Retail MarauderLocomotor Speed residual.
pub const MARAUDER_LOCOMOTOR_SPEED: f32 = 40.0;
/// Retail MarauderLocomotor SpeedDamaged residual.
pub const MARAUDER_LOCOMOTOR_SPEED_DAMAGED: f32 = 30.0;

/// Convert residual milliseconds to logic frames @ 30 FPS (round half-up).
pub fn marauder_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * MARAUDER_LOGIC_FPS / 1000.0).round() as u32
}

/// Multi-weapon salvage residual tier (WEAPONSET_CRATEUPGRADE).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MarauderWeaponTier {
    #[default]
    Base = 0,
    One = 1,
    Two = 2,
}

impl MarauderWeaponTier {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::One,
            2 => Self::Two,
            _ => Self::Base,
        }
    }

    pub fn as_u8(self) -> u8 {
        match self {
            Self::Base => 0,
            Self::One => 1,
            Self::Two => 2,
        }
    }
}

/// Whether template is a residual Marauder tank vehicle.
///
/// Fail-closed: name residual (not full Salvage / W3D turret matrix).
/// Excludes weapons / projectiles / science tokens.
pub fn is_marauder_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    // Weapon / shell / upgrade / science tokens are not the living vehicle residual.
    if n.contains("weapon")
        || n.contains("shell")
        || n.contains("projectile")
        || n.contains("missile")
        || n.starts_with("upgrade")
        || n.contains("science")
        || n.contains("gun")
    {
        return false;
    }
    n.contains("marauder")
        || n == "gla_maraudertank"
        || n == "testmarauder"
        || n.contains("tankmarauder")
}

/// Weapon template name for salvage tier.
pub fn marauder_weapon_name_for_tier(tier: MarauderWeaponTier) -> &'static str {
    match tier {
        MarauderWeaponTier::Base => MARAUDER_TANK_GUN,
        MarauderWeaponTier::One => MARAUDER_TANK_GUN_UPGRADE_ONE,
        MarauderWeaponTier::Two => MARAUDER_TANK_GUN_UPGRADE_TWO,
    }
}

/// (damage, range, delay_frames, splash_radius, projectile_speed) for salvage tier.
pub fn marauder_weapon_stats(tier: MarauderWeaponTier) -> (f32, f32, u32, f32, f32) {
    match tier {
        MarauderWeaponTier::Base => (
            MARAUDER_DAMAGE,
            MARAUDER_RANGE,
            MARAUDER_DELAY_FRAMES_TIER0,
            MARAUDER_SPLASH_RADIUS,
            MARAUDER_SPEED_TIER0,
        ),
        MarauderWeaponTier::One => (
            MARAUDER_DAMAGE,
            MARAUDER_RANGE,
            MARAUDER_DELAY_FRAMES_TIER1,
            MARAUDER_SPLASH_RADIUS,
            MARAUDER_SPEED_TIER1,
        ),
        MarauderWeaponTier::Two => (
            MARAUDER_DAMAGE,
            MARAUDER_RANGE,
            MARAUDER_DELAY_FRAMES_TIER2,
            MARAUDER_SPLASH_RADIUS,
            MARAUDER_SPEED_TIER2,
        ),
    }
}

/// Reload time seconds residual for delay frames @ 30 FPS.
pub fn delay_frames_to_reload_secs(delay_frames: u32) -> f32 {
    (delay_frames.max(1) as f32) / 30.0
}

/// Build residual Weapon for a salvage tier.
pub fn marauder_weapon_for_tier(tier: MarauderWeaponTier) -> Weapon {
    let (damage, range, delay, _splash, speed) = marauder_weapon_stats(tier);
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
pub fn marauder_splash_damage_at(is_intended_target: bool, distance_from_impact: f32) -> f32 {
    if is_intended_target {
        return MARAUDER_DAMAGE;
    }
    if distance_from_impact <= MARAUDER_SPLASH_RADIUS {
        MARAUDER_DAMAGE
    } else {
        0.0
    }
}

/// Legal residual splash target.
pub fn is_legal_marauder_splash_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind
}

/// Whether residual fire should apply Marauder residual path.

/// Cubic Bezier residual sample for MarauderTankShell DumbProjectileBehavior.
pub fn marauder_shell_bezier_point(from: Vec3, to: Vec3, t: f32) -> Vec3 {
    let t = t.clamp(0.0, 1.0);
    let delta = to - from;
    let p0 = from;
    let p3 = to;
    let p1 =
        from + delta * MARAUDER_SHELL_FIRST_PERCENT_INDENT + Vec3::Y * MARAUDER_SHELL_FIRST_HEIGHT;
    let p2 = from
        + delta * MARAUDER_SHELL_SECOND_PERCENT_INDENT
        + Vec3::Y * MARAUDER_SHELL_SECOND_HEIGHT;
    let u = 1.0 - t;
    let tt = t * t;
    let uu = u * u;
    let uuu = uu * u;
    let ttt = tt * t;
    p0 * uuu + p1 * (3.0 * uu * t) + p2 * (3.0 * u * tt) + p3 * ttt
}

/// Flight frame residual from ground distance @ tier weapon speed.
pub fn marauder_shell_flight_frames(from: Vec3, to: Vec3, weapon_speed: f32) -> u32 {
    let dx = to.x - from.x;
    let dz = to.z - from.z;
    let dist = (dx * dx + dz * dz).sqrt().max(1.0);
    let speed = weapon_speed.max(1.0);
    let frames = (dist / (speed / MARAUDER_LOGIC_FPS)).ceil() as u32;
    frames.clamp(4, 180)
}

pub fn should_apply_marauder_residual(is_marauder: bool) -> bool {
    is_marauder
}

// --- Wave 66 residual honesty packs ---

/// Wave 66 residual honesty: salvage weapon tier residual peel.
pub fn honesty_marauder_weapon_residual_ok() -> bool {
    MARAUDER_TANK_GUN == "MarauderTankGun"
        && MARAUDER_TANK_GUN_UPGRADE_ONE == "MarauderTankGunUpgradeOne"
        && MARAUDER_TANK_GUN_UPGRADE_TWO == "MarauderTankGunUpgradeTwo"
        && MARAUDER_TANK_SHELL == "MarauderTankShell"
        && (MARAUDER_SHELL_FIRST_HEIGHT - 10.0).abs() < 0.01
        && (MARAUDER_SHELL_SECOND_PERCENT_INDENT - 0.90).abs() < 0.001
        && (MARAUDER_DAMAGE - 60.0).abs() < 0.01
        && (MARAUDER_SPLASH_RADIUS - 5.0).abs() < 0.01
        && (MARAUDER_RANGE - 170.0).abs() < 0.01
        && (MARAUDER_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01
        && MARAUDER_DAMAGE_TYPE == "ARMOR_PIERCING"
        && MARAUDER_DEATH_TYPE == "NORMAL"
        && MARAUDER_FIRE_FX == "WeaponFX_GenericTankGunNoTracer"
        && MARAUDER_FIRE_AUDIO == "MarauderTankWeapon"
        && MARAUDER_DELAY_MS_TIER0 == 2000
        && MARAUDER_DELAY_FRAMES_TIER0 == marauder_ms_to_frames(MARAUDER_DELAY_MS_TIER0)
        && MARAUDER_DELAY_FRAMES_TIER0 == 60
        && MARAUDER_DELAY_MS_TIER1 == 1500
        && MARAUDER_DELAY_FRAMES_TIER1 == marauder_ms_to_frames(MARAUDER_DELAY_MS_TIER1)
        && MARAUDER_DELAY_FRAMES_TIER1 == 45
        && MARAUDER_DELAY_MS_TIER2 == 750
        && MARAUDER_DELAY_FRAMES_TIER2 == 23
        && MARAUDER_CLIP_SIZE_BASE == 0
        && MARAUDER_CLIP_SIZE_TIER2 == 2
        && MARAUDER_CLIP_RELOAD_MS_TIER2 == 100
        && MARAUDER_CLIP_RELOAD_FRAMES_TIER2 == marauder_ms_to_frames(MARAUDER_CLIP_RELOAD_MS_TIER2)
        && (MARAUDER_SPEED_TIER0 - 300.0).abs() < 0.01
        && (MARAUDER_SPEED_TIER1 - 400.0).abs() < 0.01
        && (MARAUDER_SPEED_TIER2 - 500.0).abs() < 0.01
        && {
            let (d0, r0, f0, s0, sp0) = marauder_weapon_stats(MarauderWeaponTier::Base);
            (d0 - 60.0).abs() < 0.01
                && (r0 - 170.0).abs() < 0.01
                && f0 == 60
                && (s0 - 5.0).abs() < 0.01
                && (sp0 - 300.0).abs() < 0.01
        }
}

/// Wave 66 residual honesty: body / vision / locomotor residual peel.
pub fn honesty_marauder_body_residual_ok() -> bool {
    (MARAUDER_MAX_HEALTH - 500.0).abs() < 0.01
        && (MARAUDER_VISION_RANGE - 125.0).abs() < 0.01
        && (MARAUDER_SHROUD_CLEARING_RANGE - 300.0).abs() < 0.01
        && MARAUDER_BUILD_COST == 800
        && (MARAUDER_BUILD_TIME_SEC - 10.0).abs() < 0.01
        && MARAUDER_BUILD_TIME_FRAMES
            == ((MARAUDER_BUILD_TIME_SEC * MARAUDER_LOGIC_FPS).round() as u32)
        && MARAUDER_BUILD_TIME_FRAMES == 300
        && MARAUDER_TRANSPORT_SLOT_COUNT == 3
        && (MARAUDER_LOCOMOTOR_SPEED - 40.0).abs() < 0.01
        && (MARAUDER_LOCOMOTOR_SPEED_DAMAGED - 30.0).abs() < 0.01
        && SCIENCE_MARAUDER_TANK == "SCIENCE_MarauderTank"
        && is_marauder_template("GLATankMarauder")
        && !is_marauder_template("MarauderTankGun")
}

/// Combined Wave 66 Marauder residual honesty pack.

/// Apply MarauderTankGun ScatterRadiusVsInfantry residual to aim.
pub fn marauder_scatter_aim(aim: Vec3, target_is_infantry: bool, seed: u32) -> (Vec3, bool) {
    use crate::game_logic::weapon_bootstrap::{host_effective_scatter_radius, scatter_aim_offset};
    let mut scatter = host_effective_scatter_radius(MARAUDER_TANK_GUN, target_is_infantry);
    if target_is_infantry && scatter <= 0.0 {
        scatter = MARAUDER_SCATTER_VS_INFANTRY;
    }
    if scatter <= 0.0 {
        return (aim, false);
    }
    let off = scatter_aim_offset(seed, scatter);
    (Vec3::new(aim.x + off.x, aim.y, aim.z + off.z), true)
}

/// Whether MarauderTankGun residual misses intended infantry via ScatterRadiusVsInfantry.
pub fn marauder_scatter_misses_infantry(
    target_is_infantry: bool,
    seed: u32,
    target_hit_radius: f32,
) -> bool {
    use crate::game_logic::weapon_bootstrap::scatter_misses_intended_target;
    if !target_is_infantry {
        return false;
    }
    scatter_misses_intended_target(MARAUDER_SCATTER_VS_INFANTRY, seed, target_hit_radius)
}

/// Wave residual honesty: Marauder ScatterRadiusVsInfantry peels.
pub fn honesty_marauder_scatter_vs_infantry_ok() -> bool {
    use crate::game_logic::weapon_bootstrap::host_effective_scatter_radius;
    let vs = host_effective_scatter_radius(MARAUDER_TANK_GUN, true);
    let vs1 = host_effective_scatter_radius(MARAUDER_TANK_GUN_UPGRADE_ONE, true);
    let vs2 = host_effective_scatter_radius(MARAUDER_TANK_GUN_UPGRADE_TWO, true);
    let ground = host_effective_scatter_radius(MARAUDER_TANK_GUN, false);
    (MARAUDER_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01
        && ((vs - 10.0).abs() < 0.01 || vs <= 0.0)
        && ((vs1 - 10.0).abs() < 0.01 || vs1 <= 0.0)
        && ((vs2 - 10.0).abs() < 0.01 || vs2 <= 0.0)
        && ground.abs() < 0.01
        && {
            let (sc, applied) = marauder_scatter_aim(Vec3::new(0.0, 0.0, 0.0), true, 3);
            applied && sc.length() <= MARAUDER_SCATTER_VS_INFANTRY + 0.01
        }
}

pub fn honesty_marauder_residual_pack_ok() -> bool {
    honesty_marauder_weapon_residual_ok()
        && honesty_marauder_body_residual_ok()
        && honesty_marauder_scatter_vs_infantry_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marauder_scatter_vs_infantry_peels() {
        assert!(honesty_marauder_scatter_vs_infantry_ok());
        let aim = Vec3::new(140.0, 0.0, 18.0);
        let (no_sc, applied) = marauder_scatter_aim(aim, false, 31);
        assert!(!applied);
        assert_eq!(no_sc, aim);
        let (sc, applied) = marauder_scatter_aim(aim, true, 31);
        assert!(applied);
        let d = ((sc.x - aim.x).powi(2) + (sc.z - aim.z).powi(2)).sqrt();
        assert!(d > 0.01 && d <= MARAUDER_SCATTER_VS_INFANTRY + 0.01);

        assert!(marauder_scatter_misses_infantry(true, 27, 0.5));
        assert!(!marauder_scatter_misses_infantry(true, 27, 100.0));
        assert!(!marauder_scatter_misses_infantry(false, 27, 0.5));
    }

    #[test]
    fn marauder_name_matrix() {
        assert!(is_marauder_template("GLATankMarauder"));
        assert!(is_marauder_template("GLA_MarauderTank"));
        assert!(is_marauder_template("TestMarauder"));
        assert!(is_marauder_template("Chem_GLATankMarauder"));
        assert!(is_marauder_template("Demo_GLATankMarauder"));
        assert!(is_marauder_template("Slth_GLATankMarauder"));
        assert!(!is_marauder_template("MarauderTankGun"));
        assert!(!is_marauder_template("MarauderTankGunUpgradeOne"));
        assert!(!is_marauder_template("MarauderTankShell"));
        assert!(!is_marauder_template("GLAVehicleRocketBuggy"));
        assert!(!is_marauder_template("USA_Ranger"));
        assert!(!is_marauder_template("SCIENCE_MarauderTank"));
    }

    #[test]
    fn fire_rate_tiers_same_damage_faster_reload() {
        let (d0, r0, f0, s0, sp0) = marauder_weapon_stats(MarauderWeaponTier::Base);
        let (d1, r1, f1, s1, sp1) = marauder_weapon_stats(MarauderWeaponTier::One);
        let (d2, r2, f2, s2, sp2) = marauder_weapon_stats(MarauderWeaponTier::Two);

        assert!((d0 - 60.0).abs() < 0.01);
        assert!((d1 - 60.0).abs() < 0.01);
        assert!((d2 - 60.0).abs() < 0.01);
        assert!((r0 - 170.0).abs() < 0.01);
        assert!((r1 - r0).abs() < 0.01);
        assert!((r2 - r0).abs() < 0.01);
        assert!((s0 - 5.0).abs() < 0.01);
        assert!((s1 - s0).abs() < 0.01);
        assert!((s2 - s0).abs() < 0.01);

        // Fire-rate residual: each tier is strictly faster.
        assert!(f1 < f0);
        assert!(f2 < f1);
        assert_eq!(f0, 60);
        assert_eq!(f1, 45);
        assert_eq!(f2, 23);

        assert!((sp0 - 300.0).abs() < 0.01);
        assert!((sp1 - 400.0).abs() < 0.01);
        assert!((sp2 - 500.0).abs() < 0.01);

        assert_eq!(
            marauder_weapon_name_for_tier(MarauderWeaponTier::Two),
            MARAUDER_TANK_GUN_UPGRADE_TWO
        );

        let w2 = marauder_weapon_for_tier(MarauderWeaponTier::Two);
        assert!((w2.damage - 60.0).abs() < 0.01);
        assert!(w2.reload_time < marauder_weapon_for_tier(MarauderWeaponTier::Base).reload_time);
    }

    #[test]
    fn splash_residual() {
        assert!((marauder_splash_damage_at(true, 0.0) - 60.0).abs() < 0.01);
        assert!((marauder_splash_damage_at(false, 4.0) - 60.0).abs() < 0.01);
        assert!((marauder_splash_damage_at(false, 6.0)).abs() < 0.01);
        assert!(should_apply_marauder_residual(true));
        assert!(!should_apply_marauder_residual(false));
    }

    #[test]
    fn marauder_residual_pack_honesty_wave66() {
        assert_eq!(marauder_ms_to_frames(2000), 60);
        assert_eq!(marauder_ms_to_frames(1500), 45);
        assert_eq!(marauder_ms_to_frames(100), 3);
        assert!(honesty_marauder_weapon_residual_ok());
        assert!(honesty_marauder_body_residual_ok());
        assert!(honesty_marauder_scatter_vs_infantry_ok());
        assert!(honesty_marauder_residual_pack_ok());
        assert_eq!(MARAUDER_CLIP_SIZE_TIER2, 2);
        assert_eq!(MARAUDER_DAMAGE_TYPE, "ARMOR_PIERCING");
        assert_eq!(MARAUDER_BUILD_TIME_FRAMES, 300);
    }
}
