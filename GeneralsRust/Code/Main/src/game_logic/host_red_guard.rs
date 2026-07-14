//! Host China Red Guard residual (machine gun + horde/nationalism ROF + bayonet).
//!
//! Residual slice (playability):
//! - `ChinaInfantryRedguard` / variants spawn with PRIMARY `RedguardMachineGun`
//!   (dmg **15** / range **100** / Delay **1000**ms → 30 frames).
//! - Horde residual (`HordeUpdate` KindOf INFANTRY, AlliesOnly, ExactMatch=No,
//!   Radius **30**, Count **5**): WeaponBonus HORDE RATE_OF_FIRE **150%**
//!   → delay floor(30/1.5)=**20** frames.
//! - Nationalism residual (`Upgrade_Nationalism` while in horde):
//!   additional RATE_OF_FIRE **125%** (stacks) → floor(30/1.875)=**16** frames.
//! - Bayonet residual (`RedguardBayonet` stats): when attacking infantry within
//!   AttackRange **2**, one-shot MELEE residual (PrimaryDamage **10000**).
//!   Retail ZH WeaponSet is PRIMARY-only; bayonet is residual from weapon def +
//!   PREATTACK_C/FIRING_C animations (CINE units bind TERTIARY).
//!
//! Wave 67 residual pack (retail ChinaInfantry.ini / Weapon.ini / Locomotor.ini):
//! - Weapon residual: DamageType **SMALL_ARMS**, DeathType **NORMAL**,
//!   PrimaryDamageRadius **0**, FireFX **WeaponFX_GenericMachineGunFire**,
//!   WeaponSpeed instant, ClipSize **0**, Delay **1000**ms → **30**f.
//! - Bayonet residual: DamageType **MELEE**, PreAttackDelay **1400**ms → **42**f,
//!   Delay **1900**ms → **57**f, range **2**.
//! - Body residual: MaxHealth **120**, Vision **100**, Shroud **200**,
//!   BuildCost **300**, BuildTime **10**s → **300**f, TransportSlotCount **1**,
//!   Locomotor Speed **25**/Damaged **15**, Geometry CYLINDER r**7**/h**12**.
//! - Capture residual: SpecialAbilityRedGuardCaptureBuilding StartAbilityRange **5**,
//!   Unpack **3000**ms / Prep **20000**ms / Pack **2000**ms honesty.
//!
//! Fail-closed honesty:
//! - Not full HordeUpdate RubOffRadius honorary-member / terrain-decal flag matrix
//! - Not full Fanaticism infantry-general nationalism branch
//! - Not full WeaponSet tertiary auto-choose / pre-attack anim lock matrix
//! - SCIENCE_RedGuardTraining VETERAN spawn residual closed in host_unit_training
//! - Not network horde / nationalism replication (network deferred)

use super::Weapon;
use crate::game_logic::host_battlemaster::{has_nationalism_upgrade, UPGRADE_NATIONALISM};

// Re-export nationalism helpers for integration call sites.
pub use crate::game_logic::host_battlemaster::has_nationalism_upgrade as red_guard_has_nationalism;
pub use crate::game_logic::host_battlemaster::UPGRADE_NATIONALISM as RED_GUARD_UPGRADE_NATIONALISM;

/// Logic frames per second (host fixed step).
pub const RED_GUARD_LOGIC_FPS: f32 = 30.0;

/// Retail primary weapon.
pub const REDGUARD_MACHINE_GUN: &str = "RedguardMachineGun";
/// Residual bayonet weapon name.
pub const REDGUARD_BAYONET: &str = "RedguardBayonet";
/// Retail capture special ability residual.
pub const SPECIAL_ABILITY_RED_GUARD_CAPTURE: &str = "SpecialAbilityRedGuardCaptureBuilding";
/// Retail Upgrade_InfantryCaptureBuilding residual.
pub const UPGRADE_INFANTRY_CAPTURE_BUILDING: &str = "Upgrade_InfantryCaptureBuilding";

