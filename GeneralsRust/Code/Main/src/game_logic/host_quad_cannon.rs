//! Host GLA Quad Cannon residual (ground gun + anti-air secondary + multi-barrel).
//!
//! Residual slice (playability):
//! - Spawns with PRIMARY `QuadCannonGun` (ground, range 150, dmg 10) and
//!   SECONDARY `QuadCannonGunAir` (air only, range 350, dmg 5, AntiGround=No).
//! - Weapon chooser residual: airborne targets fire secondary AA gun; ground
//!   targets fire primary (via can_target_air / can_target_ground residual).
//! - Multi-barrel salvage residual tiers:
//!   - Tier 0: DelayBetweenShots 100ms
//!   - Tier 1 (CRATEUPGRADE_ONE): 50ms, ground dmg residual 8
//!   - Tier 2 (CRATEUPGRADE_TWO): 25ms, ground dmg residual 8
//! - AP Bullets residual: damage × 1.25 when upgrade tag present (WeaponBonus).
//!
//! Fail-closed honesty:
//! - Not full SalvageCrate collate / W3D turret subobject swap matrix
//! - Not full Heroic VeterancyFireFX / pitch-turret AI matrix
//! - Not network salvage / weapon-set replication (network deferred)

/// Retail primary ground gun.
pub const QUAD_CANNON_GUN: &str = "QuadCannonGun";
/// Retail secondary anti-air gun.
pub const QUAD_CANNON_GUN_AIR: &str = "QuadCannonGunAir";
/// Retail upgrade-one ground / air.
pub const QUAD_CANNON_GUN_UPGRADE_ONE: &str = "QuadCannonGunUpgradeOne";
pub const QUAD_CANNON_GUN_UPGRADE_ONE_AIR: &str = "QuadCannonGunUpgradeOneAir";
/// Retail upgrade-two ground / air.
pub const QUAD_CANNON_GUN_UPGRADE_TWO: &str = "QuadCannonGunUpgradeTwo";
pub const QUAD_CANNON_GUN_UPGRADE_TWO_AIR: &str = "QuadCannonGunUpgradeTwoAir";
/// Retail Upgrade_GLAAPBullets.
pub const UPGRADE_GLA_AP_BULLETS: &str = "Upgrade_GLAAPBullets";

/// Retail QuadCannonGun PrimaryDamage.
pub const QUAD_GROUND_DAMAGE: f32 = 10.0;
/// Retail QuadCannonGun AttackRange.
pub const QUAD_GROUND_RANGE: f32 = 150.0;
/// Retail QuadCannonGunAir PrimaryDamage.
pub const QUAD_AIR_DAMAGE: f32 = 5.0;
/// Retail QuadCannonGunAir AttackRange.
pub const QUAD_AIR_RANGE: f32 = 350.0;
/// Retail DelayBetweenShots 100ms → 3 frames @ 30 FPS.
pub const QUAD_DELAY_FRAMES_TIER0: u32 = 3;
/// Retail UpgradeOne Delay 50ms → 2 frames @ 30 FPS (ceil).
pub const QUAD_DELAY_FRAMES_TIER1: u32 = 2;
/// Retail UpgradeTwo Delay 25ms → 1 frame @ 30 FPS.
pub const QUAD_DELAY_FRAMES_TIER2: u32 = 1;
/// UpgradeOne/Two ground PrimaryDamage residual.
pub const QUAD_GROUND_DAMAGE_UPGRADED: f32 = 8.0;
/// AP bullets WeaponBonus DAMAGE 125%.
pub const QUAD_AP_DAMAGE_MULT: f32 = 1.25;
/// Residual fire audio.
pub const QUAD_FIRE_AUDIO: &str = "QuadCannonWeapon";

/// Multi-barrel salvage residual tier (WEAPONSET_CRATEUPGRADE).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QuadCannonBarrelTier {
    #[default]
    Base = 0,
    One = 1,
    Two = 2,
}

impl QuadCannonBarrelTier {
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

/// Whether template is a residual Quad Cannon vehicle.
///
/// Fail-closed: name residual (not full Salvage / W3D turret matrix).
pub fn is_quad_cannon_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    // Weapon / upgrade tokens (QuadCannonGun, QuadCannonGunAir, Upgrade_…)
    // are not the living vehicle residual.
    if n.contains("gun") || n.contains("weapon") || n.starts_with("upgrade") {
        return false;
    }
    n.contains("quadcannon")
        || n.contains("quad_cannon")
        || n == "gla_quadcannon"
        || n == "testquadcannon"
}

/// Whether residual target is airborne (AA secondary path).
pub fn target_is_airborne_for_quad(is_aircraft: bool, airborne_target: bool) -> bool {
    is_aircraft || airborne_target
}

/// Slot residual for Quad Cannon: 1 = AA secondary, 0 = ground primary.
///
/// Fail-closed: not full PreferredAgainst matrix beyond air/ground anti masks.
pub fn preferred_quad_slot(target_is_air: bool) -> u8 {
    if target_is_air {
        1
    } else {
        0
    }
}

