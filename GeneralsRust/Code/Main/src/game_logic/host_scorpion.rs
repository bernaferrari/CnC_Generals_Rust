//! Host GLA Scorpion tank residual (main gun + salvage + Scorpion Rocket secondary).
//!
//! Residual slice (playability):
//! - `GLATankScorpion` / `GLA_ScorpionTank` / variants spawn with PRIMARY
//!   `ScorpionTankGun` (dmg **20** / radius **5** / range **150** / Delay **1000**ms →
//!   30 frames / WeaponSpeed 400).
//! - Salvage residual (`WEAPON_SALVAGER` / CRATEUPGRADE):
//!   - Tier 0: PrimaryDamage **20** (`ScorpionTankGun`)
//!   - Tier 1+ CRATEUPGRADE: PrimaryDamage **25** (`ScorpionTankGunPlusOne`)
//!     (retail: PlusTwo keeps PlusOne gun damage — no further primary bonus)
//! - `Upgrade_GLAScorpionRocket` PLAYER_UPGRADE residual equips SECONDARY
//!   `ScorpionMissileWeapon` (Primary **100**/radius **5** + Secondary **80**/radius **25**,
//!   range **150**, min **40**, ClipReload **15000**ms → 450 frames).
//! - AP Rockets PLAYER_UPGRADE residual (`Upgrade_GLAAPRockets`) on the missile:
//!   WeaponBonus DAMAGE **125%** on primary + secondary rings.
//! - CRATEUPGRADE_TWO + rocket residual: ClipSize **2** honesty (dual-missile residual
//!   collapses to same reload with ClipSize=2 flag; not full dual-shot cadence).
//!
//! Fail-closed honesty:
//! - Not full SalvageCrate W3D turret / missile-rack subobject swap matrix
//! - Not full ClipSize=2 DelayBetweenShots 200ms dual-volley cadence
//! - Not full ScatterRadiusVsInfantry / projectile exhaust FX matrix
//! - Not network salvage / rocket replication (network deferred)

use super::Weapon;
use std::collections::HashSet;

/// Retail primary base gun.
pub const SCORPION_TANK_GUN: &str = "ScorpionTankGun";
/// Retail CRATEUPGRADE_ONE primary gun (damage up).
pub const SCORPION_TANK_GUN_PLUS_ONE: &str = "ScorpionTankGunPlusOne";
/// Retail secondary rocket after Upgrade_GLAScorpionRocket.
pub const SCORPION_MISSILE_WEAPON: &str = "ScorpionMissileWeapon";
/// Retail dual-clip rocket at CRATEUPGRADE_TWO + PLAYER_UPGRADE.
pub const SCORPION_MISSILE_WEAPON_PLUS_TWO: &str = "ScorpionMissileWeaponPlusTwo";
/// Retail Upgrade_GLAScorpionRocket (WeaponSetUpgrade → PLAYER_UPGRADE secondary).
pub const UPGRADE_GLA_SCORPION_ROCKET: &str = "Upgrade_GLAScorpionRocket";
/// Retail Upgrade_GLAAPRockets (missile damage mult residual).
pub const UPGRADE_GLA_AP_ROCKETS: &str = "Upgrade_GLAAPRockets";

/// Retail PrimaryDamage base gun.
pub const SCORPION_GUN_DAMAGE: f32 = 20.0;
/// Retail PrimaryDamage salvage PlusOne gun.
pub const SCORPION_GUN_DAMAGE_PLUS: f32 = 25.0;
/// Retail PrimaryDamageRadius residual splash (gun).
pub const SCORPION_GUN_SPLASH_RADIUS: f32 = 5.0;
/// Retail AttackRange (gun + missile).
pub const SCORPION_RANGE: f32 = 150.0;
/// Retail DelayBetweenShots 1000ms → 30 frames @ 30 FPS (gun).
pub const SCORPION_GUN_DELAY_FRAMES: u32 = 30;
/// Retail WeaponSpeed residual (shell flight residual; host hits still residual-instant).
pub const SCORPION_GUN_PROJECTILE_SPEED: f32 = 400.0;

/// Retail ScorpionMissileWeapon PrimaryDamage.
pub const SCORPION_MISSILE_PRIMARY_DAMAGE: f32 = 100.0;
/// Retail PrimaryDamageRadius.
pub const SCORPION_MISSILE_PRIMARY_RADIUS: f32 = 5.0;
/// Retail SecondaryDamage.
pub const SCORPION_MISSILE_SECONDARY_DAMAGE: f32 = 80.0;
/// Retail SecondaryDamageRadius.
pub const SCORPION_MISSILE_SECONDARY_RADIUS: f32 = 25.0;
/// Retail MinimumAttackRange (missile).
pub const SCORPION_MISSILE_MIN_RANGE: f32 = 40.0;
/// Retail ClipReloadTime 15000ms → 450 frames @ 30 FPS.
pub const SCORPION_MISSILE_RELOAD_FRAMES: u32 = 450;
/// Retail WeaponSpeed residual for missile.
pub const SCORPION_MISSILE_PROJECTILE_SPEED: f32 = 600.0;
/// AP Rockets WeaponBonus DAMAGE 125%.
pub const SCORPION_AP_DAMAGE_MULT: f32 = 1.25;

