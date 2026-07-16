//! Host GLA Jarmen Kell residual combat polish (sniper rifle + AP Bullets).
//!
//! Residual slice (playability):
//! - `GLAInfantryJarmenKell` / Chem_/Demo_/Slth_/GC_* / TestJarmenKell spawn with
//!   PRIMARY `GLAJarmenKellRifle` (dmg **180** / range **225** / Delay **1000**ms
//!   → 30 frames). DamageType SNIPER residual (intended-only; radius **0**).
//! - AP Bullets PLAYER_UPGRADE residual (`Upgrade_GLAAPBullets`): DAMAGE **125%**
//!   → PrimaryDamage **225**.
//! - Vehicle pilot-snipe special residual already closed via host_hero_abilities
//!   (`SnipeVehicle` / DAMAGE_KILLPILOT → unmanned) — residual weapon peel here.
//! - Combat Cycle rider sniper residual remains host_combat_cycle (not re-opened).
//!
//! Wave 57 residual pack (retail INI honesty):
//! - Sniper residual: PrimaryDamage **180**, AttackRange **225**, Delay **1000**ms → **30**f,
//!   DamageType **SNIPER**, PrimaryDamageRadius **0**, ClipSize **0**, FireSound
//!   **JarmenKellWeapon**, FireFX **WeaponFX_GenericMachineGunFire**
//! - AP Bullets WeaponBonus PLAYER_UPGRADE DAMAGE **125%** residual
//! - Vehicle pilot-snipe residual: `GLAJarmenKellVehiclePilotSniperRifle`
//!   PrimaryDamage **1**, DamageType **KILL_PILOT**, AttackRange **225**,
//!   ClipSize **1**, ClipReloadTime **30000**ms → **900**f, AutoReloadsClip **Yes**,
//!   DelayBetweenShots **0**, FireSound **JarmenKellWeaponSnipe**
//! - StealthUpdate residual: StealthDelay **2000**ms → **60**f, InnateStealth **Yes**,
//!   Forbidden **ATTACKING**, OrderIdleEnemiesToAttackMeUponReveal **Yes**
//! - Body residual: MaxHealth **200**, VisionRange **200**, ShroudClearingRange **400**,
//!   BuildCost **1500**
//! - Biker sniper residual honesty (fail-closed infantry path stays 1000ms):
//!   `GLABikerKellSniperRifle` Delay **750**ms → **23**f (not auto-applied on foot)
//!
//! Fail-closed honesty:
//! - Not full SECONDARY AutoChooseSources=NONE pilot-sniper WeaponSet chooser matrix
//! - Not full StealthUpdate / Camouflage / Science prereq residual matrix
//! - Not full biker sniper Delay 750ms when dismounted (infantry stays 1000ms)
//! - Not network sniper / AP Bullets replication (network deferred)

use super::Weapon;
use crate::game_logic::host_red_guard::delay_frames_to_reload_secs;
use std::collections::HashSet;

/// Logic frames per second (host fixed step).
pub const JARMEN_LOGIC_FPS: f32 = 30.0;

/// Retail primary sniper weapon.
pub const JARMEN_KELL_RIFLE: &str = "GLAJarmenKellRifle";
/// Retail secondary pilot-snipe weapon (special residual; not host primary combat).
pub const JARMEN_KELL_PILOT_SNIPER: &str = "GLAJarmenKellVehiclePilotSniperRifle";
/// Retail biker sniper weapon residual (combat-cycle rider path; fail-closed here).
pub const JARMEN_KELL_BIKER_RIFLE: &str = "GLABikerKellSniperRifle";
/// Retail Upgrade_GLAAPBullets (WeaponBonus PLAYER_UPGRADE DAMAGE 125%).
pub const UPGRADE_GLA_AP_BULLETS: &str = "Upgrade_GLAAPBullets";

/// Retail PrimaryDamage base (sniper).
pub const JARMEN_KELL_DAMAGE: f32 = 180.0;
/// Retail AttackRange.
pub const JARMEN_KELL_RANGE: f32 = 225.0;
/// Retail DelayBetweenShots 1000ms.
pub const JARMEN_KELL_DELAY_MS: u32 = 1_000;
/// Retail DelayBetweenShots 1000ms → 30 frames @ 30 FPS.
pub const JARMEN_KELL_DELAY_FRAMES: u32 = 30;
/// AP Bullets WeaponBonus DAMAGE 125%.
pub const JARMEN_KELL_AP_DAMAGE_MULT: f32 = 1.25;
/// Retail DamageType residual.
pub const JARMEN_KELL_DAMAGE_TYPE: &str = "SNIPER";
/// Retail PrimaryDamageRadius residual (intended-only).
pub const JARMEN_KELL_DAMAGE_RADIUS: f32 = 0.0;
/// Retail ClipSize residual (0 == infinite).
pub const JARMEN_KELL_CLIP_SIZE: u32 = 0;
/// Retail FireFX residual.
pub const JARMEN_KELL_FIRE_FX: &str = "WeaponFX_GenericMachineGunFire";

