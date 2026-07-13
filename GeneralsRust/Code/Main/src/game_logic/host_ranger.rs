//! Host USA Ranger residual (rifle primary + FlashBang grenade secondary splash).
//!
//! Residual slice (playability):
//! - `AmericaInfantryRanger` / USA_ / AirF_ / Lazr_ / SupW_ / Test variants spawn with
//!   PRIMARY `RangerAdvancedCombatRifle` (dmg **5** / range **100** / Delay **100**ms
//!   → 3 frames). ClipSize **3** honesty (volley matrix fail-closed).
//! - SECONDARY `RangerFlashBangGrenadeWeapon` residual (after FlashBang upgrade):
//!   PrimaryDamage **35** / radius **10** + SecondaryDamage **10** / radius **40**,
//!   AttackRange **175**, MinimumAttackRange **20**, ClipReload **2000**ms → 60 frames.
//! - PreferredAgainst residual: flashbang secondary preferred vs infantry / structures
//!   when secondary is equipped (damage 35 > 5). Host `select_combat_weapon_slot`
//!   already encodes this; residual fire path applies dual-radius splash.
//!
//! Fail-closed honesty:
//! - Not full SURRENDER DamageType infantry-surrender AI / garrison clear matrix
//! - Not full ClipSize=3 in-clip DelayBetweenShots + ClipReload 700ms volley
//! - Not full ScatterRadius projectile lob / PreAttackDelay flashbang anim lock
//! - Not network flashbang / fire replication (network deferred)

use super::Weapon;
use crate::game_logic::host_red_guard::delay_frames_to_reload_secs;

/// Retail primary weapon.
pub const RANGER_RIFLE_WEAPON: &str = "RangerAdvancedCombatRifle";
/// Retail secondary flashbang weapon.
pub const RANGER_FLASHBANG_WEAPON: &str = "RangerFlashBangGrenadeWeapon";
/// Retail FlashBang upgrade.
pub const UPGRADE_AMERICA_FLASHBANG: &str = "Upgrade_AmericaRangerFlashBangGrenade";

/// Retail PrimaryDamage base (rifle).
pub const RANGER_RIFLE_DAMAGE: f32 = 5.0;
/// Retail AttackRange (rifle).
pub const RANGER_RIFLE_RANGE: f32 = 100.0;
/// Retail DelayBetweenShots 100ms → 3 frames @ 30 FPS.
pub const RANGER_RIFLE_DELAY_FRAMES: u32 = 3;
/// Retail ClipSize residual honesty (fail-closed volley matrix).
pub const RANGER_RIFLE_CLIP_SIZE: u32 = 3;

/// Retail FlashBang PrimaryDamage.
pub const FLASHBANG_PRIMARY_DAMAGE: f32 = 35.0;
/// Retail FlashBang PrimaryDamageRadius.
pub const FLASHBANG_PRIMARY_RADIUS: f32 = 10.0;
/// Retail FlashBang SecondaryDamage.
pub const FLASHBANG_SECONDARY_DAMAGE: f32 = 10.0;
/// Retail FlashBang SecondaryDamageRadius.
pub const FLASHBANG_SECONDARY_RADIUS: f32 = 40.0;
/// Retail FlashBang AttackRange.
pub const FLASHBANG_RANGE: f32 = 175.0;
/// Retail FlashBang MinimumAttackRange.
pub const FLASHBANG_MIN_RANGE: f32 = 20.0;
/// Retail ClipReloadTime 2000ms → 60 frames @ 30 FPS.
pub const FLASHBANG_RELOAD_FRAMES: u32 = 60;
/// Retail WeaponSpeed residual.
pub const FLASHBANG_PROJECTILE_SPEED: f32 = 120.0;

/// Residual rifle fire audio.
pub const RANGER_RIFLE_FIRE_AUDIO: &str = "RangerWeapon";
/// Residual flashbang fire audio.
pub const RANGER_FLASHBANG_FIRE_AUDIO: &str = "RangerFlashBangWeapon";

