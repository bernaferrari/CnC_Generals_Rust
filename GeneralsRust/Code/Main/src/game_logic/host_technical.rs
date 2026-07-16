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
//! - Not full chassis reskin (ChassisOne/Two/Three) visual matrix
//! - Not network salvage / transport replication (network deferred)

use super::Weapon;

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
pub fn honesty_technical_residual_pack_ok() -> bool {
    honesty_technical_weapon_residual_ok()
        && honesty_technical_transport_residual_ok()
        && honesty_technical_body_residual_ok()
        && honesty_technical_training_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn technical_residual_pack_honesty() {
        assert_eq!(technical_ms_to_frames(200), 6);
        assert_eq!(technical_ms_to_frames(1000), 30);
        assert!(honesty_technical_weapon_residual_ok());
        assert!(honesty_technical_transport_residual_ok());
        assert!(honesty_technical_body_residual_ok());
        assert!(honesty_technical_training_residual_ok());
        assert!(honesty_technical_residual_pack_ok());
    }
}