/// Retail PrimaryDamage base.
pub const REDGUARD_DAMAGE: f32 = 15.0;
/// Retail PrimaryDamageRadius residual (0 = intended only).
pub const REDGUARD_PRIMARY_RADIUS: f32 = 0.0;
/// Retail AttackRange.
pub const REDGUARD_RANGE: f32 = 100.0;
/// Retail DelayBetweenShots residual (msec).
pub const REDGUARD_BASE_DELAY_MS: u32 = 1_000;
/// Retail DelayBetweenShots 1000ms → 30 frames @ 30 FPS.
pub const REDGUARD_BASE_DELAY_FRAMES: u32 = 30;
/// Retail DamageType residual.
pub const REDGUARD_DAMAGE_TYPE: &str = "SMALL_ARMS";
/// Retail DeathType residual.
pub const REDGUARD_DEATH_TYPE: &str = "NORMAL";
/// Retail FireFX residual.
pub const REDGUARD_FIRE_FX: &str = "WeaponFX_GenericMachineGunFire";
/// Retail ClipSize residual (0 == infinite).
pub const REDGUARD_CLIP_SIZE: u32 = 0;

/// Bayonet PrimaryDamage residual (one-shot kill).
pub const BAYONET_DAMAGE: f32 = 10_000.0;
/// Bayonet AttackRange residual.
pub const BAYONET_RANGE: f32 = 2.0;
/// Bayonet DelayBetweenShots residual (msec).
pub const BAYONET_DELAY_MS: u32 = 1_900;
/// Bayonet DelayBetweenShots 1900ms → 57 frames @ 30 FPS.
pub const BAYONET_DELAY_FRAMES: u32 = 57;
/// Bayonet PreAttackDelay residual (msec).
pub const BAYONET_PRE_ATTACK_MS: u32 = 1_400;
/// Bayonet PreAttackDelay 1400ms residual (fail-closed vs full pre-attack lock).
pub const BAYONET_PRE_ATTACK_FRAMES: u32 = 42;
/// Bayonet DamageType residual.
pub const BAYONET_DAMAGE_TYPE: &str = "MELEE";

/// HORDE WeaponBonus RATE_OF_FIRE 150%.
pub const INFANTRY_HORDE_ROF_MULT: f32 = 1.5;
/// NATIONALISM WeaponBonus RATE_OF_FIRE 125% (stacks with horde when both active).
pub const INFANTRY_NATIONALISM_ROF_MULT: f32 = 1.25;

/// Retail HordeUpdate Radius for China infantry (Red Guard / Tank Hunter).
pub const INFANTRY_HORDE_RADIUS: f32 = 30.0;
/// Retail HordeUpdate Count (includes self via C++ minCount-1 others).
pub const INFANTRY_HORDE_COUNT: u32 = 5;
/// Retail HordeUpdate UpdateRate residual (msec).
pub const INFANTRY_HORDE_UPDATE_MS: u32 = 1_000;
/// Retail HordeUpdate UpdateRate 1000ms → 30 frames @ 30 FPS.
pub const INFANTRY_HORDE_UPDATE_FRAMES: u32 = 30;
/// Retail HordeUpdate ExactMatch residual (No for infantry).
pub const INFANTRY_HORDE_EXACT_MATCH: bool = false;
/// Retail HordeUpdate KindOf residual.
pub const INFANTRY_HORDE_KIND_OF: &str = "INFANTRY";

/// Residual fire audio.
pub const REDGUARD_FIRE_AUDIO: &str = "RedGuardWeapon";
/// Residual bayonet audio.
pub const BAYONET_FIRE_AUDIO: &str = "HeroUSAKnifeAttack";

// --- Body residual (ChinaInfantryRedguard) ---

/// Retail MaxHealth residual.
pub const REDGUARD_MAX_HEALTH: f32 = 120.0;
/// Retail VisionRange residual.
pub const REDGUARD_VISION_RANGE: f32 = 100.0;
/// Retail ShroudClearingRange residual.
pub const REDGUARD_SHROUD_CLEARING_RANGE: f32 = 200.0;
/// Retail BuildCost residual.
pub const REDGUARD_BUILD_COST: u32 = 300;
/// Retail BuildTime residual (seconds).
pub const REDGUARD_BUILD_TIME_SEC: f32 = 10.0;
/// BuildTime 10s → 300 frames @ 30 FPS.
pub const REDGUARD_BUILD_TIME_FRAMES: u32 = 300;
/// Retail TransportSlotCount residual.
pub const REDGUARD_TRANSPORT_SLOT_COUNT: u32 = 1;
/// Retail RedguardLocomotor Speed residual.
pub const REDGUARD_LOCOMOTOR_SPEED: f32 = 25.0;
/// Retail RedguardLocomotor SpeedDamaged residual.
pub const REDGUARD_LOCOMOTOR_SPEED_DAMAGED: f32 = 15.0;
/// Retail Geometry CYLINDER MajorRadius residual.
pub const REDGUARD_GEOMETRY_RADIUS: f32 = 7.0;
/// Retail GeometryHeight residual.
pub const REDGUARD_GEOMETRY_HEIGHT: f32 = 12.0;
/// Retail ExperienceValue residual.
pub const REDGUARD_EXPERIENCE_VALUE: [u32; 4] = [5, 5, 10, 20];
/// Retail ExperienceRequired residual.
pub const REDGUARD_EXPERIENCE_REQUIRED: [u32; 4] = [0, 20, 40, 80];