/// Whether template is a residual USA Ranger infantry.
///
/// Fail-closed: name residual. Excludes weapons / flashbang / science / debris.
pub fn is_ranger_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("missile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("science")
        || n.contains("crate")
        || n.contains("locomotor")
        || n.contains("voice")
        || n.contains("flashbang")
        || n.contains("flash_bang")
        || n.contains("combatrifle")
        || n.contains("combat_rifle")
        || n.contains("pathfinder")
        || n.contains("missiledefender")
        || n.contains("missile_defender")
        || n.contains("colonel")
        || n.contains("burton")
        || n.contains("pilot")
        || n.contains("cia")
    {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "testranger"
        || n == "usa_ranger"
        || n == "goldenranger"
        || n == "airanger"
    {
        return true;
    }
    n.contains("ranger") || n.contains("infantryranger")
}

/// Whether residual fire should apply Ranger residual path.
pub fn should_apply_ranger_residual(is_ranger: bool) -> bool {
    is_ranger
}

/// Whether flashbang secondary is residual-equipped.
pub fn has_flashbang_equipped(has_secondary: bool, applied_upgrades: &std::collections::HashSet<String>) -> bool {
    if has_secondary {
        return true;
    }
    applied_upgrades.iter().any(|u| {
        let n = u.to_ascii_lowercase();
        n.contains("flashbang")
            || n.contains("flash_bang")
            || n == "upgrade_americarangerflashbanggrenade"
            || n.contains("americarangerflashbang")
    })
}

/// Build residual PRIMARY rifle Weapon.
pub fn ranger_rifle_weapon() -> Weapon {
    Weapon {
        damage: RANGER_RIFLE_DAMAGE,
        range: RANGER_RIFLE_RANGE,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(RANGER_RIFLE_DELAY_FRAMES),
        last_fire_time: 0.0,
        ammo: Some(RANGER_RIFLE_CLIP_SIZE),
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 999_999.0,
        pre_attack_delay: 0.0,
    }
}

/// Build residual SECONDARY flashbang Weapon.
pub fn ranger_flashbang_weapon() -> Weapon {
    Weapon {
        damage: FLASHBANG_PRIMARY_DAMAGE,
        range: FLASHBANG_RANGE,
        min_range: FLASHBANG_MIN_RANGE,
        reload_time: delay_frames_to_reload_secs(FLASHBANG_RELOAD_FRAMES),
        last_fire_time: 0.0,
        ammo: Some(1),
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: FLASHBANG_PROJECTILE_SPEED,
        pre_attack_delay: 0.0,
    }
}

/// (damage, range, delay_frames) for rifle residual.
pub fn ranger_rifle_stats() -> (f32, f32, u32) {
    (RANGER_RIFLE_DAMAGE, RANGER_RIFLE_RANGE, RANGER_RIFLE_DELAY_FRAMES)
}

/// Dual-radius flashbang residual damage at distance from impact.
///
/// - Intended target: full PrimaryDamage **35**
/// - Others within PrimaryDamageRadius **10**: PrimaryDamage **35**
/// - Others within SecondaryDamageRadius **40**: SecondaryDamage **10**
/// - Outside both: 0
pub fn flashbang_damage_at(is_intended_target: bool, distance_from_impact: f32) -> f32 {
    if is_intended_target {
        return FLASHBANG_PRIMARY_DAMAGE;
    }
    if distance_from_impact <= FLASHBANG_PRIMARY_RADIUS {
        FLASHBANG_PRIMARY_DAMAGE
    } else if distance_from_impact <= FLASHBANG_SECONDARY_RADIUS {
        FLASHBANG_SECONDARY_DAMAGE
    } else {
        0.0
    }
}

/// Legal residual fire / splash target.
pub fn is_legal_ranger_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind
}