/// Residual fire audio (gun).
pub const SCORPION_GUN_FIRE_AUDIO: &str = "ScorpionTankWeapon";
/// Residual fire audio (missile).
pub const SCORPION_MISSILE_FIRE_AUDIO: &str = "ScorpionMissileWeapon";

/// Multi-weapon salvage residual tier for primary gun damage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScorpionSalvageTier {
    #[default]
    Base = 0,
    One = 1,
    Two = 2,
}

impl ScorpionSalvageTier {
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

    /// Retail primary damage for salvage tier (PlusTwo keeps PlusOne gun).
    pub fn gun_damage(self) -> f32 {
        match self {
            Self::Base => SCORPION_GUN_DAMAGE,
            Self::One | Self::Two => SCORPION_GUN_DAMAGE_PLUS,
        }
    }

    /// Whether CRATEUPGRADE_TWO dual-missile clip residual is active.
    pub fn dual_missile_clip(self) -> bool {
        matches!(self, Self::Two)
    }
}

/// Whether template is a residual Scorpion tank chassis.
///
/// Fail-closed: name residual. Excludes weapons/shells/debris/science tokens.
pub fn is_scorpion_template(template_name: &str) -> bool {
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
        || n.contains("gun")
        || n.contains("crate")
        || n.contains("locomotor")
        || n.contains("voice")
        || n.contains("rack")
    {
        return false;
    }
    n.contains("scorpion") || n == "gla_scorpion" || n == "gla_scorpiontank" || n == "testscorpion"
}

/// Whether residual fire should apply Scorpion residual path.
pub fn should_apply_scorpion_residual(is_scorpion: bool) -> bool {
    is_scorpion
}

/// Whether Scorpion Rocket PLAYER_UPGRADE tag is present.
pub fn has_scorpion_rocket_upgrade(applied_upgrades: &HashSet<String>) -> bool {
    applied_upgrades.iter().any(|u| {
        let l = u.to_ascii_lowercase();
        l.contains("scorpionrocket")
            || l == "upgrade_glascorpionrocket"
            || l.contains("gla_scorpion_rocket")
    })
}

/// Whether AP Rockets upgrade tag is present (missile damage mult).
pub fn has_ap_rockets_upgrade(applied_upgrades: &HashSet<String>) -> bool {
    applied_upgrades.iter().any(|u| {
        let l = u.to_ascii_lowercase();
        l.contains("aprockets")
            || l == UPGRADE_GLA_AP_ROCKETS.to_ascii_lowercase()
            || l.contains("gla_ap_rockets")
    })
}

/// Infer salvage tier from upgrade tags (caller may also pass explicit tier).
pub fn salvage_tier_from_upgrades(applied_upgrades: &HashSet<String>) -> ScorpionSalvageTier {
    if applied_upgrades.iter().any(|u| {
        let l = u.to_ascii_lowercase();
        l.contains("crateupgrade_two")
            || l.contains("crateupgradetwo")
            || l.contains("weaponset_crateupgrade_two")
    }) {
        return ScorpionSalvageTier::Two;
    }
    if applied_upgrades.iter().any(|u| {
        let l = u.to_ascii_lowercase();
        l.contains("crateupgrade_one")
            || l.contains("crateupgradeone")
            || l.contains("weaponset_crateupgrade_one")
    }) {
        return ScorpionSalvageTier::One;
    }
    ScorpionSalvageTier::Base
}

/// Primary gun weapon name for salvage tier.
pub fn scorpion_gun_name_for_tier(tier: ScorpionSalvageTier) -> &'static str {
    match tier {
        ScorpionSalvageTier::Base => SCORPION_TANK_GUN,
        ScorpionSalvageTier::One | ScorpionSalvageTier::Two => SCORPION_TANK_GUN_PLUS_ONE,
    }
}

/// Reload time seconds residual for delay frames @ 30 FPS.
pub fn delay_frames_to_reload_secs(delay_frames: u32) -> f32 {
    (delay_frames.max(1) as f32) / 30.0
}

