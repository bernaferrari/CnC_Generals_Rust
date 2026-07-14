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
//! Wave 60 residual pack (retail INI honesty):
//! - Gun residual: PrimaryDamage **5**, AttackRange **100**, Delay **100**ms → **3**f,
//!   ClipSize **3**, ClipReload **700**ms → **21**f, DamageType **SMALL_ARMS**,
//!   FireSound **RebelWeapon**, FireFX **WeaponFX_GenericMachineGunFire**,
//!   radius **0** (intended-only).
//! - Capture residual: `SpecialAbilityRebelCaptureBuilding` Reload **15000**ms → **450**f,
//!   StartAbilityRange **5**, Unpack **3000**ms → **90**f, Prep **20000**ms → **600**f,
//!   Pack **2000**ms → **60**f, AwardXP **12**, gated by Upgrade_InfantryCaptureBuilding.
//! - Body residual: MaxHealth **120**, Vision **150**, Shroud **300**, BuildCost **150**.
//! - BoobyTrap residual name honesty (Upgrade_GLAInfantryRebelBoobyTrapAttack /
//!   SpecialAbilityBoobyTrap Reload **7500**ms → **225**f — host not full plant matrix).
//!
//! Fail-closed honesty:
//! - Not full ClipSize=3 in-clip DelayBetweenShots 100ms + ClipReload 700ms volley
//! - Not full CaptureBuilding BinaryDataStream attach / packing anim matrix
//! - Not full BoobyTrap SpecialObject plant / MaxSpecialObjects list UI
//! - Not full StealthUpdate forbidden-condition matrix (Camouflage closed elsewhere)
//! - Not network AP / fire / capture replication (network deferred)

use super::Weapon;
use crate::game_logic::host_red_guard::delay_frames_to_reload_secs;

/// Logic frames per second (host fixed step).
pub const REBEL_LOGIC_FPS: f32 = 30.0;

/// Retail primary weapon.
pub const REBEL_MACHINE_GUN: &str = "GLARebelMachineGun";
/// Retail Upgrade_GLAAPBullets.
pub const UPGRADE_GLA_AP_BULLETS: &str = "Upgrade_GLAAPBullets";
/// Retail Upgrade_InfantryCaptureBuilding residual.
pub const UPGRADE_INFANTRY_CAPTURE_BUILDING: &str = "Upgrade_InfantryCaptureBuilding";
/// Retail Upgrade_GLAInfantryRebelBoobyTrapAttack residual.
pub const UPGRADE_GLA_REBEL_BOOBY_TRAP: &str = "Upgrade_GLAInfantryRebelBoobyTrapAttack";
/// Retail SpecialAbilityRebelCaptureBuilding residual.
pub const SPECIAL_ABILITY_REBEL_CAPTURE: &str = "SpecialAbilityRebelCaptureBuilding";
/// Retail SpecialAbilityBoobyTrap residual.
pub const SPECIAL_ABILITY_BOOBY_TRAP: &str = "SpecialAbilityBoobyTrap";

/// Retail PrimaryDamage base.
pub const REBEL_DAMAGE: f32 = 5.0;
/// Retail PrimaryDamageRadius residual (intended-only).
pub const REBEL_DAMAGE_RADIUS: f32 = 0.0;
/// Retail AttackRange.
pub const REBEL_RANGE: f32 = 100.0;
/// Retail DelayBetweenShots residual (msec).
pub const REBEL_DELAY_MS: u32 = 100;
/// Retail DelayBetweenShots 100ms → 3 frames @ 30 FPS.
pub const REBEL_BASE_DELAY_FRAMES: u32 = 3;
/// Retail ClipSize residual honesty (fail-closed volley matrix).
pub const REBEL_CLIP_SIZE: u32 = 3;
/// Retail ClipReloadTime residual (msec).
pub const REBEL_CLIP_RELOAD_MS: u32 = 700;
/// Retail ClipReloadTime 700ms → 21 frames @ 30 FPS (honesty only).
pub const REBEL_CLIP_RELOAD_FRAMES: u32 = 21;
/// Retail DamageType residual.
pub const REBEL_DAMAGE_TYPE: &str = "SMALL_ARMS";
/// Retail FireFX residual.
pub const REBEL_FIRE_FX: &str = "WeaponFX_GenericMachineGunFire";
/// Residual fire audio.
pub const REBEL_FIRE_AUDIO: &str = "RebelWeapon";

/// AP Bullets WeaponBonus DAMAGE 125%.
pub const REBEL_AP_DAMAGE_MULT: f32 = 1.25;

// --- Capture residual ---