/// Whether residual fire is flashbang secondary path (active_weapon_slot == 1).
pub fn is_flashbang_slot(active_weapon_slot: u8) -> bool {
    active_weapon_slot == 1
}

/// Prefer flashbang residual vs infantry / structure when secondary equipped.
pub fn ranger_prefer_flashbang(
    is_ranger: bool,
    has_flashbang: bool,
    target_is_infantry: bool,
    target_is_structure: bool,
) -> bool {
    is_ranger && has_flashbang && (target_is_infantry || target_is_structure)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ranger_name_matrix() {
        assert!(is_ranger_template("AmericaInfantryRanger"));
        assert!(is_ranger_template("USA_Ranger"));
        assert!(is_ranger_template("GoldenRanger"));
        assert!(is_ranger_template("AirF_AmericaInfantryRanger"));
        assert!(is_ranger_template("Lazr_AmericaInfantryRanger"));
        assert!(is_ranger_template("SupW_AmericaInfantryRanger"));
        assert!(is_ranger_template("TestRanger"));
        assert!(!is_ranger_template("RangerAdvancedCombatRifle"));
        assert!(!is_ranger_template("RangerFlashBangGrenadeWeapon"));
        assert!(!is_ranger_template("Upgrade_AmericaRangerFlashBangGrenade"));
        assert!(!is_ranger_template("AmericaInfantryPathfinder"));
        assert!(!is_ranger_template("AmericaInfantryMissileDefender"));
        assert!(!is_ranger_template("AmericaInfantryColonelBurton"));
        assert!(!is_ranger_template("GLAInfantryRebel"));
    }

    #[test]
    fn rifle_stats() {
        let (d, r, f) = ranger_rifle_stats();
        assert!((d - 5.0).abs() < 0.01);
        assert!((r - 100.0).abs() < 0.01);
        assert_eq!(f, 3);
        let w = ranger_rifle_weapon();
        assert!((w.damage - 5.0).abs() < 0.01);
        assert!((w.range - 100.0).abs() < 0.01);
        assert!((w.reload_time - (3.0 / 30.0)).abs() < 0.01);
        assert_eq!(w.ammo, Some(3));
        assert!(!w.can_target_air && w.can_target_ground);
    }

    #[test]
    fn flashbang_stats_and_splash() {
        let w = ranger_flashbang_weapon();
        assert!((w.damage - 35.0).abs() < 0.01);
        assert!((w.range - 175.0).abs() < 0.01);
        assert!((w.min_range - 20.0).abs() < 0.01);
        assert!((w.reload_time - (60.0 / 30.0)).abs() < 0.01);
        assert!((flashbang_damage_at(true, 100.0) - 35.0).abs() < 0.01);
        assert!((flashbang_damage_at(false, 5.0) - 35.0).abs() < 0.01);
        assert!((flashbang_damage_at(false, 10.0) - 35.0).abs() < 0.01);
        assert!((flashbang_damage_at(false, 25.0) - 10.0).abs() < 0.01);
        assert!((flashbang_damage_at(false, 40.0) - 10.0).abs() < 0.01);
        assert!((flashbang_damage_at(false, 40.1)).abs() < 0.01);
    }

    #[test]
    fn flashbang_equip_and_prefer() {
        let mut ups = HashSet::new();
        assert!(!has_flashbang_equipped(false, &ups));
        assert!(has_flashbang_equipped(true, &ups));
        ups.insert(UPGRADE_AMERICA_FLASHBANG.to_string());
        assert!(has_flashbang_equipped(false, &ups));
        assert!(ranger_prefer_flashbang(true, true, true, false));
        assert!(ranger_prefer_flashbang(true, true, false, true));
        assert!(!ranger_prefer_flashbang(true, true, false, false));
        assert!(!ranger_prefer_flashbang(true, false, true, false));
        assert!(!ranger_prefer_flashbang(false, true, true, false));
    }
}
