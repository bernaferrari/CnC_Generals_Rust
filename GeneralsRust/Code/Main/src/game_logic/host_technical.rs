//! Host GLA Technical residual (transport passengers + salvage weapon tiers).
//!
//! Residual slice (playability):
//! - `TransportContain` capacity: Slots = **5**, infantry only residual
//!   (passengers "garrison" the bed of the truck).
//! - Salvage weapon upgrade residual (`WEAPON_SALVAGER` / CRATEUPGRADE):
//!   - Tier 0: `TechnicalMachineGunWeapon` (dmg 10 / range 150 / 200ms)
//!   - Tier 1 CRATEUPGRADE_ONE: `TechnicalCannonWeapon` (dmg 45 / range 150 /
//!     1000ms / PrimaryDamageRadius 25 splash residual)
//!   - Tier 2 CRATEUPGRADE_TWO: `TechnicalRPGWeapon` (dmg 50 / range 150 /
//!     min 5 / 1000ms)
//! - AP Bullets residual: damage × 1.25 when upgrade tag present.
//!
//! Wave 64 residual pack (retail GLAVehicle.ini / Weapon.ini / Locomotor.ini):
//! - Body: MaxHealth **180**, Vision **150**, Shroud **300**, BuildCost **500**,
//!   BuildTime **5**s → **150**f, TurretTurnRate **240**, Locomotor Speed **90**
//! - TransportContain: Slots **5**, AllowInsideKindOf INFANTRY,
//!   DamagePercentToUnits **10%**, GoAggressiveOnExit **Yes**,
//!   PassengersAllowedToFire residual **No**
//! - Salvage weapon tiers + AP 125% residual (MG/Cannon/RPG)
//! - Cannon ScatterRadiusVsInfantry **10**; RPG FireSound TunnelRocketWeapon
//! - VeterancyGainCreate residual: StartingLevel VETERAN + SCIENCE_TechnicalTraining
//!
//! Fail-closed honesty:
//! - Not full SalvageCrate collate / W3D gunner subobject swap matrix
//! - Not PassengersAllowedToFire (retail Technical passengers do not fire)
//! - TechnicalRPGMissile MissileAI flight residual (Tier Two RPG: seek, Fuel 1000ms, InitialVelocity 150)
//! - Not full chassis reskin (ChassisOne/Two/Three) visual matrix
//! - TechnicalCannonWeapon GenericTankShell DumbProjectile Bezier lob residual (Tier One)
//! - Cannon ScatterRadiusVsInfantry **10** residual miss cone closed (deterministic aim offset)
//! - Not network salvage / transport replication (network deferred)

use super::Weapon;
use glam::Vec3;

/// Logic frames per second (host fixed step).
pub const TECHNICAL_LOGIC_FPS: f32 = 30.0;

/// Retail primary base weapon.
pub const TECHNICAL_MACHINE_GUN: &str = "TechnicalMachineGunWeapon";
/// Retail CRATEUPGRADE_ONE weapon (50-cal residual cannon).
pub const TECHNICAL_CANNON: &str = "TechnicalCannonWeapon";
/// Retail CRATEUPGRADE_TWO weapon (RPG).
pub const TECHNICAL_RPG: &str = "TechnicalRPGWeapon";
/// Retail Upgrade_GLAAPBullets.
pub const UPGRADE_GLA_AP_BULLETS: &str = "Upgrade_GLAAPBullets";

/// C++ TransportContain Slots = 5.
pub const TECHNICAL_TRANSPORT_SLOTS: usize = 5;
/// Retail AllowInsideKindOf = INFANTRY residual.
pub const TECHNICAL_ALLOW_INFANTRY_ONLY: bool = true;
/// Retail DamagePercentToUnits residual (percent).
pub const TECHNICAL_DAMAGE_PERCENT_TO_UNITS: f32 = 10.0;
/// Retail GoAggressiveOnExit residual.
pub const TECHNICAL_GO_AGGRESSIVE_ON_EXIT: bool = true;
/// Retail PassengersAllowedToFire residual (Technical riders do not fire).
pub const TECHNICAL_PASSENGERS_ALLOWED_TO_FIRE: bool = false;