/// (damage, range, delay_frames, splash_radius, projectile_speed) for primary gun.
pub fn scorpion_gun_stats(tier: ScorpionSalvageTier) -> (f32, f32, u32, f32, f32) {
    (
        tier.gun_damage(),
        SCORPION_RANGE,
        SCORPION_GUN_DELAY_FRAMES,
        SCORPION_GUN_SPLASH_RADIUS,
        SCORPION_GUN_PROJECTILE_SPEED,
    )
}

/// Build residual primary gun Weapon for salvage tier.
pub fn scorpion_gun_weapon(tier: ScorpionSalvageTier) -> Weapon {
    let (damage, range, delay, _splash, speed) = scorpion_gun_stats(tier);
    Weapon {
        damage,
        range,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(delay),
        last_fire_time: 0.0,
        ammo: None,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: speed,
        pre_attack_delay: 0.0,
    }
}

/// Missile primary/secondary ring damage with optional AP mult.
pub fn scorpion_missile_ring_damage(has_ap: bool) -> (f32, f32) {
    let mult = if has_ap { SCORPION_AP_DAMAGE_MULT } else { 1.0 };
    (
        SCORPION_MISSILE_PRIMARY_DAMAGE * mult,
        SCORPION_MISSILE_SECONDARY_DAMAGE * mult,
    )
}

/// Build residual secondary missile Weapon (rocket upgrade).
pub fn scorpion_missile_weapon(has_ap: bool, dual_clip: bool) -> Weapon {
    let (primary_dmg, _sec) = scorpion_missile_ring_damage(has_ap);
    // Host Weapon stores primary ring damage; dual-radius splash applied at fire.
    // ClipSize residual is honesty-only (ammo None); reload stays ClipReload residual.
    let _ = dual_clip;
    Weapon {
        damage: primary_dmg,
        range: SCORPION_RANGE,
        min_range: SCORPION_MISSILE_MIN_RANGE,
        reload_time: delay_frames_to_reload_secs(SCORPION_MISSILE_RELOAD_FRAMES),
        last_fire_time: 0.0,
        ammo: None,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: SCORPION_MISSILE_PROJECTILE_SPEED,
        pre_attack_delay: 0.0,
    }
}

/// Primary gun splash residual damage at distance from impact.
///
/// Intended target takes full PrimaryDamage; others within PrimaryDamageRadius
/// take full PrimaryDamage residual (fail-closed vs continuous falloff).
pub fn scorpion_gun_splash_damage_at(
    is_intended_target: bool,
    distance_from_impact: f32,
    gun_damage: f32,
) -> f32 {
    if is_intended_target {
        return gun_damage;
    }
    if distance_from_impact <= SCORPION_GUN_SPLASH_RADIUS {
        gun_damage
    } else {
        0.0
    }
}

/// Missile dual-radius residual damage at distance (max of rings).
pub fn scorpion_missile_damage_at(distance_from_impact: f32, has_ap: bool) -> f32 {
    let (primary, secondary) = scorpion_missile_ring_damage(has_ap);
    if distance_from_impact <= SCORPION_MISSILE_PRIMARY_RADIUS {
        primary
    } else if distance_from_impact <= SCORPION_MISSILE_SECONDARY_RADIUS {
        secondary
    } else {
        0.0
    }
}

/// Legal residual splash target.
pub fn is_legal_scorpion_splash_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind
}

// --- Wave 62 thin residual honesty pack (optional peel) ---

/// Scorpion gun + salvage residual honesty.
pub fn honesty_scorpion_gun_residual_ok() -> bool {
    (SCORPION_GUN_DAMAGE - 20.0).abs() < 0.01
        && (SCORPION_GUN_DAMAGE_PLUS - 25.0).abs() < 0.01
        && (SCORPION_GUN_SPLASH_RADIUS - 5.0).abs() < 0.01
        && (SCORPION_RANGE - 150.0).abs() < 0.01
        && SCORPION_GUN_DELAY_FRAMES == 30
        && (SCORPION_GUN_PROJECTILE_SPEED - 400.0).abs() < 0.1
        && SCORPION_TANK_GUN == "ScorpionTankGun"
        && SCORPION_TANK_GUN_PLUS_ONE == "ScorpionTankGunPlusOne"
}

