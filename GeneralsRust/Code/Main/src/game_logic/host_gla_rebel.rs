//! Host GLA Rebel residual (machine gun + AP Bullets damage upgrade).
//!
//! Residual slice (playability):
//! - `GLAInfantryRebel` / Chem_/Demo_/Slth_/GC_* variants spawn with PRIMARY
//!   `GLARebelMachineGun` (dmg **5** / range **100** / Delay **100**ms → 3 frames).
//! - Clip residual honesty: ClipSize **3** / ClipReload **700**ms (fail-closed vs
//!   full in-clip DelayBetweenShots volley matrix — host uses DelayBetweenShots
//!   as continuous residual cadence).
//! - AP Bullets PLAYER_UPGRADE residual (`Upgrade_GLAAPBullets`):
//!   WeaponBonus DAMAGE **125%** → PrimaryDamage **6.25**.
//! - Camouflage residual already closed via `host_upgrades` (not re-opened).
//!
//! Fail-closed honesty:
//! - Not full ClipSize=3 in-clip DelayBetweenShots 100ms + ClipReload 700ms volley
//! - Not CaptureBuilding / BoobyTrap special ability residual (separate)
//! - Not full StealthUpdate forbidden-condition matrix (Camouflage closed elsewhere)
//! - Not network AP / fire replication (network deferred)

use super::Weapon;
use crate::game_logic::host_red_guard::delay_frames_to_reload_secs;

/// Retail primary weapon.
pub const REBEL_MACHINE_GUN: &str = "GLARebelMachineGun";
/// Retail Upgrade_GLAAPBullets.
pub const UPGRADE_GLA_AP_BULLETS: &str = "Upgrade_GLAAPBullets";

/// Retail PrimaryDamage base.
pub const REBEL_DAMAGE: f32 = 5.0;
/// Retail AttackRange.
pub const REBEL_RANGE: f32 = 100.0;
/// Retail DelayBetweenShots 100ms → 3 frames @ 30 FPS.
pub const REBEL_BASE_DELAY_FRAMES: u32 = 3;
/// Retail ClipSize residual honesty (fail-closed volley matrix).
pub const REBEL_CLIP_SIZE: u32 = 3;
/// Retail ClipReloadTime 700ms → 21 frames @ 30 FPS (honesty only).
pub const REBEL_CLIP_RELOAD_FRAMES: u32 = 21;

/// AP Bullets WeaponBonus DAMAGE 125%.
pub const REBEL_AP_DAMAGE_MULT: f32 = 1.25;

/// Residual fire audio.
pub const REBEL_FIRE_AUDIO: &str = "RebelWeapon";