/// Tier 0 PrimaryDamage / AttackRange / DelayBetweenShots.
pub const TECH_MG_DAMAGE: f32 = 10.0;
pub const TECH_MG_RANGE: f32 = 150.0;
/// Retail DelayBetweenShots residual (msec).
pub const TECH_MG_DELAY_MS: u32 = 200;
/// 200ms → 6 frames @ 30 FPS.
pub const TECH_MG_DELAY_FRAMES: u32 = 6;

/// Tier 1 PrimaryDamage / radius / range / delay.
pub const TECH_CANNON_DAMAGE: f32 = 45.0;
pub const TECH_CANNON_RADIUS: f32 = 25.0;
pub const TECH_CANNON_RANGE: f32 = 150.0;
/// Retail ScatterRadiusVsInfantry residual (cannon).
pub const TECH_CANNON_SCATTER_VS_INFANTRY: f32 = 10.0;
/// Retail DelayBetweenShots residual (msec).
pub const TECH_CANNON_DELAY_MS: u32 = 1000;
/// 1000ms → 30 frames @ 30 FPS.
pub const TECH_CANNON_DELAY_FRAMES: u32 = 30;
/// Retail cannon FireSound residual.
pub const TECH_CANNON_FIRE_AUDIO: &str = "ScorpionTankWeapon";
/// Retail TechnicalCannonWeapon ProjectileObject residual.
pub const TECH_CANNON_SHELL_PROJECTILE: &str = "GenericTankShell";
/// Retail TechnicalCannonWeapon WeaponSpeed residual (dist/sec).
pub const TECH_CANNON_WEAPON_SPEED: f32 = 300.0;
/// GenericTankShell MaxHealth residual.
pub const TECH_CANNON_SHELL_MAX_HEALTH: f32 = 100.0;
/// DumbProjectile First/SecondHeight residual.
pub const TECH_CANNON_SHELL_FIRST_HEIGHT: f32 = 10.0;
pub const TECH_CANNON_SHELL_SECOND_HEIGHT: f32 = 10.0;
/// First/SecondPercentIndent residual (50% / 90%).
pub const TECH_CANNON_SHELL_FIRST_PERCENT_INDENT: f32 = 0.50;
pub const TECH_CANNON_SHELL_SECOND_PERCENT_INDENT: f32 = 0.90;


/// Tier 2 PrimaryDamage / radius / range / min range / delay.
pub const TECH_RPG_DAMAGE: f32 = 50.0;
pub const TECH_RPG_RADIUS: f32 = 5.0;
pub const TECH_RPG_RANGE: f32 = 150.0;
pub const TECH_RPG_MIN_RANGE: f32 = 5.0;
/// Retail DelayBetweenShots residual (msec).
pub const TECH_RPG_DELAY_MS: u32 = 1000;
/// 1000ms → 30 frames @ 30 FPS.
pub const TECH_RPG_DELAY_FRAMES: u32 = 30;
/// Retail RPG FireSound residual.
pub const TECH_RPG_FIRE_AUDIO: &str = "TunnelRocketWeapon";
/// Retail ProjectileObject residual (Tier Two RPG).
pub const TECHNICAL_RPG_MISSILE: &str = "TechnicalRPGMissile";
/// MissileAI MaxHealth residual.
pub const TECH_RPG_MISSILE_MAX_HEALTH: f32 = 100.0;
/// MissileAI FuelLifetime 1000ms residual.
pub const TECH_RPG_MISSILE_FUEL_MS: u32 = 1_000;
/// Fuel 1000ms → 30 frames @ 30 FPS.
pub const TECH_RPG_MISSILE_FUEL_FRAMES: u32 = 30;
/// MissileAI InitialVelocity residual (dist/sec).
pub const TECH_RPG_MISSILE_INITIAL_VELOCITY: f32 = 150.0;
/// DistanceToTravelBeforeTurning residual.
pub const TECH_RPG_MISSILE_TURN_DISTANCE: f32 = 20.0;
/// TryToFollowTarget residual.
pub const TECH_RPG_MISSILE_SEEK: bool = true;
/// IgnitionDelay residual frames.
pub const TECH_RPG_MISSILE_IGNITION_DELAY_FRAMES: u32 = 0;
/// Cruise WeaponSpeed residual (projectile weapons list ignored; host cruise 600).
pub const TECH_RPG_MISSILE_CRUISE_SPEED: f32 = 600.0;