/// Scorpion rocket residual honesty (Upgrade + AP + dual clip).
pub fn honesty_scorpion_rocket_residual_ok() -> bool {
    (SCORPION_MISSILE_PRIMARY_DAMAGE - 100.0).abs() < 0.01
        && (SCORPION_MISSILE_PRIMARY_RADIUS - 5.0).abs() < 0.01
        && (SCORPION_MISSILE_SECONDARY_DAMAGE - 80.0).abs() < 0.01
        && (SCORPION_MISSILE_SECONDARY_RADIUS - 25.0).abs() < 0.01
        && (SCORPION_MISSILE_MIN_RANGE - 40.0).abs() < 0.01
        && SCORPION_MISSILE_RELOAD_FRAMES == 450
        && (SCORPION_AP_DAMAGE_MULT - 1.25).abs() < 0.001
        && UPGRADE_GLA_SCORPION_ROCKET == "Upgrade_GLAScorpionRocket"
        && UPGRADE_GLA_AP_ROCKETS == "Upgrade_GLAAPRockets"
        && SCORPION_MISSILE_WEAPON == "ScorpionMissileWeapon"
}

/// Combined Wave 62 scorpion thin residual honesty pack.
pub fn honesty_scorpion_residual_pack_ok() -> bool {
    honesty_scorpion_gun_residual_ok() && honesty_scorpion_rocket_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scorpion_name_matrix() {
        assert!(is_scorpion_template("GLATankScorpion"));
        assert!(is_scorpion_template("GLA_ScorpionTank"));
        assert!(is_scorpion_template("GLA_Scorpion"));
        assert!(is_scorpion_template("TestScorpion"));
        assert!(is_scorpion_template("Chem_GLATankScorpion"));
        assert!(is_scorpion_template("Demo_GLATankScorpion"));
        assert!(is_scorpion_template("Slth_GLATankScorpion"));
        assert!(!is_scorpion_template("ScorpionTankGun"));
        assert!(!is_scorpion_template("ScorpionMissileWeapon"));
        assert!(!is_scorpion_template("ScorpionTankShell"));
        assert!(!is_scorpion_template("Upgrade_GLAScorpionRocket"));
        assert!(!is_scorpion_template("ScorpionLocomotor"));
        assert!(!is_scorpion_template("GLATankMarauder"));
        assert!(!is_scorpion_template("USA_Ranger"));
    }

    #[test]
    fn gun_stats_and_salvage_tiers() {
        let (d0, r, f, s, sp) = scorpion_gun_stats(ScorpionSalvageTier::Base);
        assert!((d0 - 20.0).abs() < 0.01);
        assert!((r - 150.0).abs() < 0.01);
        assert_eq!(f, 30);
        assert!((s - 5.0).abs() < 0.01);
        assert!((sp - 400.0).abs() < 0.01);

        let (d1, _, _, _, _) = scorpion_gun_stats(ScorpionSalvageTier::One);
        assert!((d1 - 25.0).abs() < 0.01);
        let (d2, _, _, _, _) = scorpion_gun_stats(ScorpionSalvageTier::Two);
        assert!((d2 - 25.0).abs() < 0.01);

        let w = scorpion_gun_weapon(ScorpionSalvageTier::Base);
        assert!((w.damage - 20.0).abs() < 0.01);
        assert!((w.reload_time - 1.0).abs() < 0.02);
    }

    #[test]
    fn rocket_and_ap_damage() {
        let (p, s) = scorpion_missile_ring_damage(false);
        assert!((p - 100.0).abs() < 0.01);
        assert!((s - 80.0).abs() < 0.01);
        let (pa, sa) = scorpion_missile_ring_damage(true);
        assert!((pa - 125.0).abs() < 0.01);
        assert!((sa - 100.0).abs() < 0.01);

        assert!((scorpion_missile_damage_at(0.0, false) - 100.0).abs() < 0.01);
        assert!((scorpion_missile_damage_at(10.0, false) - 80.0).abs() < 0.01);
        assert!((scorpion_missile_damage_at(30.0, false)).abs() < 0.01);

        let m = scorpion_missile_weapon(false, false);
        assert!((m.min_range - 40.0).abs() < 0.01);
        assert!((m.reload_time - 15.0).abs() < 0.05);
    }

    #[test]
    fn upgrade_tag_detect() {
        let mut set = HashSet::new();
        assert!(!has_scorpion_rocket_upgrade(&set));
        set.insert(UPGRADE_GLA_SCORPION_ROCKET.to_string());
        assert!(has_scorpion_rocket_upgrade(&set));
        set.insert(UPGRADE_GLA_AP_ROCKETS.to_string());
        assert!(has_ap_rockets_upgrade(&set));
        set.insert("WEAPONSET_CRATEUPGRADE_TWO".to_string());
        assert_eq!(salvage_tier_from_upgrades(&set), ScorpionSalvageTier::Two);
    }

    #[test]
    fn scorpion_residual_pack_honesty() {
        assert!(honesty_scorpion_gun_residual_ok());
        assert!(honesty_scorpion_rocket_residual_ok());
        assert!(honesty_scorpion_residual_pack_ok());
    }
}