/// Whether template is a residual GLA Rebel infantry.
///
/// Fail-closed: name residual. Excludes weapons/biker/science/debris tokens.
pub fn is_gla_rebel_template(template_name: &str) -> bool {
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
        || n.contains("biker")
        || n.contains("machinegun")
        || n.contains("machine_gun")
        || n.contains("booby")
        || n.contains("ambush")
    {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "testrebel" || n == "testglarebel" || n == "gla_soldier" {
        return true;
    }
    // Rebel residual (not worker / hijacker / terrorist / tunnel defender).
    if n.contains("worker")
        || n.contains("hijacker")
        || n.contains("terrorist")
        || n.contains("tunneldefender")
        || n.contains("tunnel_defender")
        || n.contains("angrymob")
        || n.contains("angry_mob")
        || n.contains("jarmen")
        || n.contains("saboteur")
        || n.contains("hq")
    {
        return false;
    }
    n.contains("rebel") || n.contains("infantryrebel")
}

/// Whether upgrade set includes AP Bullets residual.
pub fn has_ap_bullets_upgrade(applied_upgrades: &std::collections::HashSet<String>) -> bool {
    applied_upgrades.iter().any(|u| {
        let n = u.to_ascii_lowercase();
        n.contains("apbullets")
            || n.contains("ap_bullets")
            || n == "upgrade_glaapbullets"
            || n.contains("glaapbullets")
    })
}

/// Apply AP Bullets residual damage mult when upgrade present.
pub fn rebel_damage_with_ap(has_ap_bullets: bool) -> f32 {
    if has_ap_bullets {
        REBEL_DAMAGE * REBEL_AP_DAMAGE_MULT
    } else {
        REBEL_DAMAGE
    }
}

/// Delay frames residual (base DelayBetweenShots; clip reload fail-closed).
pub fn rebel_delay_frames() -> u32 {
    REBEL_BASE_DELAY_FRAMES
}

/// (damage, range, delay_frames) for gun residual with optional AP.
pub fn rebel_weapon_stats(has_ap_bullets: bool) -> (f32, f32, u32) {
    (
        rebel_damage_with_ap(has_ap_bullets),
        REBEL_RANGE,
        rebel_delay_frames(),
    )
}

/// Build residual PRIMARY machine-gun Weapon with optional AP Bullets residual.
pub fn rebel_weapon(has_ap_bullets: bool) -> Weapon {
    let (damage, range, delay) = rebel_weapon_stats(has_ap_bullets);
    Weapon {
        damage,
        range,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(delay),
        last_fire_time: 0.0,
        ammo: Some(REBEL_CLIP_SIZE),
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 999_999.0,
        pre_attack_delay: 0.0,
    }
}

/// Whether residual fire should apply GLA Rebel residual path (gun honesty).
pub fn should_apply_rebel_residual(is_rebel: bool) -> bool {
    is_rebel
}

/// Legal residual fire target.
pub fn is_legal_rebel_target(
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
    use std::collections::HashSet;

    #[test]
    fn rebel_name_matrix() {
        assert!(is_gla_rebel_template("GLAInfantryRebel"));
        assert!(is_gla_rebel_template("GLA_Rebel"));
        assert!(is_gla_rebel_template("Demo_GLAInfantryRebel"));
        assert!(is_gla_rebel_template("Chem_GLAInfantryRebel"));
        assert!(is_gla_rebel_template("Slth_GLAInfantryRebel"));
        assert!(is_gla_rebel_template("GC_Chem_GLAInfantryRebel"));
        assert!(is_gla_rebel_template("TestRebel"));
        assert!(is_gla_rebel_template("GLA_Soldier"));
        assert!(!is_gla_rebel_template("GLARebelMachineGun"));
        assert!(!is_gla_rebel_template("GLARebelBikerMachineGun"));
        assert!(!is_gla_rebel_template("GLAInfantryTerrorist"));
        assert!(!is_gla_rebel_template("GLAInfantryTunnelDefender"));
        assert!(!is_gla_rebel_template("GLAInfantryWorker"));
        assert!(!is_gla_rebel_template("Upgrade_GLAAPBullets"));
        assert!(!is_gla_rebel_template("Upgrade_GLAInfantryRebelBoobyTrapAttack"));
        assert!(!is_gla_rebel_template("RebelWeapon"));
        assert!(!is_gla_rebel_template("ChinaInfantryRedguard"));
    }

    #[test]
    fn base_gun_stats() {
        let (d, r, f) = rebel_weapon_stats(false);
        assert!((d - 5.0).abs() < 0.01);
        assert!((r - 100.0).abs() < 0.01);
        assert_eq!(f, 3);
        let w = rebel_weapon(false);
        assert!((w.damage - 5.0).abs() < 0.01);
        assert!((w.range - 100.0).abs() < 0.01);
        assert!((w.reload_time - (3.0 / 30.0)).abs() < 0.01);
        assert_eq!(w.ammo, Some(3));
        assert!(!w.can_target_air && w.can_target_ground);
    }

    #[test]
    fn ap_bullets_damage() {
        assert!((rebel_damage_with_ap(false) - 5.0).abs() < 0.01);
        assert!((rebel_damage_with_ap(true) - 6.25).abs() < 0.01);
        let w = rebel_weapon(true);
        assert!((w.damage - 6.25).abs() < 0.01);
        // ROF unchanged by AP.
        assert!((w.reload_time - (3.0 / 30.0)).abs() < 0.01);
    }

    #[test]
    fn ap_upgrade_detect() {
        let mut tags = HashSet::new();
        assert!(!has_ap_bullets_upgrade(&tags));
        tags.insert(UPGRADE_GLA_AP_BULLETS.to_string());
        assert!(has_ap_bullets_upgrade(&tags));
        let mut tags2 = HashSet::new();
        tags2.insert("Upgrade_GLA_AP_Bullets".to_string());
        assert!(has_ap_bullets_upgrade(&tags2));
    }

    #[test]
    fn residual_gate() {
        assert!(should_apply_rebel_residual(true));
        assert!(!should_apply_rebel_residual(false));
        assert!(is_legal_rebel_target(true, false, false, true));
        assert!(!is_legal_rebel_target(false, false, false, true));
        assert!(!is_legal_rebel_target(true, true, false, true));
        assert!(!is_legal_rebel_target(true, false, true, true));
    }
}