/// Retail Capture SpecialPower ReloadTime residual (msec).
pub const REBEL_CAPTURE_RELOAD_MS: u32 = 15_000;
/// ReloadTime 15000ms → 450 frames @ 30 FPS.
pub const REBEL_CAPTURE_RELOAD_FRAMES: u32 = 450;
/// Retail StartAbilityRange residual.
pub const REBEL_CAPTURE_START_RANGE: f32 = 5.0;
/// Retail UnpackTime residual (msec).
pub const REBEL_CAPTURE_UNPACK_MS: u32 = 3_000;
/// UnpackTime 3000ms → 90 frames.
pub const REBEL_CAPTURE_UNPACK_FRAMES: u32 = 90;
/// Retail PreparationTime residual (msec).
pub const REBEL_CAPTURE_PREP_MS: u32 = 20_000;
/// PreparationTime 20000ms → 600 frames.
pub const REBEL_CAPTURE_PREP_FRAMES: u32 = 600;
/// Retail PackTime residual (msec).
pub const REBEL_CAPTURE_PACK_MS: u32 = 2_000;
/// PackTime 2000ms → 60 frames.
pub const REBEL_CAPTURE_PACK_FRAMES: u32 = 60;
/// Retail AwardXPForTriggering residual.
pub const REBEL_CAPTURE_AWARD_XP: u32 = 12;
/// Retail InitiateSound residual for capture.
pub const REBEL_CAPTURE_INITIATE_AUDIO: &str = "RebelVoiceCapture";

// --- BoobyTrap residual name honesty (fail-closed plant matrix) ---

/// Retail BoobyTrap SpecialPower ReloadTime residual (msec).
pub const REBEL_BOOBY_TRAP_RELOAD_MS: u32 = 7_500;
/// ReloadTime 7500ms → 225 frames.
pub const REBEL_BOOBY_TRAP_RELOAD_FRAMES: u32 = 225;
/// Retail BoobyTrap StartAbilityRange residual.
pub const REBEL_BOOBY_TRAP_START_RANGE: f32 = 5.0;
/// Retail SpecialObject residual name.
pub const REBEL_BOOBY_TRAP_OBJECT: &str = "BoobyTrap";

// --- Body residual ---

/// Retail MaxHealth residual.
pub const REBEL_MAX_HEALTH: f32 = 120.0;
/// Retail VisionRange residual.
pub const REBEL_VISION_RANGE: f32 = 150.0;
/// Retail ShroudClearingRange residual.
pub const REBEL_SHROUD_CLEARING_RANGE: f32 = 300.0;
/// Retail BuildCost residual.
pub const REBEL_BUILD_COST: u32 = 150;

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn rebel_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * REBEL_LOGIC_FPS / 1000.0).round() as u32
}

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