/// Ground gun residual stats for salvage tier.
pub fn quad_ground_stats(tier: QuadCannonBarrelTier) -> (f32, f32, u32) {
    // (damage, range, delay_frames)
    match tier {
        QuadCannonBarrelTier::Base => (QUAD_GROUND_DAMAGE, QUAD_GROUND_RANGE, QUAD_DELAY_FRAMES_TIER0),
        QuadCannonBarrelTier::One | QuadCannonBarrelTier::Two => {
            let delay = if tier == QuadCannonBarrelTier::Two {
                QUAD_DELAY_FRAMES_TIER2
            } else {
                QUAD_DELAY_FRAMES_TIER1
            };
            (QUAD_GROUND_DAMAGE_UPGRADED, QUAD_GROUND_RANGE, delay)
        }
    }
}

/// Air gun residual stats for salvage tier.
pub fn quad_air_stats(tier: QuadCannonBarrelTier) -> (f32, f32, u32) {
    let delay = match tier {
        QuadCannonBarrelTier::Base => QUAD_DELAY_FRAMES_TIER0,
        QuadCannonBarrelTier::One => QUAD_DELAY_FRAMES_TIER1,
        QuadCannonBarrelTier::Two => QUAD_DELAY_FRAMES_TIER2,
    };
    (QUAD_AIR_DAMAGE, QUAD_AIR_RANGE, delay)
}

/// Apply AP bullets residual damage mult when upgrade present.
pub fn quad_damage_with_ap(base_damage: f32, has_ap_bullets: bool) -> f32 {
    if has_ap_bullets {
        base_damage * QUAD_AP_DAMAGE_MULT
    } else {
        base_damage
    }
}

/// Reload time seconds residual for delay frames @ 30 FPS.
pub fn delay_frames_to_reload_secs(delay_frames: u32) -> f32 {
    (delay_frames.max(1) as f32) / 30.0
}

/// Primary/secondary weapon names for residual tier.
pub fn quad_weapon_names_for_tier(tier: QuadCannonBarrelTier) -> (&'static str, &'static str) {
    match tier {
        QuadCannonBarrelTier::Base => (QUAD_CANNON_GUN, QUAD_CANNON_GUN_AIR),
        QuadCannonBarrelTier::One => (QUAD_CANNON_GUN_UPGRADE_ONE, QUAD_CANNON_GUN_UPGRADE_ONE_AIR),
        QuadCannonBarrelTier::Two => (QUAD_CANNON_GUN_UPGRADE_TWO, QUAD_CANNON_GUN_UPGRADE_TWO_AIR),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quad_cannon_name_matrix() {
        assert!(is_quad_cannon_template("GLAVehicleQuadCannon"));
        assert!(is_quad_cannon_template("Chem_GLAVehicleQuadCannon"));
        assert!(is_quad_cannon_template("Demo_GLAVehicleQuadCannon"));
        assert!(is_quad_cannon_template("Slth_GLAVehicleQuadCannon"));
        assert!(is_quad_cannon_template("TestQuadCannon"));
        assert!(is_quad_cannon_template("GLA_QuadCannon"));
        assert!(!is_quad_cannon_template("QuadCannonGun"));
        assert!(!is_quad_cannon_template("QuadCannonGunAir"));
        assert!(!is_quad_cannon_template("GLAVehicleRocketBuggy"));
        assert!(!is_quad_cannon_template("USA_Ranger"));
    }

    #[test]
    fn air_slot_preference() {
        assert_eq!(preferred_quad_slot(true), 1);
        assert_eq!(preferred_quad_slot(false), 0);
        assert!(target_is_airborne_for_quad(true, false));
        assert!(target_is_airborne_for_quad(false, true));
        assert!(!target_is_airborne_for_quad(false, false));
    }

    #[test]
    fn multi_barrel_stats() {
        let (d0, r0, f0) = quad_ground_stats(QuadCannonBarrelTier::Base);
        assert!((d0 - 10.0).abs() < 0.01);
        assert!((r0 - 150.0).abs() < 0.01);
        assert_eq!(f0, 3);
        let (d1, _, f1) = quad_ground_stats(QuadCannonBarrelTier::One);
        assert!((d1 - 8.0).abs() < 0.01);
        assert_eq!(f1, 2);
        let (_, _, f2) = quad_ground_stats(QuadCannonBarrelTier::Two);
        assert_eq!(f2, 1);
        let (ad, ar, _) = quad_air_stats(QuadCannonBarrelTier::Base);
        assert!((ad - 5.0).abs() < 0.01);
        assert!((ar - 350.0).abs() < 0.01);
    }

    #[test]
    fn ap_bullets_mult() {
        assert!((quad_damage_with_ap(10.0, false) - 10.0).abs() < 0.01);
        assert!((quad_damage_with_ap(10.0, true) - 12.5).abs() < 0.01);
    }

    #[test]
    fn weapon_names_per_tier() {
        assert_eq!(
            quad_weapon_names_for_tier(QuadCannonBarrelTier::Base),
            (QUAD_CANNON_GUN, QUAD_CANNON_GUN_AIR)
        );
        assert_eq!(
            quad_weapon_names_for_tier(QuadCannonBarrelTier::Two).0,
            QUAD_CANNON_GUN_UPGRADE_TWO
        );
    }
}