/// Capture residual: StartAbilityRange.
pub const REDGUARD_CAPTURE_START_RANGE: f32 = 5.0;
/// Capture residual: UnpackTime (msec).
pub const REDGUARD_CAPTURE_UNPACK_MS: u32 = 3_000;
/// Capture residual: PreparationTime (msec).
pub const REDGUARD_CAPTURE_PREP_MS: u32 = 20_000;
/// Capture residual: PackTime (msec).
pub const REDGUARD_CAPTURE_PACK_MS: u32 = 2_000;
/// Capture residual frames.
pub const REDGUARD_CAPTURE_UNPACK_FRAMES: u32 = 90;
pub const REDGUARD_CAPTURE_PREP_FRAMES: u32 = 600;
pub const REDGUARD_CAPTURE_PACK_FRAMES: u32 = 60;

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn red_guard_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * RED_GUARD_LOGIC_FPS / 1000.0).round() as u32
}

/// Whether template is a residual Red Guard infantry.
///
/// Fail-closed: name residual. Excludes weapons/science/debris tokens.
pub fn is_red_guard_template(template_name: &str) -> bool {
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
        || n.contains("training")
        || n.contains("crate")
        || n.contains("locomotor")
        || n.contains("voice")
        || n.contains("bayonet")
        || n.contains("machinegun")
        || n.contains("machine_gun")
    {
        return false;
    }
    n.contains("redguard") || n.contains("red_guard") || n == "china_soldier" || n == "testredguard"
}

/// Whether residual unit participates in China infantry HordeUpdate residual
/// (Red Guard + Tank Hunter share KindOf INFANTRY horde params).
///
/// Name residual only (avoids circular host_tank_hunter import).
pub fn is_china_infantry_horde_unit(template_name: &str) -> bool {
    if is_red_guard_template(template_name) {
        return true;
    }
    let n = template_name.to_ascii_lowercase();
    if n.contains("weapon")
        || n.contains("missile")
        || n.contains("projectile")
        || n.contains("locomotor")
        || n.contains("sticky")
        || n.contains("detonation")
    {
        return false;
    }
    if n.contains("tankhunter") || n.contains("tank_hunter") || n == "testtankhunter" {
        return true;
    }
    // MiniGunner residual shares China infantry HordeUpdate params.
    n.contains("minigunner")
        || n.contains("mini_gunner")
        || n == "testminigunner"
        || n == "china_minigunner"
}

/// Combined ROF multiplier residual (HORDE * NATIONALISM when both active).
///
/// Nationalism only applies while in horde (C++ AIUpdate evaluateMoraleBonus).
pub fn red_guard_rof_multiplier(in_horde: bool, has_nationalism: bool) -> f32 {
    let mut rof = 1.0_f32;
    if in_horde {
        rof *= INFANTRY_HORDE_ROF_MULT;
        if has_nationalism {
            rof *= INFANTRY_NATIONALISM_ROF_MULT;
        }
    }
    rof
}

/// Delay frames residual: floor(base / ROF), min 1.
pub fn red_guard_delay_frames(in_horde: bool, has_nationalism: bool) -> u32 {
    let base = REDGUARD_BASE_DELAY_FRAMES as f32;
    let rof = red_guard_rof_multiplier(in_horde, has_nationalism);
    (base / rof).floor().max(1.0) as u32
}

/// Reload time seconds residual for delay frames @ 30 FPS.
pub fn delay_frames_to_reload_secs(delay_frames: u32) -> f32 {
    (delay_frames.max(1) as f32) / 30.0
}

/// (damage, range, delay_frames) for gun residual with ROF bonuses.
pub fn red_guard_weapon_stats(in_horde: bool, has_nationalism: bool) -> (f32, f32, u32) {
    (
        REDGUARD_DAMAGE,
        REDGUARD_RANGE,
        red_guard_delay_frames(in_horde, has_nationalism),
    )
}