/// Residual sniper fire audio.
pub const JARMEN_KELL_FIRE_AUDIO: &str = "JarmenKellWeapon";

// --- Vehicle pilot-snipe residual ---

/// Retail pilot-snipe PrimaryDamage residual.
pub const JARMEN_PILOT_SNIPE_DAMAGE: f32 = 1.0;
/// Retail pilot-snipe AttackRange residual.
pub const JARMEN_PILOT_SNIPE_RANGE: f32 = 225.0;
/// Retail pilot-snipe DamageType residual.
pub const JARMEN_PILOT_SNIPE_DAMAGE_TYPE: &str = "KILL_PILOT";
/// Retail pilot-snipe DelayBetweenShots residual.
pub const JARMEN_PILOT_SNIPE_DELAY_MS: u32 = 0;
/// Retail pilot-snipe ClipSize residual.
pub const JARMEN_PILOT_SNIPE_CLIP_SIZE: u32 = 1;
/// Retail pilot-snipe ClipReloadTime residual (msec).
pub const JARMEN_PILOT_SNIPE_CLIP_RELOAD_MS: u32 = 30_000;
/// ClipReloadTime 30000ms → 900 frames @ 30 FPS.
pub const JARMEN_PILOT_SNIPE_CLIP_RELOAD_FRAMES: u32 = 900;
/// Retail AutoReloadsClip residual.
pub const JARMEN_PILOT_SNIPE_AUTO_RELOADS_CLIP: bool = true;
/// Retail pilot-snipe FireSound residual.
pub const JARMEN_PILOT_SNIPE_FIRE_AUDIO: &str = "JarmenKellWeaponSnipe";
/// Retail VoiceSnipePilot residual.
pub const JARMEN_VOICE_SNIPE_PILOT: &str = "JarmenKellVoiceSnipe";

// --- Biker sniper residual honesty (fail-closed; not applied on foot) ---

/// Retail GLABikerKellSniperRifle DelayBetweenShots residual (msec).
pub const JARMEN_BIKER_DELAY_MS: u32 = 750;
/// Biker Delay 750ms → 23 frames @ 30 FPS.
pub const JARMEN_BIKER_DELAY_FRAMES: u32 = 23;

// --- Body / vision residual ---

/// Retail MaxHealth residual.
pub const JARMEN_MAX_HEALTH: f32 = 200.0;
/// Retail VisionRange residual.
pub const JARMEN_VISION_RANGE: f32 = 200.0;
/// Retail ShroudClearingRange residual.
pub const JARMEN_SHROUD_CLEARING_RANGE: f32 = 400.0;
/// Retail BuildCost residual.
pub const JARMEN_BUILD_COST: u32 = 1_500;

// --- StealthUpdate residual ---

/// Retail StealthUpdate StealthDelay residual (msec).
pub const JARMEN_STEALTH_DELAY_MS: u32 = 2_000;
/// StealthDelay 2000ms → 60 frames @ 30 FPS.
pub const JARMEN_STEALTH_DELAY_FRAMES: u32 = 60;
/// Retail InnateStealth residual.
pub const JARMEN_INNATE_STEALTH: bool = true;
/// Retail StealthForbiddenConditions = ATTACKING residual.
pub const JARMEN_STEALTH_BREAKS_ON_ATTACK: bool = true;
/// Retail OrderIdleEnemiesToAttackMeUponReveal residual.
pub const JARMEN_ORDER_IDLE_ENEMIES_ON_REVEAL: bool = true;
/// Retail EnemyDetectionEvaEvent residual.
pub const JARMEN_ENEMY_DETECTION_EVA: &str = "EnemyJarmenKellDetected";
/// Retail OwnDetectionEvaEvent residual.
pub const JARMEN_OWN_DETECTION_EVA: &str = "OwnJarmenKellDetected";
/// Residual stealth on/off audio.
pub const JARMEN_STEALTH_ON_AUDIO: &str = "StealthOn";
pub const JARMEN_STEALTH_OFF_AUDIO: &str = "StealthOff";

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn jarmen_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / JARMEN_LOGIC_FPS)).round() as u32
}

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
        splash_radius: 0.0,
    }
}