/// Whether upgrade set includes Capture Building residual.
pub fn has_capture_building_upgrade(applied_upgrades: &std::collections::HashSet<String>) -> bool {
    applied_upgrades.iter().any(|u| {
        let n = u.to_ascii_lowercase();
        n.contains("capturebuilding")
            || n.contains("capture_building")
            || n.contains("infantrycapturebuilding")
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

/// Whether unit can issue capture residual (upgrade + alive).
pub fn can_activate_rebel_capture(
    is_rebel: bool,
    is_alive: bool,
    has_capture_upgrade: bool,
) -> bool {
    is_rebel && is_alive && has_capture_upgrade
}

/// Whether target is within StartAbilityRange residual for capture.
pub fn rebel_capture_in_start_range(distance: f32) -> bool {
    distance <= REBEL_CAPTURE_START_RANGE
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

// --- Wave 60 residual honesty packs ---

/// Wave 60 residual honesty: gun damage / range / ROF / clip residual.
pub fn honesty_rebel_gun_residual_ok() -> bool {
    (REBEL_DAMAGE - 5.0).abs() < 0.01
        && (REBEL_RANGE - 100.0).abs() < 0.01
        && (REBEL_DAMAGE_RADIUS - 0.0).abs() < 0.01
        && REBEL_DELAY_MS == 100
        && REBEL_BASE_DELAY_FRAMES == rebel_ms_to_frames(REBEL_DELAY_MS)
        && REBEL_CLIP_SIZE == 3
        && REBEL_CLIP_RELOAD_MS == 700
        && REBEL_CLIP_RELOAD_FRAMES == rebel_ms_to_frames(REBEL_CLIP_RELOAD_MS)
        && REBEL_MACHINE_GUN == "GLARebelMachineGun"
        && REBEL_DAMAGE_TYPE == "SMALL_ARMS"
        && REBEL_FIRE_AUDIO == "RebelWeapon"
        && REBEL_FIRE_FX == "WeaponFX_GenericMachineGunFire"
        && (rebel_damage_with_ap(false) - 5.0).abs() < 0.01
        && (rebel_damage_with_ap(true) - 6.25).abs() < 0.01
        && (REBEL_AP_DAMAGE_MULT - 1.25).abs() < 0.001
        && UPGRADE_GLA_AP_BULLETS == "Upgrade_GLAAPBullets"
        && {
            let w = rebel_weapon(false);
            (w.damage - 5.0).abs() < 0.01
                && (w.range - 100.0).abs() < 0.01
                && w.ammo == Some(3)
                && !w.can_target_air
                && w.can_target_ground
        }
}

/// Wave 60 residual honesty: capture building residual peel.
pub fn honesty_rebel_capture_residual_ok() -> bool {
    SPECIAL_ABILITY_REBEL_CAPTURE == "SpecialAbilityRebelCaptureBuilding"
        && UPGRADE_INFANTRY_CAPTURE_BUILDING == "Upgrade_InfantryCaptureBuilding"
        && REBEL_CAPTURE_RELOAD_MS == 15_000
        && REBEL_CAPTURE_RELOAD_FRAMES == rebel_ms_to_frames(REBEL_CAPTURE_RELOAD_MS)
        && (REBEL_CAPTURE_START_RANGE - 5.0).abs() < 0.01
        && REBEL_CAPTURE_UNPACK_MS == 3_000
        && REBEL_CAPTURE_UNPACK_FRAMES == rebel_ms_to_frames(REBEL_CAPTURE_UNPACK_MS)
        && REBEL_CAPTURE_PREP_MS == 20_000
        && REBEL_CAPTURE_PREP_FRAMES == rebel_ms_to_frames(REBEL_CAPTURE_PREP_MS)
        && REBEL_CAPTURE_PACK_MS == 2_000
        && REBEL_CAPTURE_PACK_FRAMES == rebel_ms_to_frames(REBEL_CAPTURE_PACK_MS)
        && REBEL_CAPTURE_AWARD_XP == 12
        && REBEL_CAPTURE_INITIATE_AUDIO == "RebelVoiceCapture"
        && rebel_capture_in_start_range(5.0)
        && !rebel_capture_in_start_range(5.1)
        && can_activate_rebel_capture(true, true, true)
        && !can_activate_rebel_capture(true, true, false)
        && !can_activate_rebel_capture(false, true, true)
}

/// Wave 60 residual honesty: body / vision + booby-trap name residual.
pub fn honesty_rebel_body_booby_residual_ok() -> bool {
    (REBEL_MAX_HEALTH - 120.0).abs() < 0.01
        && (REBEL_VISION_RANGE - 150.0).abs() < 0.01
        && (REBEL_SHROUD_CLEARING_RANGE - 300.0).abs() < 0.01
        && REBEL_BUILD_COST == 150
        && SPECIAL_ABILITY_BOOBY_TRAP == "SpecialAbilityBoobyTrap"
        && UPGRADE_GLA_REBEL_BOOBY_TRAP == "Upgrade_GLAInfantryRebelBoobyTrapAttack"
        && REBEL_BOOBY_TRAP_RELOAD_MS == 7_500
        && REBEL_BOOBY_TRAP_RELOAD_FRAMES == rebel_ms_to_frames(REBEL_BOOBY_TRAP_RELOAD_MS)
        && (REBEL_BOOBY_TRAP_START_RANGE - 5.0).abs() < 0.01
        && REBEL_BOOBY_TRAP_OBJECT == "BoobyTrap"
}

/// Combined Wave 60 GLA Rebel residual honesty pack.
pub fn honesty_gla_rebel_residual_pack_ok() -> bool {
    honesty_rebel_gun_residual_ok()
        && honesty_rebel_capture_residual_ok()
        && honesty_rebel_body_booby_residual_ok()
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
        assert!(!is_gla_rebel_template(
            "Upgrade_GLAInfantryRebelBoobyTrapAttack"
        ));
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

    #[test]
    fn gla_rebel_residual_pack_honesty() {
        assert!(honesty_rebel_gun_residual_ok());
        assert!(honesty_rebel_capture_residual_ok());
        assert!(honesty_rebel_body_booby_residual_ok());
        assert!(honesty_gla_rebel_residual_pack_ok());
        assert_eq!(rebel_ms_to_frames(100), 3);
        assert_eq!(rebel_ms_to_frames(700), 21);
        assert_eq!(rebel_ms_to_frames(15_000), 450);
        assert_eq!(rebel_ms_to_frames(3_000), 90);
        assert_eq!(rebel_ms_to_frames(20_000), 600);
        assert_eq!(rebel_ms_to_frames(2_000), 60);
        assert_eq!(rebel_ms_to_frames(7_500), 225);
        assert_eq!(rebel_ms_to_frames(0), 0);
    }

    #[test]
    fn capture_upgrade_and_range() {
        let mut tags = HashSet::new();
        assert!(!has_capture_building_upgrade(&tags));
        tags.insert(UPGRADE_INFANTRY_CAPTURE_BUILDING.to_string());
        assert!(has_capture_building_upgrade(&tags));
        assert!(rebel_capture_in_start_range(0.0));
        assert!(rebel_capture_in_start_range(5.0));
        assert!(!rebel_capture_in_start_range(5.1));
    }
}