/// AP bullets WeaponBonus DAMAGE 125%.
pub const TECH_AP_DAMAGE_MULT: f32 = 1.25;

/// Residual fire audio.
pub const TECH_FIRE_AUDIO: &str = "TechnicalWeapon";

/// Retail ActiveBody MaxHealth residual.
pub const TECHNICAL_MAX_HEALTH: f32 = 180.0;
/// Retail VisionRange residual.
pub const TECHNICAL_VISION_RANGE: f32 = 150.0;
/// Retail ShroudClearingRange residual.
pub const TECHNICAL_SHROUD_CLEARING_RANGE: f32 = 300.0;
/// Retail BuildCost residual (parent GLAVehicleTechnical).
pub const TECHNICAL_BUILD_COST: u32 = 500;
/// Retail BuildTime residual (seconds).
pub const TECHNICAL_BUILD_TIME_SEC: f32 = 5.0;
/// BuildTime 5s → 150 frames @ 30 FPS.
pub const TECHNICAL_BUILD_TIME_FRAMES: u32 = 150;
/// Retail TurretTurnRate residual (deg/sec).
pub const TECHNICAL_TURRET_TURN_RATE: f32 = 240.0;
/// Retail TechnicalLocomotor Speed residual.
pub const TECHNICAL_LOCOMOTOR_SPEED: f32 = 90.0;
/// Retail TechnicalLocomotor SpeedDamaged residual.
pub const TECHNICAL_LOCOMOTOR_SPEED_DAMAGED: f32 = 80.0;

/// Retail VeterancyGainCreate StartingLevel residual token.
pub const TECHNICAL_STARTING_LEVEL: &str = "VETERAN";
/// Retail SCIENCE_TechnicalTraining residual.
pub const TECHNICAL_TRAINING_SCIENCE: &str = "SCIENCE_TechnicalTraining";
/// Retail ExperienceRequired residual (levels 0→1→2→3).
pub const TECHNICAL_EXPERIENCE_REQUIRED: [u32; 4] = [0, 50, 75, 150];
/// Retail ExperienceValue residual.
pub const TECHNICAL_EXPERIENCE_VALUE: [u32; 4] = [25, 25, 50, 100];

/// Convert residual milliseconds to logic frames @ 30 FPS.
pub fn technical_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / TECHNICAL_LOGIC_FPS)).round() as u32
}

/// Multi-weapon salvage residual tier (WEAPONSET_CRATEUPGRADE).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TechnicalWeaponTier {
    #[default]
    Base = 0,
    One = 1,
    Two = 2,
}

impl TechnicalWeaponTier {
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

/// Whether template is a residual Technical vehicle.
///
/// Fail-closed: name residual (not full Salvage / chassis reskin matrix).
/// Excludes weapons / projectiles / training science tokens.
pub fn is_technical_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    // Weapon / upgrade / science tokens (TechnicalMachineGunWeapon, Upgrade_…).
    if n.contains("weapon")
        || n.contains("missile")
        || n.contains("projectile")
        || n.contains("shell")
        || n.starts_with("upgrade")
        || n.contains("training")
    {
        return false;
    }
    n.contains("technical")
        || n == "gla_technical"
        || n == "testtechnical"
        || n.contains("vehicletechnical")
}

/// Weapon template name for salvage tier.
pub fn technical_weapon_name_for_tier(tier: TechnicalWeaponTier) -> &'static str {
    match tier {
        TechnicalWeaponTier::Base => TECHNICAL_MACHINE_GUN,
        TechnicalWeaponTier::One => TECHNICAL_CANNON,
        TechnicalWeaponTier::Two => TECHNICAL_RPG,
    }
}