/// (damage, range, delay_frames) for pilot-snipe residual weapon honesty.
pub fn jarmen_pilot_snipe_weapon_stats() -> (f32, f32, u32) {
    (
        JARMEN_PILOT_SNIPE_DAMAGE,
        JARMEN_PILOT_SNIPE_RANGE,
        JARMEN_PILOT_SNIPE_CLIP_RELOAD_FRAMES,
    )
}

/// Build residual pilot-snipe Weapon (special residual; not primary combat path).
pub fn jarmen_pilot_snipe_weapon() -> Weapon {
    let (damage, range, delay) = jarmen_pilot_snipe_weapon_stats();
    Weapon {
        damage,
        range,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(delay),
        last_fire_time: 0.0,
        ammo: Some(JARMEN_PILOT_SNIPE_CLIP_SIZE),
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 999_999.0,
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
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

/// Legal residual pilot-snipe target (enemy manned ground vehicle).
pub fn is_legal_pilot_snipe_target(
    is_alive: bool,
    is_vehicle: bool,
    is_airborne: bool,
    is_enemy: bool,
    unmanned: bool,
) -> bool {
    is_alive && is_vehicle && !is_airborne && is_enemy && !unmanned
}

/// Maintain Jarmen Kell stealth residual (ATTACKING breaks cloak).
///
/// Returns `Some(desired_stealthed)` for honesty bookkeeping.
pub fn jarmen_stealth_desired(
    is_kell: bool,
    innate_stealth: bool,
    is_alive: bool,
    is_attacking: bool,
) -> Option<bool> {
    if !is_kell || !innate_stealth || !is_alive {
        return None;
    }
    if JARMEN_STEALTH_BREAKS_ON_ATTACK && is_attacking {
        Some(false)
    } else {
        Some(true)
    }
}

// --- Wave 57 residual honesty packs ---

/// Wave 57 residual honesty: sniper range / damage / reload residual.
pub fn honesty_jarmen_sniper_residual_ok() -> bool {
    (JARMEN_KELL_DAMAGE - 180.0).abs() < 0.01
        && (JARMEN_KELL_RANGE - 225.0).abs() < 0.01
        && JARMEN_KELL_DELAY_MS == 1_000
        && JARMEN_KELL_DELAY_FRAMES == jarmen_ms_to_frames(JARMEN_KELL_DELAY_MS)
        && JARMEN_KELL_RIFLE == "GLAJarmenKellRifle"
        && JARMEN_KELL_DAMAGE_TYPE == "SNIPER"
        && (JARMEN_KELL_DAMAGE_RADIUS - 0.0).abs() < 0.01
        && JARMEN_KELL_CLIP_SIZE == 0
        && JARMEN_KELL_FIRE_AUDIO == "JarmenKellWeapon"
        && JARMEN_KELL_FIRE_FX == "WeaponFX_GenericMachineGunFire"
        && (jarmen_kell_damage_with_ap(false) - 180.0).abs() < 0.01
        && (jarmen_kell_damage_with_ap(true) - 225.0).abs() < 0.01
        && (JARMEN_KELL_AP_DAMAGE_MULT - 1.25).abs() < 0.001
        && UPGRADE_GLA_AP_BULLETS == "Upgrade_GLAAPBullets"
}

/// Wave 57 residual honesty: vehicle pilot-snipe residual.
pub fn honesty_jarmen_pilot_snipe_residual_ok() -> bool {
    JARMEN_KELL_PILOT_SNIPER == "GLAJarmenKellVehiclePilotSniperRifle"
        && (JARMEN_PILOT_SNIPE_DAMAGE - 1.0).abs() < 0.01
        && (JARMEN_PILOT_SNIPE_RANGE - 225.0).abs() < 0.01
        && JARMEN_PILOT_SNIPE_DAMAGE_TYPE == "KILL_PILOT"
        && JARMEN_PILOT_SNIPE_DELAY_MS == 0
        && JARMEN_PILOT_SNIPE_CLIP_SIZE == 1
        && JARMEN_PILOT_SNIPE_CLIP_RELOAD_MS == 30_000
        && JARMEN_PILOT_SNIPE_CLIP_RELOAD_FRAMES
            == jarmen_ms_to_frames(JARMEN_PILOT_SNIPE_CLIP_RELOAD_MS)
        && JARMEN_PILOT_SNIPE_AUTO_RELOADS_CLIP
        && JARMEN_PILOT_SNIPE_FIRE_AUDIO == "JarmenKellWeaponSnipe"
        && JARMEN_VOICE_SNIPE_PILOT == "JarmenKellVoiceSnipe"
        && is_legal_pilot_snipe_target(true, true, false, true, false)
        && !is_legal_pilot_snipe_target(true, true, true, true, false)
        && !is_legal_pilot_snipe_target(true, true, false, true, true)
        && !is_legal_pilot_snipe_target(true, false, false, true, false)
}

/// Wave 57 residual honesty: StealthUpdate residual.
pub fn honesty_jarmen_stealth_residual_ok() -> bool {
    JARMEN_STEALTH_DELAY_MS == 2_000
        && JARMEN_STEALTH_DELAY_FRAMES == jarmen_ms_to_frames(JARMEN_STEALTH_DELAY_MS)
        && JARMEN_INNATE_STEALTH
        && JARMEN_STEALTH_BREAKS_ON_ATTACK
        && JARMEN_ORDER_IDLE_ENEMIES_ON_REVEAL
        && JARMEN_ENEMY_DETECTION_EVA == "EnemyJarmenKellDetected"
        && JARMEN_OWN_DETECTION_EVA == "OwnJarmenKellDetected"
        && JARMEN_STEALTH_ON_AUDIO == "StealthOn"
        && JARMEN_STEALTH_OFF_AUDIO == "StealthOff"
        && jarmen_stealth_desired(true, true, true, true) == Some(false)
        && jarmen_stealth_desired(true, true, true, false) == Some(true)
}

/// Wave 57 residual honesty: body / vision + biker delay residual.
pub fn honesty_jarmen_body_biker_residual_ok() -> bool {
    (JARMEN_MAX_HEALTH - 200.0).abs() < 0.01
        && (JARMEN_VISION_RANGE - 200.0).abs() < 0.01
        && (JARMEN_SHROUD_CLEARING_RANGE - 400.0).abs() < 0.01
        && JARMEN_BUILD_COST == 1_500
        && JARMEN_KELL_BIKER_RIFLE == "GLABikerKellSniperRifle"
        && JARMEN_BIKER_DELAY_MS == 750
        && JARMEN_BIKER_DELAY_FRAMES == jarmen_ms_to_frames(JARMEN_BIKER_DELAY_MS)
        // Fail-closed: infantry sniper stays 1000ms, not biker 750ms.
        && JARMEN_KELL_DELAY_FRAMES != JARMEN_BIKER_DELAY_FRAMES
}

/// Combined Wave 57 Jarmen Kell residual honesty pack.
pub fn honesty_jarmen_kell_residual_pack_ok() -> bool {
    honesty_jarmen_sniper_residual_ok()
        && honesty_jarmen_pilot_snipe_residual_ok()
        && honesty_jarmen_stealth_residual_ok()
        && honesty_jarmen_body_biker_residual_ok()
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
        assert!(!is_jarmen_kell_template(
            "GLAJarmenKellVehiclePilotSniperRifle"
        ));
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

    #[test]
    fn pilot_snipe_weapon_and_gate() {
        let w = jarmen_pilot_snipe_weapon();
        assert!((w.damage - 1.0).abs() < 0.01);
        assert!((w.range - 225.0).abs() < 0.01);
        assert_eq!(w.ammo, Some(1));
        assert!((w.reload_time - 30.0).abs() < 0.05);
        assert!(is_legal_pilot_snipe_target(true, true, false, true, false));
        assert!(!is_legal_pilot_snipe_target(true, true, false, true, true));
        assert!(!is_legal_pilot_snipe_target(true, true, true, true, false));
    }

    #[test]
    fn jarmen_kell_residual_pack_honesty() {
        assert!(honesty_jarmen_kell_residual_pack_ok());
        assert_eq!(jarmen_ms_to_frames(1_000), 30);
        assert_eq!(jarmen_ms_to_frames(2_000), 60);
        assert_eq!(jarmen_ms_to_frames(30_000), 900);
        assert_eq!(jarmen_ms_to_frames(750), 23);
        assert_eq!(jarmen_ms_to_frames(0), 0);
    }

    #[test]
    fn jarmen_stealth_desired_residual() {
        assert_eq!(
            jarmen_stealth_desired(true, true, true, true),
            Some(false),
            "attacking uncloaks"
        );
        assert_eq!(
            jarmen_stealth_desired(true, true, true, false),
            Some(true),
            "idle re-cloaks after delay residual"
        );
        assert_eq!(jarmen_stealth_desired(false, true, true, false), None);
    }
}
