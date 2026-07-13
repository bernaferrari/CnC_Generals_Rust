//! Host GLA Jarmen Kell residual combat polish (sniper rifle + AP Bullets).
//!
//! Residual slice (playability):
//! - `GLAInfantryJarmenKell` / Chem_/Demo_/Slth_/GC_* / TestJarmenKell spawn with
//!   PRIMARY `GLAJarmenKellRifle` (dmg **180** / range **225** / Delay **1000**ms
//!   → 30 frames). DamageType SNIPER residual (intended-only; radius **0**).
//! - AP Bullets PLAYER_UPGRADE residual (`Upgrade_GLAAPBullets`): DAMAGE **125%**
//!   → PrimaryDamage **225**.
//! - Vehicle pilot-snipe special residual already closed via host_hero_abilities
//!   (`SnipeVehicle` / DAMAGE_KILLPILOT → unmanned) — not re-opened.
//! - Combat Cycle rider sniper residual remains host_combat_cycle (not re-opened).
//!
//! Fail-closed honesty:
//! - Not full SECONDARY AutoChooseSources=NONE pilot-sniper WeaponSet chooser matrix
//! - Not full StealthUpdate / Camouflage / Science prereq residual matrix
//! - Not full biker sniper Delay 750ms when dismounted (infantry stays 1000ms)
//! - Not network sniper / AP Bullets replication (network deferred)

use super::Weapon;
use crate::game_logic::host_red_guard::delay_frames_to_reload_secs;
use std::collections::HashSet;

/// Retail primary sniper weapon.
pub const JARMEN_KELL_RIFLE: &str = "GLAJarmenKellRifle";
/// Retail secondary pilot-snipe weapon (special residual; not host primary combat).
pub const JARMEN_KELL_PILOT_SNIPER: &str = "GLAJarmenKellVehiclePilotSniperRifle";
/// Retail Upgrade_GLAAPBullets (WeaponBonus PLAYER_UPGRADE DAMAGE 125%).
pub const UPGRADE_GLA_AP_BULLETS: &str = "Upgrade_GLAAPBullets";

/// Retail PrimaryDamage base (sniper).
pub const JARMEN_KELL_DAMAGE: f32 = 180.0;
/// Retail AttackRange.
pub const JARMEN_KELL_RANGE: f32 = 225.0;
/// Retail DelayBetweenShots 1000ms → 30 frames @ 30 FPS.
pub const JARMEN_KELL_DELAY_FRAMES: u32 = 30;
/// AP Bullets WeaponBonus DAMAGE 125%.
pub const JARMEN_KELL_AP_DAMAGE_MULT: f32 = 1.25;

/// Residual sniper fire audio.
pub const JARMEN_KELL_FIRE_AUDIO: &str = "JarmenKellWeapon";

/// Whether template is a residual Jarmen Kell hero infantry.
///
/// Fail-closed: name residual. Excludes weapons / science / biker tokens as unit.
pub fn is_jarmen_kell_template(template_name: &str) -> bool {
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
        || n.contains("command")
        || n.contains("button")
        || n.contains("portrait")
        || n.contains("biker")
        || n.contains("combatbike")
        || n.contains("combat_bike")
        || n.ends_with("rifle")
        || n.contains("sniper")
        || n.contains("pilotsniper")
    {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "testjarmenkell"
        || n == "test_jarmen_kell"
        || n == "testkell"
        || n == "gla_jarmenkell"
        || n == "gla_jarmen_kell"
        || n == "gla_kell"
    {
        return true;
    }
    n.contains("jarmenkell")
        || n.contains("jarmen_kell")
        || (n.contains("jarmen") && n.contains("kell"))
        || (n.contains("infantry") && n.contains("kell") && !n.contains("rebel"))
}

/// Whether residual fire should apply Jarmen Kell sniper residual path.
pub fn should_apply_jarmen_kell_residual(is_kell: bool) -> bool {
    is_kell
}

/// Whether AP Bullets upgrade tag is present.
pub fn has_ap_bullets_upgrade(applied_upgrades: &HashSet<String>) -> bool {
    applied_upgrades.iter().any(|u| {
        let l = u.to_ascii_lowercase();
        l.contains("apbullets")
            || l == UPGRADE_GLA_AP_BULLETS.to_ascii_lowercase()
            || l.contains("gla_ap_bullets")
    })
}

/// Apply AP Bullets residual damage mult when upgrade present.
pub fn jarmen_kell_damage_with_ap(has_ap: bool) -> f32 {
    if has_ap {
        JARMEN_KELL_DAMAGE * JARMEN_KELL_AP_DAMAGE_MULT
    } else {
        JARMEN_KELL_DAMAGE
    }
}

/// (damage, range, delay_frames) for sniper residual.
pub fn jarmen_kell_weapon_stats(has_ap: bool) -> (f32, f32, u32) {
    (
        jarmen_kell_damage_with_ap(has_ap),
        JARMEN_KELL_RANGE,
        JARMEN_KELL_DELAY_FRAMES,
    )
}

/// Build residual PRIMARY sniper Weapon.
pub fn jarmen_kell_weapon(has_ap: bool) -> Weapon {
    let (damage, range, delay) = jarmen_kell_weapon_stats(has_ap);
    Weapon {
        damage,
        range,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(delay),
        last_fire_time: 0.0,
        ammo: None,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 999_999.0,
        pre_attack_delay: 0.0,
    }
}

/// Legal residual fire target (intended-only sniper residual).
pub fn is_legal_jarmen_kell_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jarmen_kell_name_matrix() {
        assert!(is_jarmen_kell_template("GLAInfantryJarmenKell"));
        assert!(is_jarmen_kell_template("TestJarmenKell"));
        assert!(is_jarmen_kell_template("GLA_JarmenKell"));
        assert!(is_jarmen_kell_template("Slth_GLAInfantryJarmenKell"));
        assert!(is_jarmen_kell_template("Demo_GLAInfantryJarmenKell"));
        assert!(is_jarmen_kell_template("Chem_GLAInfantryJarmenKell"));
        assert!(is_jarmen_kell_template("GC_Slth_GLAInfantryJarmenKell"));
        assert!(!is_jarmen_kell_template("GLAJarmenKellRifle"));
        assert!(!is_jarmen_kell_template("GLABikerKellSniperRifle"));
        assert!(!is_jarmen_kell_template("GLAJarmenKellVehiclePilotSniperRifle"));
        assert!(!is_jarmen_kell_template("GLAInfantryRebel"));
        assert!(!is_jarmen_kell_template("AmericaInfantryColonelBurton"));
        assert!(!is_jarmen_kell_template("GLAVehicleCombatBike"));
    }

    #[test]
    fn weapon_and_ap_bullets() {
        let w = jarmen_kell_weapon(false);
        assert!((w.damage - 180.0).abs() < 0.01);
        assert!((w.range - 225.0).abs() < 0.01);
        assert!((w.reload_time - 1.0).abs() < 0.05);

        let wap = jarmen_kell_weapon(true);
        assert!((wap.damage - 225.0).abs() < 0.01);

        let mut tags = HashSet::new();
        assert!(!has_ap_bullets_upgrade(&tags));
        tags.insert(UPGRADE_GLA_AP_BULLETS.to_string());
        assert!(has_ap_bullets_upgrade(&tags));
    }
}