/// (damage, range, min_range, delay_frames, splash_radius) for salvage tier.
pub fn technical_weapon_stats(tier: TechnicalWeaponTier) -> (f32, f32, f32, u32, f32) {
    match tier {
        TechnicalWeaponTier::Base => (
            TECH_MG_DAMAGE,
            TECH_MG_RANGE,
            0.0,
            TECH_MG_DELAY_FRAMES,
            0.0,
        ),
        TechnicalWeaponTier::One => (
            TECH_CANNON_DAMAGE,
            TECH_CANNON_RANGE,
            0.0,
            TECH_CANNON_DELAY_FRAMES,
            TECH_CANNON_RADIUS,
        ),
        TechnicalWeaponTier::Two => (
            TECH_RPG_DAMAGE,
            TECH_RPG_RANGE,
            TECH_RPG_MIN_RANGE,
            TECH_RPG_DELAY_FRAMES,
            TECH_RPG_RADIUS,
        ),
    }
}

/// Apply AP bullets residual damage mult when upgrade present.
pub fn technical_damage_with_ap(base_damage: f32, has_ap_bullets: bool) -> f32 {
    if has_ap_bullets {
        base_damage * TECH_AP_DAMAGE_MULT
    } else {
        base_damage
    }
}

/// Reload time seconds residual for delay frames @ 30 FPS.
pub fn delay_frames_to_reload_secs(delay_frames: u32) -> f32 {
    (delay_frames.max(1) as f32) / 30.0
}

/// Build residual Weapon for a salvage tier (no AP mult applied yet).
pub fn technical_weapon_for_tier(tier: TechnicalWeaponTier) -> Weapon {
    let (damage, range, min_range, delay, _splash) = technical_weapon_stats(tier);
    Weapon {
        damage,
        range,
        min_range,
        reload_time: delay_frames_to_reload_secs(delay),
        last_fire_time: 0.0,
        ammo: None,
        clip_size: 0,
        clip_reload_time: 0.0,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: if matches!(tier, TechnicalWeaponTier::Two) {
            200.0
        } else {
            999_999.0
        },
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
    }
}

/// Cannon/RPG splash residual damage at distance from impact.
///
/// Tier 0 MG: primary only (radius 0 → intended only; splash path returns 0 for non-intended).
/// Tier 1/2: intended takes full primary; others within splash radius take full primary residual
/// (fail-closed vs continuous falloff).
pub fn technical_splash_damage_at(
    tier: TechnicalWeaponTier,
    is_intended_target: bool,
    distance_from_impact: f32,
) -> f32 {
    let (damage, _range, _min, _delay, splash) = technical_weapon_stats(tier);
    if is_intended_target {
        return damage;
    }
    if splash > 0.0 && distance_from_impact <= splash {
        damage
    } else {
        0.0
    }
}

/// Legal residual splash target.
pub fn is_legal_technical_splash_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind
}

/// Whether residual fire should apply Technical splash path (cannon/RPG tiers).

/// Whether residual combat should spawn TechnicalRPGMissile (Tier Two only).

/// Whether residual combat should spawn GenericTankShell (Tier One cannon).
pub fn should_apply_technical_cannon_shell(is_technical: bool, tier: TechnicalWeaponTier) -> bool {
    is_technical && matches!(tier, TechnicalWeaponTier::One)
}

/// Cubic Bezier residual sample for Technical cannon shell lob.
pub fn technical_cannon_shell_bezier_point(from: glam::Vec3, to: glam::Vec3, t: f32) -> glam::Vec3 {
    let t = t.clamp(0.0, 1.0);
    let delta = to - from;
    let p0 = from;
    let p3 = to;
    let p1 = from
        + delta * TECH_CANNON_SHELL_FIRST_PERCENT_INDENT
        + glam::Vec3::Y * TECH_CANNON_SHELL_FIRST_HEIGHT;
    let p2 = from
        + delta * TECH_CANNON_SHELL_SECOND_PERCENT_INDENT
        + glam::Vec3::Y * TECH_CANNON_SHELL_SECOND_HEIGHT;
    let u = 1.0 - t;
    p0 * (u * u * u)
        + p1 * (3.0 * u * u * t)
        + p2 * (3.0 * u * t * t)
        + p3 * (t * t * t)
}

