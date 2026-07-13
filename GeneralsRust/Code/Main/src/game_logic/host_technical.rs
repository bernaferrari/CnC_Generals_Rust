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
//! Fail-closed honesty:
//! - Not full SalvageCrate collate / W3D gunner subobject swap matrix
//! - Not PassengersAllowedToFire (retail Technical passengers do not fire)
//! - Not full chassis reskin (ChassisOne/Two/Three) visual matrix
//! - Not network salvage / transport replication (network deferred)

use super::Weapon;

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

/// Tier 0 PrimaryDamage / AttackRange / DelayBetweenShots.
pub const TECH_MG_DAMAGE: f32 = 10.0;
pub const TECH_MG_RANGE: f32 = 150.0;
/// 200ms → 6 frames @ 30 FPS.
pub const TECH_MG_DELAY_FRAMES: u32 = 6;

/// Tier 1 PrimaryDamage / radius / range / delay.
pub const TECH_CANNON_DAMAGE: f32 = 45.0;
pub const TECH_CANNON_RADIUS: f32 = 25.0;
pub const TECH_CANNON_RANGE: f32 = 150.0;
/// 1000ms → 30 frames @ 30 FPS.
pub const TECH_CANNON_DELAY_FRAMES: u32 = 30;

/// Tier 2 PrimaryDamage / radius / range / min range / delay.
pub const TECH_RPG_DAMAGE: f32 = 50.0;
pub const TECH_RPG_RADIUS: f32 = 5.0;
pub const TECH_RPG_RANGE: f32 = 150.0;
pub const TECH_RPG_MIN_RANGE: f32 = 5.0;
/// 1000ms → 30 frames @ 30 FPS.
pub const TECH_RPG_DELAY_FRAMES: u32 = 30;

/// AP bullets WeaponBonus DAMAGE 125%.
pub const TECH_AP_DAMAGE_MULT: f32 = 1.25;

/// Residual fire audio.
pub const TECH_FIRE_AUDIO: &str = "TechnicalWeapon";

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
        TechnicalWeaponTier::Base => {
            (TECH_MG_DAMAGE, TECH_MG_RANGE, 0.0, TECH_MG_DELAY_FRAMES, 0.0)
        }
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
        assert!(
            (technical_splash_damage_at(TechnicalWeaponTier::One, false, 30.0)).abs() < 0.01
        );
        assert!(!should_apply_technical_splash(true, TechnicalWeaponTier::Base));
        assert!(should_apply_technical_splash(true, TechnicalWeaponTier::One));
    }

    #[test]
    fn transport_slots() {
        assert_eq!(TECHNICAL_TRANSPORT_SLOTS, 5);
    }
}