/// Build residual PRIMARY machine-gun Weapon with horde/nationalism ROF residual.
pub fn red_guard_weapon(in_horde: bool, has_nationalism: bool) -> Weapon {
    let (damage, range, delay) = red_guard_weapon_stats(in_horde, has_nationalism);
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

/// Residual bayonet Weapon (close-range one-shot).
pub fn red_guard_bayonet_weapon() -> Weapon {
    Weapon {
        damage: BAYONET_DAMAGE,
        range: BAYONET_RANGE,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(BAYONET_DELAY_FRAMES),
        last_fire_time: 0.0,
        ammo: None,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 999_999.0,
        pre_attack_delay: delay_frames_to_reload_secs(BAYONET_PRE_ATTACK_FRAMES),
    }
}

/// Whether bayonet residual should apply for this shot.
///
/// Residual: target is living infantry, horizontal distance ≤ BAYONET_RANGE.
pub fn should_apply_bayonet_residual(
    is_red_guard: bool,
    target_is_infantry: bool,
    target_alive: bool,
    distance: f32,
) -> bool {
    is_red_guard
        && target_is_infantry
        && target_alive
        && distance <= BAYONET_RANGE
        && distance >= 0.0
}

/// Horde residual: Count includes self (C++: others >= Count-1).
pub fn is_in_infantry_horde(nearby_infantry_allies: u32) -> bool {
    nearby_infantry_allies + 1 >= INFANTRY_HORDE_COUNT
}

/// 2D distance residual (XZ plane).
pub fn distance_2d(ax: f32, az: f32, bx: f32, bz: f32) -> f32 {
    let dx = ax - bx;
    let dz = az - bz;
    (dx * dx + dz * dz).sqrt()
}

/// Whether other unit counts toward self's infantry horde residual
/// (KindOf INFANTRY + AlliesOnly + Radius; ExactMatch=No).
pub fn counts_toward_infantry_horde(
    self_alive: bool,
    other_alive: bool,
    same_team: bool,
    other_is_infantry: bool,
    distance: f32,
    radius: f32,
) -> bool {
    self_alive
        && other_alive
        && same_team
        && other_is_infantry
        && distance <= radius
        && distance >= 0.0
}

/// Whether residual fire should apply Red Guard residual path (gun honesty).
pub fn should_apply_red_guard_residual(is_red_guard: bool) -> bool {
    is_red_guard
}

/// Re-export nationalism tag helper for call sites.
pub fn apply_nationalism_tag_detect(applied_upgrades: &std::collections::HashSet<String>) -> bool {
    has_nationalism_upgrade(applied_upgrades)
}

// --- Wave 67 residual honesty packs ---

/// Wave 67 residual honesty: Red Guard gun / bayonet residual peel.
pub fn honesty_red_guard_weapon_residual_ok() -> bool {
    REDGUARD_MACHINE_GUN == "RedguardMachineGun"
        && REDGUARD_BAYONET == "RedguardBayonet"
        && (REDGUARD_DAMAGE - 15.0).abs() < 0.01
        && (REDGUARD_PRIMARY_RADIUS - 0.0).abs() < 0.01
        && (REDGUARD_RANGE - 100.0).abs() < 0.01
        && REDGUARD_BASE_DELAY_MS == 1_000
        && REDGUARD_BASE_DELAY_FRAMES == red_guard_ms_to_frames(REDGUARD_BASE_DELAY_MS)
        && REDGUARD_BASE_DELAY_FRAMES == 30
        && REDGUARD_DAMAGE_TYPE == "SMALL_ARMS"
        && REDGUARD_DEATH_TYPE == "NORMAL"
        && REDGUARD_FIRE_FX == "WeaponFX_GenericMachineGunFire"
        && REDGUARD_CLIP_SIZE == 0
        && REDGUARD_FIRE_AUDIO == "RedGuardWeapon"
        && (BAYONET_DAMAGE - 10_000.0).abs() < 0.1
        && (BAYONET_RANGE - 2.0).abs() < 0.01
        && BAYONET_DELAY_MS == 1_900
        && BAYONET_DELAY_FRAMES == red_guard_ms_to_frames(BAYONET_DELAY_MS)
        && BAYONET_PRE_ATTACK_MS == 1_400
        && BAYONET_PRE_ATTACK_FRAMES == red_guard_ms_to_frames(BAYONET_PRE_ATTACK_MS)
        && BAYONET_DAMAGE_TYPE == "MELEE"
        && BAYONET_FIRE_AUDIO == "HeroUSAKnifeAttack"
        && {
            let w = red_guard_weapon(false, false);
            (w.damage - 15.0).abs() < 0.01 && !w.can_target_air && w.can_target_ground
        }
}

/// Wave 67 residual honesty: infantry horde residual peel.
pub fn honesty_red_guard_horde_residual_ok() -> bool {
    (INFANTRY_HORDE_RADIUS - 30.0).abs() < 0.01
        && INFANTRY_HORDE_COUNT == 5
        && INFANTRY_HORDE_UPDATE_MS == 1_000
        && INFANTRY_HORDE_UPDATE_FRAMES == red_guard_ms_to_frames(INFANTRY_HORDE_UPDATE_MS)
        && !INFANTRY_HORDE_EXACT_MATCH
        && INFANTRY_HORDE_KIND_OF == "INFANTRY"
        && (INFANTRY_HORDE_ROF_MULT - 1.5).abs() < 0.001
        && (INFANTRY_NATIONALISM_ROF_MULT - 1.25).abs() < 0.001
        && red_guard_delay_frames(true, false) == 20
        && red_guard_delay_frames(true, true) == 16
        && is_in_infantry_horde(4)
        && !is_in_infantry_horde(3)
}

/// Wave 67 residual honesty: Red Guard body / capture residual peel.
pub fn honesty_red_guard_body_residual_ok() -> bool {
    (REDGUARD_MAX_HEALTH - 120.0).abs() < 0.01
        && (REDGUARD_VISION_RANGE - 100.0).abs() < 0.01
        && (REDGUARD_SHROUD_CLEARING_RANGE - 200.0).abs() < 0.01
        && REDGUARD_BUILD_COST == 300
        && (REDGUARD_BUILD_TIME_SEC - 10.0).abs() < 0.01
        && REDGUARD_BUILD_TIME_FRAMES
            == (REDGUARD_BUILD_TIME_SEC * RED_GUARD_LOGIC_FPS).round() as u32
        && REDGUARD_BUILD_TIME_FRAMES == 300
        && REDGUARD_TRANSPORT_SLOT_COUNT == 1
        && (REDGUARD_LOCOMOTOR_SPEED - 25.0).abs() < 0.01
        && (REDGUARD_LOCOMOTOR_SPEED_DAMAGED - 15.0).abs() < 0.01
        && (REDGUARD_GEOMETRY_RADIUS - 7.0).abs() < 0.01
        && (REDGUARD_GEOMETRY_HEIGHT - 12.0).abs() < 0.01
        && REDGUARD_EXPERIENCE_VALUE == [5, 5, 10, 20]
        && REDGUARD_EXPERIENCE_REQUIRED == [0, 20, 40, 80]
        && SPECIAL_ABILITY_RED_GUARD_CAPTURE == "SpecialAbilityRedGuardCaptureBuilding"
        && UPGRADE_INFANTRY_CAPTURE_BUILDING == "Upgrade_InfantryCaptureBuilding"
        && (REDGUARD_CAPTURE_START_RANGE - 5.0).abs() < 0.01
        && REDGUARD_CAPTURE_UNPACK_FRAMES == red_guard_ms_to_frames(REDGUARD_CAPTURE_UNPACK_MS)
        && REDGUARD_CAPTURE_PREP_FRAMES == red_guard_ms_to_frames(REDGUARD_CAPTURE_PREP_MS)
        && REDGUARD_CAPTURE_PACK_FRAMES == red_guard_ms_to_frames(REDGUARD_CAPTURE_PACK_MS)
}

/// Combined Wave 67 Red Guard residual honesty pack.
pub fn honesty_red_guard_residual_pack_ok() -> bool {
    honesty_red_guard_weapon_residual_ok()
        && honesty_red_guard_horde_residual_ok()
        && honesty_red_guard_body_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn red_guard_name_matrix() {
        assert!(is_red_guard_template("ChinaInfantryRedguard"));
        assert!(is_red_guard_template("China_RedGuard"));
        assert!(is_red_guard_template("Tank_ChinaInfantryRedguard"));
        assert!(is_red_guard_template("Nuke_ChinaInfantryRedguard"));
        assert!(is_red_guard_template("TestRedGuard"));
        assert!(is_red_guard_template("China_Soldier"));
        assert!(!is_red_guard_template("RedguardMachineGun"));
        assert!(!is_red_guard_template("RedguardBayonet"));
        assert!(!is_red_guard_template("SCIENCE_RedGuardTraining"));
        assert!(!is_red_guard_template("ChinaInfantryTankHunter"));
        assert!(!is_red_guard_template("RedguardLocomotor"));
        assert!(!is_red_guard_template("Upgrade_Nationalism"));
    }

    #[test]
    fn base_gun_stats() {
        let (d, r, f) = red_guard_weapon_stats(false, false);
        assert!((d - 15.0).abs() < 0.01);
        assert!((r - 100.0).abs() < 0.01);
        assert_eq!(f, 30);
        let w = red_guard_weapon(false, false);
        assert!((w.damage - 15.0).abs() < 0.01);
        assert!((w.reload_time - 1.0).abs() < 0.01);
        assert!(!w.can_target_air);
        assert!(w.can_target_ground);
    }

    #[test]
    fn horde_and_nationalism_rof_stack() {
        // HORDE alone: floor(30/1.5)=20
        assert_eq!(red_guard_delay_frames(true, false), 20);
        // Nationalism alone without horde does nothing residual.
        assert_eq!(red_guard_delay_frames(false, true), 30);
        // HORDE + NATIONALISM: floor(30/1.875)=16
        assert_eq!(red_guard_delay_frames(true, true), 16);

        let w_horde = red_guard_weapon(true, false);
        let w_both = red_guard_weapon(true, true);
        let w_base = red_guard_weapon(false, false);
        assert!(w_horde.reload_time < w_base.reload_time - 0.05);
        assert!(w_both.reload_time < w_horde.reload_time - 0.01);
    }

    #[test]
    fn horde_count_includes_self() {
        assert!(is_in_infantry_horde(4));
        assert!(!is_in_infantry_horde(3));
        assert!(!is_in_infantry_horde(0));
        assert!(is_in_infantry_horde(5));
    }

    #[test]
    fn bayonet_residual_filters() {
        assert!(should_apply_bayonet_residual(true, true, true, 1.5));
        assert!(should_apply_bayonet_residual(true, true, true, 2.0));
        assert!(!should_apply_bayonet_residual(true, true, true, 2.1));
        assert!(!should_apply_bayonet_residual(true, false, true, 1.0));
        assert!(!should_apply_bayonet_residual(false, true, true, 1.0));
        assert!(!should_apply_bayonet_residual(true, true, false, 1.0));
        let w = red_guard_bayonet_weapon();
        assert!((w.damage - 10_000.0).abs() < 0.1);
        assert!((w.range - 2.0).abs() < 0.01);
    }

    #[test]
    fn nationalism_tag_detect() {
        let mut tags = HashSet::new();
        tags.insert(UPGRADE_NATIONALISM.to_string());
        assert!(has_nationalism_upgrade(&tags));
        assert!(apply_nationalism_tag_detect(&tags));
    }

    #[test]
    fn counts_toward_infantry_horde_filters() {
        assert!(counts_toward_infantry_horde(
            true, true, true, true, 20.0, 30.0
        ));
        assert!(!counts_toward_infantry_horde(
            true, true, false, true, 20.0, 30.0
        ));
        assert!(!counts_toward_infantry_horde(
            true, true, true, true, 40.0, 30.0
        ));
        assert!(!counts_toward_infantry_horde(
            true, true, true, false, 10.0, 30.0
        ));
    }

    #[test]
    fn red_guard_residual_pack_honesty_wave67() {
        assert!(honesty_red_guard_weapon_residual_ok());
        assert!(honesty_red_guard_horde_residual_ok());
        assert!(honesty_red_guard_body_residual_ok());
        assert!(honesty_red_guard_residual_pack_ok());
        assert_eq!(red_guard_ms_to_frames(1_000), 30);
        assert_eq!(red_guard_ms_to_frames(1_900), 57);
        assert_eq!(red_guard_ms_to_frames(1_400), 42);
        assert_eq!(REDGUARD_BUILD_TIME_FRAMES, 300);
        assert_eq!(REDGUARD_DAMAGE_TYPE, "SMALL_ARMS");
        assert_eq!(BAYONET_DAMAGE_TYPE, "MELEE");
        assert!(!INFANTRY_HORDE_EXACT_MATCH);
    }
}