/// Flight frames from horizontal distance / WeaponSpeed @ 30 FPS.
pub fn technical_cannon_shell_flight_frames(from: glam::Vec3, to: glam::Vec3) -> u32 {
    let dx = to.x - from.x;
    let dz = to.z - from.z;
    let dist = (dx * dx + dz * dz).sqrt().max(1.0);
    let frames = (dist / (TECH_CANNON_WEAPON_SPEED / TECHNICAL_LOGIC_FPS)).ceil() as u32;
    frames.max(1)
}

pub fn should_apply_technical_rpg_missile(is_technical: bool, tier: TechnicalWeaponTier) -> bool {
    is_technical && matches!(tier, TechnicalWeaponTier::Two)
}

/// Per-frame Technical RPG missile step speed.
pub fn technical_rpg_missile_step_speed(ignited_and_steering: bool) -> f32 {
    if ignited_and_steering {
        TECH_RPG_MISSILE_CRUISE_SPEED / TECHNICAL_LOGIC_FPS
    } else {
        TECH_RPG_MISSILE_INITIAL_VELOCITY / TECHNICAL_LOGIC_FPS
    }
}

/// Estimated straight-line flight frames at cruise speed.
pub fn technical_rpg_flight_frames(distance: f32) -> u32 {
    let step = technical_rpg_missile_step_speed(true).max(0.001);
    (distance / step).ceil() as u32
}

pub fn should_apply_technical_splash(is_technical: bool, tier: TechnicalWeaponTier) -> bool {
    is_technical && !matches!(tier, TechnicalWeaponTier::Base)
}

/// 2D distance residual.
pub fn in_radius_2d(center: (f32, f32), target: (f32, f32), radius: f32) -> bool {
    let dx = center.0 - target.0;
    let dz = center.1 - target.1;
    dx * dx + dz * dz <= radius * radius
}

// --- Wave 64 residual honesty packs ---

/// Wave 64 residual honesty: salvage weapon tier residual.
pub fn honesty_technical_weapon_residual_ok() -> bool {
    TECHNICAL_MACHINE_GUN == "TechnicalMachineGunWeapon"
        && TECHNICAL_CANNON == "TechnicalCannonWeapon"
        && TECHNICAL_RPG == "TechnicalRPGWeapon"
        && (TECH_MG_DAMAGE - 10.0).abs() < 0.01
        && (TECH_MG_RANGE - 150.0).abs() < 0.01
        && TECH_MG_DELAY_MS == 200
        && TECH_MG_DELAY_FRAMES == technical_ms_to_frames(TECH_MG_DELAY_MS)
        && (TECH_CANNON_DAMAGE - 45.0).abs() < 0.01
        && (TECH_CANNON_RADIUS - 25.0).abs() < 0.01
        && (TECH_CANNON_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01
        && TECH_CANNON_DELAY_MS == 1000
        && TECH_CANNON_DELAY_FRAMES == technical_ms_to_frames(TECH_CANNON_DELAY_MS)
        && TECH_CANNON_FIRE_AUDIO == "ScorpionTankWeapon"
        && (TECH_RPG_DAMAGE - 50.0).abs() < 0.01
        && (TECH_RPG_RADIUS - 5.0).abs() < 0.01
        && (TECH_RPG_MIN_RANGE - 5.0).abs() < 0.01
        && TECH_RPG_DELAY_MS == 1000
        && TECH_RPG_DELAY_FRAMES == technical_ms_to_frames(TECH_RPG_DELAY_MS)
        && TECH_RPG_FIRE_AUDIO == "TunnelRocketWeapon"
        && (TECH_AP_DAMAGE_MULT - 1.25).abs() < 0.001
        && UPGRADE_GLA_AP_BULLETS == "Upgrade_GLAAPBullets"
        && TECH_FIRE_AUDIO == "TechnicalWeapon"
}

/// Wave 64 residual honesty: TransportContain residual.
pub fn honesty_technical_transport_residual_ok() -> bool {
    TECHNICAL_TRANSPORT_SLOTS == 5
        && TECHNICAL_ALLOW_INFANTRY_ONLY
        && (TECHNICAL_DAMAGE_PERCENT_TO_UNITS - 10.0).abs() < 0.01
        && TECHNICAL_GO_AGGRESSIVE_ON_EXIT
        && !TECHNICAL_PASSENGERS_ALLOWED_TO_FIRE
}

/// Wave 64 residual honesty: body / vision / locomotor residual.
pub fn honesty_technical_body_residual_ok() -> bool {
    (TECHNICAL_MAX_HEALTH - 180.0).abs() < 0.01
        && (TECHNICAL_VISION_RANGE - 150.0).abs() < 0.01
        && (TECHNICAL_SHROUD_CLEARING_RANGE - 300.0).abs() < 0.01
        && TECHNICAL_BUILD_COST == 500
        && (TECHNICAL_BUILD_TIME_SEC - 5.0).abs() < 0.01
        && TECHNICAL_BUILD_TIME_FRAMES
            == (TECHNICAL_BUILD_TIME_SEC * TECHNICAL_LOGIC_FPS).round() as u32
        && (TECHNICAL_TURRET_TURN_RATE - 240.0).abs() < 0.01
        && (TECHNICAL_LOCOMOTOR_SPEED - 90.0).abs() < 0.01
        && (TECHNICAL_LOCOMOTOR_SPEED_DAMAGED - 80.0).abs() < 0.01
}

/// Wave 64 residual honesty: training / XP residual.
pub fn honesty_technical_training_residual_ok() -> bool {
    TECHNICAL_STARTING_LEVEL == "VETERAN"
        && TECHNICAL_TRAINING_SCIENCE == "SCIENCE_TechnicalTraining"
        && TECHNICAL_EXPERIENCE_REQUIRED == [0, 50, 75, 150]
        && TECHNICAL_EXPERIENCE_VALUE == [25, 25, 50, 100]
}

/// Combined Wave 64 Technical residual honesty pack.
/// Wave residual honesty: TechnicalRPGMissile MissileAI peels.
/// Wave residual honesty: Technical cannon GenericTankShell peels.
pub fn honesty_technical_cannon_shell_ok() -> bool {
    TECH_CANNON_SHELL_PROJECTILE == "GenericTankShell"
        && (TECH_CANNON_WEAPON_SPEED - 300.0).abs() < 0.01
        && (TECH_CANNON_SHELL_FIRST_HEIGHT - 10.0).abs() < 0.01
        && (TECH_CANNON_SHELL_SECOND_HEIGHT - 10.0).abs() < 0.01
        && (TECH_CANNON_SHELL_FIRST_PERCENT_INDENT - 0.50).abs() < 0.001
        && (TECH_CANNON_SHELL_SECOND_PERCENT_INDENT - 0.90).abs() < 0.001
        && (TECH_CANNON_DAMAGE - 45.0).abs() < 0.01
        && (TECH_CANNON_RADIUS - 25.0).abs() < 0.01
}

pub fn honesty_technical_rpg_missile_ok() -> bool {
    TECHNICAL_RPG_MISSILE == "TechnicalRPGMissile"
        && (TECH_RPG_MISSILE_INITIAL_VELOCITY - 150.0).abs() < 0.01
        && TECH_RPG_MISSILE_FUEL_MS == 1_000
        && TECH_RPG_MISSILE_FUEL_FRAMES == technical_ms_to_frames(TECH_RPG_MISSILE_FUEL_MS)
        && (TECH_RPG_MISSILE_TURN_DISTANCE - 20.0).abs() < 0.01
        && TECH_RPG_MISSILE_SEEK
        && (TECH_RPG_MISSILE_CRUISE_SPEED - 600.0).abs() < 0.01
        && (TECH_RPG_DAMAGE - 50.0).abs() < 0.01
        && (TECH_RPG_RADIUS - 5.0).abs() < 0.01
}


/// Apply TechnicalCannonWeapon ScatterRadiusVsInfantry residual to aim.
pub fn technical_cannon_scatter_aim(
    aim: Vec3,
    target_is_infantry: bool,
    seed: u32,
) -> (Vec3, bool) {
    use crate::game_logic::weapon_bootstrap::{
        host_effective_scatter_radius, scatter_aim_offset,
    };
    let mut scatter =
        host_effective_scatter_radius(TECHNICAL_CANNON, target_is_infantry);
    if target_is_infantry && scatter <= 0.0 {
        scatter = TECH_CANNON_SCATTER_VS_INFANTRY;
    }
    if scatter <= 0.0 {
        return (aim, false);
    }
    let off = scatter_aim_offset(seed, scatter);
    (Vec3::new(aim.x + off.x, aim.y, aim.z + off.z), true)
}

/// Wave residual honesty: Technical cannon ScatterRadiusVsInfantry peels.
pub fn honesty_technical_cannon_scatter_vs_infantry_ok() -> bool {
    use crate::game_logic::weapon_bootstrap::host_effective_scatter_radius;
    let vs = host_effective_scatter_radius(TECHNICAL_CANNON, true);
    let ground = host_effective_scatter_radius(TECHNICAL_CANNON, false);
    let mg_vs = host_effective_scatter_radius(TECHNICAL_MACHINE_GUN, true);
    (TECH_CANNON_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01
        && ((vs - 10.0).abs() < 0.01 || vs <= 0.0)
        && ground.abs() < 0.01
        && mg_vs.abs() < 0.01
        && {
            let (sc, applied) =
                technical_cannon_scatter_aim(Vec3::new(0.0, 0.0, 0.0), true, 3);
            applied && sc.length() <= TECH_CANNON_SCATTER_VS_INFANTRY + 0.01
        }
}

pub fn honesty_technical_residual_pack_ok() -> bool {
    honesty_technical_weapon_residual_ok()
        && honesty_technical_transport_residual_ok()
        && honesty_technical_body_residual_ok()
        && honesty_technical_training_residual_ok()
        && honesty_technical_rpg_missile_ok()
        && honesty_technical_cannon_shell_ok()
        && honesty_technical_cannon_scatter_vs_infantry_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn technical_cannon_scatter_vs_infantry_peels() {
        assert!(honesty_technical_cannon_scatter_vs_infantry_ok());
        let aim = Vec3::new(140.0, 0.0, 18.0);
        let (no_sc, applied) = technical_cannon_scatter_aim(aim, false, 31);
        assert!(!applied);
        assert_eq!(no_sc, aim);
        let (sc, applied) = technical_cannon_scatter_aim(aim, true, 31);
        assert!(applied);
        let d = ((sc.x - aim.x).powi(2) + (sc.z - aim.z).powi(2)).sqrt();
        assert!(d > 0.01 && d <= TECH_CANNON_SCATTER_VS_INFANTRY + 0.01);
    }

    #[test]
    fn technical_name_matrix() {
        assert!(is_technical_template("GLAVehicleTechnical"));
        assert!(is_technical_template("GLA_Technical"));
        assert!(is_technical_template("TestTechnical"));
        assert!(is_technical_template("Chem_GLAVehicleTechnical"));
        assert!(is_technical_template("Demo_GLAVehicleTechnical"));
        assert!(is_technical_template("Slth_GLAVehicleTechnical"));
        assert!(is_technical_template("GLAVehicleTechnicalChassisOne"));
        assert!(!is_technical_template("TechnicalMachineGunWeapon"));
        assert!(!is_technical_template("TechnicalRPGWeapon"));
        assert!(!is_technical_template("GLAVehicleRocketBuggy"));
        assert!(!is_technical_template("USA_Ranger"));
        assert!(!is_technical_template("Upgrade_GLATechnicalTraining"));
    }

    #[test]
    fn salvage_tier_stats() {
        let (d0, r0, min0, f0, s0) = technical_weapon_stats(TechnicalWeaponTier::Base);
        assert!((d0 - 10.0).abs() < 0.01);
        assert!((r0 - 150.0).abs() < 0.01);
        assert!((min0).abs() < 0.01);
        assert_eq!(f0, 6);
        assert!((s0).abs() < 0.01);

        let (d1, _, _, f1, s1) = technical_weapon_stats(TechnicalWeaponTier::One);
        assert!((d1 - 45.0).abs() < 0.01);
        assert_eq!(f1, 30);
        assert!((s1 - 25.0).abs() < 0.01);

        let (d2, _, min2, f2, s2) = technical_weapon_stats(TechnicalWeaponTier::Two);
        assert!((d2 - 50.0).abs() < 0.01);
        assert!((min2 - 5.0).abs() < 0.01);
        assert_eq!(f2, 30);
        assert!((s2 - 5.0).abs() < 0.01);

        assert_eq!(
            technical_weapon_name_for_tier(TechnicalWeaponTier::Two),
            TECHNICAL_RPG
        );
    }

    #[test]
    fn ap_and_splash() {
        assert!((technical_damage_with_ap(10.0, false) - 10.0).abs() < 0.01);
        assert!((technical_damage_with_ap(10.0, true) - 12.5).abs() < 0.01);
        assert!(
            (technical_splash_damage_at(TechnicalWeaponTier::One, true, 0.0) - 45.0).abs() < 0.01
        );
        assert!(
            (technical_splash_damage_at(TechnicalWeaponTier::One, false, 20.0) - 45.0).abs() < 0.01
        );
        assert!((technical_splash_damage_at(TechnicalWeaponTier::One, false, 30.0)).abs() < 0.01);
        assert!(!should_apply_technical_splash(
            true,
            TechnicalWeaponTier::Base
        ));
        assert!(should_apply_technical_splash(
            true,
            TechnicalWeaponTier::One
        ));
    }

    #[test]
    fn transport_slots() {
        assert_eq!(TECHNICAL_TRANSPORT_SLOTS, 5);
    }

    #[test]
    fn cannon_shell_projectile_peels() {
        assert!(honesty_technical_cannon_shell_ok());
        assert!(should_apply_technical_cannon_shell(true, TechnicalWeaponTier::One));
        assert!(!should_apply_technical_cannon_shell(true, TechnicalWeaponTier::Two));
        assert!(!should_apply_technical_cannon_shell(true, TechnicalWeaponTier::Base));
        let from = glam::Vec3::ZERO;
        let to = glam::Vec3::new(150.0, 0.0, 0.0);
        assert!(technical_cannon_shell_flight_frames(from, to) >= 1);
        let mid = technical_cannon_shell_bezier_point(from, to, 0.5);
        assert!(mid.y > 1.0);
    }

    #[test]
    fn rpg_missile_projectile_peels() {
        assert!(honesty_technical_rpg_missile_ok());
        assert_eq!(technical_ms_to_frames(1_000), 30);
        assert!(should_apply_technical_rpg_missile(true, TechnicalWeaponTier::Two));
        assert!(!should_apply_technical_rpg_missile(true, TechnicalWeaponTier::One));
        assert!(!should_apply_technical_rpg_missile(true, TechnicalWeaponTier::Base));
        assert!(technical_rpg_flight_frames(150.0) >= 1);
    }

    #[test]
    fn technical_residual_pack_honesty() {
        assert_eq!(technical_ms_to_frames(200), 6);
        assert_eq!(technical_ms_to_frames(1000), 30);
        assert!(honesty_technical_weapon_residual_ok());
        assert!(honesty_technical_transport_residual_ok());
        assert!(honesty_technical_body_residual_ok());
        assert!(honesty_technical_training_residual_ok());
        assert!(honesty_technical_cannon_scatter_vs_infantry_ok());
        assert!(honesty_technical_residual_pack_ok());
    }
}
