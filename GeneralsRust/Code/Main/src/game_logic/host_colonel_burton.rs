//! Host Colonel Burton residual combat polish (sniper rifle + knife melee).
//!
//! Residual slice (playability):
//! - `AmericaInfantryColonelBurton` / SupW_/CINE_ variants spawn with PRIMARY
//!   `ColonelBurtonSniperRifleWeapon` (dmg **40** / range **125** / Delay **100**ms
//!   → 3 frames). ClipSize **3** honesty (ClipReload 500ms fail-closed volley matrix).
//! - Knife residual (`ColonelBurtonKnifeWeapon`): close-range infantry within **3**
//!   → MELEE one-shot PrimaryDamage **10000** (LeechRangeWeapon residual).
//! - Timed / remote demo charges already closed via host_mines / host_hero_abilities
//!   (Burton-specific weapon / plant residual peeled here for Wave 57 honesty).
//!
//! Wave 57 residual pack (retail INI honesty):
//! - Knife PreAttackDelay **833**ms → **25**f, PreAttackType **PER_ATTACK**,
//!   DamageType **MELEE**, DeathType **NORMAL**, LeechRangeWeapon **Yes**,
//!   ClipReloadTime **1367**ms → **41**f, ClipSize **1**, DelayBetweenShots **0**
//! - Remote/timed charge residual: RemoteC4 **MaxSpecialObjects 8**, TimedC4 **10**,
//!   UnpackTime **5500**ms → **165**f, FleeRangeAfterCompletion **100**,
//!   LoseStealthOnTrigger **Yes**, PreTriggerUnstealthTime **5000**ms → **150**f,
//!   SpecialPower SpecialAbilityColonelBurtonRemoteCharges / TimedCharges
//! - StealthUpdate residual: StealthDelay **2000**ms → **60**f, InnateStealth **Yes**,
//!   Forbidden **FIRING_PRIMARY**, FriendlyOpacityMin **50%**, Max **100%**,
//!   OrderIdleEnemiesToAttackMeUponReveal **Yes**
//! - Body residual: MaxHealth **200**, VisionRange **150**, ShroudClearingRange **500**,
//!   BuildCost **1500**
//!
//! Fail-closed honesty:
//! - Not full ClipSize=3 in-clip DelayBetweenShots + ClipReload 500ms volley matrix
//! - Not full knife PreAttackDelay anim lock / PER_ATTACK state machine interleave
//! - Not full StealthUpdate pulse / ChemicalSuits / AdvancedTraining residual matrix
//! - RemoteC4Charge/TimedC4Charge SpecialObject + MaxSpecialObjects residual closed
//! - Not full StickyBombUpdate attach bone matrix / live max-charge list UI
//! - Not network sniper / knife / charge replication (network deferred)

use super::Weapon;
use crate::game_logic::host_red_guard::delay_frames_to_reload_secs;

/// Logic frames per second (host fixed step).
pub const BURTON_LOGIC_FPS: f32 = 30.0;

/// Retail primary sniper weapon.
pub const BURTON_SNIPER_WEAPON: &str = "ColonelBurtonSniperRifleWeapon";
/// Retail secondary knife weapon.
pub const BURTON_KNIFE_WEAPON: &str = "ColonelBurtonKnifeWeapon";

/// Retail PrimaryDamage base (sniper).
pub const BURTON_SNIPER_DAMAGE: f32 = 40.0;
/// Retail AttackRange (sniper).
pub const BURTON_SNIPER_RANGE: f32 = 125.0;
/// Retail DelayBetweenShots 100ms → 3 frames @ 30 FPS.
pub const BURTON_SNIPER_BASE_DELAY_MS: u32 = 100;
pub const BURTON_SNIPER_BASE_DELAY_FRAMES: u32 = 3;
/// Retail ClipSize residual honesty (fail-closed volley matrix).
pub const BURTON_CLIP_SIZE: u32 = 3;
/// Retail ClipReloadTime 500ms → 15 frames @ 30 FPS (honesty only).
pub const BURTON_CLIP_RELOAD_MS: u32 = 500;
pub const BURTON_CLIP_RELOAD_FRAMES: u32 = 15;
/// Retail sniper DamageType residual.
pub const BURTON_SNIPER_DAMAGE_TYPE: &str = "SMALL_ARMS";
/// Retail sniper PrimaryDamageRadius residual (intended-only).
pub const BURTON_SNIPER_DAMAGE_RADIUS: f32 = 0.0;
/// Retail sniper FireSound residual.
pub const BURTON_SNIPER_FIRE_SOUND: &str = "SentryDroneWeapon";
/// Retail sniper FireFX residual.
pub const BURTON_SNIPER_FIRE_FX: &str = "WeaponFX_GenericMachineGunFire";

/// Knife PrimaryDamage residual (one-shot kill).
pub const BURTON_KNIFE_DAMAGE: f32 = 10_000.0;
/// Knife AttackRange residual.
pub const BURTON_KNIFE_RANGE: f32 = 3.0;
/// Knife ClipReloadTime 1367ms → 41 frames @ 30 FPS.
pub const BURTON_KNIFE_CLIP_RELOAD_MS: u32 = 1_367;
pub const BURTON_KNIFE_DELAY_FRAMES: u32 = 41;
/// Knife DelayBetweenShots residual (0 = clip-gated).
pub const BURTON_KNIFE_DELAY_BETWEEN_SHOTS_MS: u32 = 0;
/// Knife ClipSize residual.
pub const BURTON_KNIFE_CLIP_SIZE: u32 = 1;
/// Knife PreAttackDelay 833ms residual (fail-closed vs full pre-attack lock).
pub const BURTON_KNIFE_PRE_ATTACK_MS: u32 = 833;
pub const BURTON_KNIFE_PRE_ATTACK_FRAMES: u32 = 25;
/// Knife PreAttackType residual.
pub const BURTON_KNIFE_PRE_ATTACK_TYPE: &str = "PER_ATTACK";
/// Knife DamageType residual.
pub const BURTON_KNIFE_DAMAGE_TYPE: &str = "MELEE";
/// Knife DeathType residual.
pub const BURTON_KNIFE_DEATH_TYPE: &str = "NORMAL";
/// Knife LeechRangeWeapon residual.
pub const BURTON_KNIFE_LEECH_RANGE_WEAPON: bool = true;
/// Knife PrimaryDamageRadius residual (intended-only).
pub const BURTON_KNIFE_DAMAGE_RADIUS: f32 = 0.0;

/// Residual sniper fire audio.
pub const BURTON_SNIPER_FIRE_AUDIO: &str = "SentryDroneWeapon";
/// Residual knife fire audio.
pub const BURTON_KNIFE_FIRE_AUDIO: &str = "HeroUSAKnifeAttack";

// --- Body / vision residual ---

/// Retail MaxHealth residual.
pub const BURTON_MAX_HEALTH: f32 = 200.0;
/// Retail VisionRange residual.
pub const BURTON_VISION_RANGE: f32 = 150.0;
/// Retail ShroudClearingRange residual.
pub const BURTON_SHROUD_CLEARING_RANGE: f32 = 500.0;
/// Retail BuildCost residual.
pub const BURTON_BUILD_COST: u32 = 1_500;

// --- StealthUpdate residual ---

/// Retail StealthUpdate StealthDelay residual (msec).
pub const BURTON_STEALTH_DELAY_MS: u32 = 2_000;
/// StealthDelay 2000ms → 60 frames @ 30 FPS.
pub const BURTON_STEALTH_DELAY_FRAMES: u32 = 60;
/// Retail InnateStealth residual.
pub const BURTON_INNATE_STEALTH: bool = true;
/// Retail StealthForbiddenConditions = FIRING_PRIMARY residual.
pub const BURTON_STEALTH_BREAKS_ON_PRIMARY_FIRE: bool = true;
/// Retail: does not break stealth solely for secondary knife (FIRING_PRIMARY only).
pub const BURTON_STEALTH_BREAKS_ON_KNIFE: bool = false;
/// Retail FriendlyOpacityMin residual (percent).
pub const BURTON_FRIENDLY_OPACITY_MIN_PCT: f32 = 50.0;
/// Retail FriendlyOpacityMax residual (percent).
pub const BURTON_FRIENDLY_OPACITY_MAX_PCT: f32 = 100.0;
/// Retail OrderIdleEnemiesToAttackMeUponReveal residual.
pub const BURTON_ORDER_IDLE_ENEMIES_ON_REVEAL: bool = true;
/// Retail EnemyDetectionEvaEvent residual.
pub const BURTON_ENEMY_DETECTION_EVA: &str = "EnemyColonelBurtonDetected";
/// Retail OwnDetectionEvaEvent residual.
pub const BURTON_OWN_DETECTION_EVA: &str = "OwnColonelBurtonDetected";
/// Residual stealth on/off audio.
pub const BURTON_STEALTH_ON_AUDIO: &str = "StealthOn";
pub const BURTON_STEALTH_OFF_AUDIO: &str = "StealthOff";

// --- Remote / timed charge residual (Burton-specific peel; host_mines owns live plant) ---

/// Retail SpecialAbilityColonelBurtonRemoteCharges residual.
pub const BURTON_SPECIAL_REMOTE_CHARGES: &str = "SpecialAbilityColonelBurtonRemoteCharges";
/// Retail SpecialAbilityColonelBurtonTimedCharges residual.
pub const BURTON_SPECIAL_TIMED_CHARGES: &str = "SpecialAbilityColonelBurtonTimedCharges";
/// Retail RemoteC4Charge SpecialObject residual.
pub const BURTON_REMOTE_CHARGE_OBJECT: &str = "RemoteC4Charge";
/// Retail TimedC4Charge SpecialObject residual.
pub const BURTON_TIMED_CHARGE_OBJECT: &str = "TimedC4Charge";
/// Retail MaxSpecialObjects remote residual.
pub const BURTON_MAX_REMOTE_CHARGES: u32 = 8;
/// Retail MaxSpecialObjects timed residual.
pub const BURTON_MAX_TIMED_CHARGES: u32 = 10;
/// Retail UniqueSpecialObjectTargets residual.
pub const BURTON_UNIQUE_CHARGE_TARGETS: bool = true;
/// Retail UnpackTime residual for plant charge (msec).
pub const BURTON_CHARGE_UNPACK_TIME_MS: u32 = 5_500;
/// UnpackTime 5500ms → 165 frames @ 30 FPS.
pub const BURTON_CHARGE_UNPACK_TIME_FRAMES: u32 = 165;
/// Retail FleeRangeAfterCompletion residual.
pub const BURTON_FLEE_RANGE_AFTER_CHARGE: f32 = 100.0;
/// Retail LoseStealthOnTrigger residual.
pub const BURTON_LOSE_STEALTH_ON_CHARGE_TRIGGER: bool = true;
/// Retail PreTriggerUnstealthTime residual (msec).
pub const BURTON_PRE_TRIGGER_UNSTEALTH_MS: u32 = 5_000;
/// PreTriggerUnstealthTime 5000ms → 150 frames @ 30 FPS.
pub const BURTON_PRE_TRIGGER_UNSTEALTH_FRAMES: u32 = 150;
/// Retail SpecialObjectsPersistWhenOwnerDies remote residual (No).
pub const BURTON_REMOTE_PERSIST_WHEN_OWNER_DIES: bool = false;
/// Retail SpecialObjectsPersistWhenOwnerDies timed residual (Yes).
pub const BURTON_TIMED_PERSIST_WHEN_OWNER_DIES: bool = true;
/// Retail UnpackSound residual.
pub const BURTON_PLANT_CHARGE_AUDIO: &str = "ColonelBurtonPlantCharge";
/// Retail InitiateSound residual for remote plant.
pub const BURTON_VOICE_PLANT_REMOTE: &str = "ColonelBurtonVoicePlantRemoteCharge";
/// Retail InitiateSound residual for timed plant.
pub const BURTON_VOICE_PLANT_TIMED: &str = "ColonelBurtonVoicePlantTimedCharge";
/// Retail ViewObjectRange residual on timed special power.
pub const BURTON_TIMED_CHARGE_VIEW_OBJECT_RANGE: f32 = 100.0;

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn burton_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / BURTON_LOGIC_FPS)).round() as u32
}

/// Whether template is a residual Colonel Burton hero.
///
/// Fail-closed: name residual. Excludes weapons / science / debris tokens.
pub fn is_colonel_burton_template(template_name: &str) -> bool {
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
        || n.contains("charge")
        || n.contains("demo")
        || n.contains("plant")
    {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "testburton"
        || n == "testcolonelburton"
        || n == "colonel_burton"
        || n == "usa_burton"
        || n == "usa_colonelburton"
    {
        return true;
    }
    n.contains("colonelburton") || n.contains("colonel_burton") || n.contains("burton")
}

/// Sniper delay frames residual (base DelayBetweenShots; clip reload fail-closed).
pub fn burton_sniper_delay_frames() -> u32 {
    BURTON_SNIPER_BASE_DELAY_FRAMES
}

/// (damage, range, delay_frames) for sniper residual.
pub fn burton_sniper_weapon_stats() -> (f32, f32, u32) {
    (
        BURTON_SNIPER_DAMAGE,
        BURTON_SNIPER_RANGE,
        burton_sniper_delay_frames(),
    )
}

/// Build residual PRIMARY sniper Weapon.
pub fn burton_sniper_weapon() -> Weapon {
    let (damage, range, delay) = burton_sniper_weapon_stats();
    Weapon {
        damage,
        range,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(delay),
        last_fire_time: 0.0,
        ammo: Some(BURTON_CLIP_SIZE),
        clip_size: 0,
        clip_reload_time: 0.0,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 999_999.0,
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
    }
}

/// Residual knife Weapon (close-range one-shot).
pub fn burton_knife_weapon() -> Weapon {
    Weapon {
        damage: BURTON_KNIFE_DAMAGE,
        range: BURTON_KNIFE_RANGE,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(BURTON_KNIFE_DELAY_FRAMES),
        last_fire_time: 0.0,
        ammo: Some(BURTON_KNIFE_CLIP_SIZE),
        clip_size: 0,
        clip_reload_time: 0.0,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 999_999.0,
        pre_attack_delay: delay_frames_to_reload_secs(BURTON_KNIFE_PRE_ATTACK_FRAMES),
        splash_radius: 0.0,
    }
}

/// Whether knife residual should apply for this shot.
///
/// Residual: target is living infantry, horizontal distance ≤ BURTON_KNIFE_RANGE.
pub fn should_apply_burton_knife_residual(
    is_burton: bool,
    target_is_infantry: bool,
    target_alive: bool,
    distance: f32,
) -> bool {
    is_burton
        && target_is_infantry
        && target_alive
        && distance <= BURTON_KNIFE_RANGE
        && distance >= 0.0
}

/// 2D distance residual (XZ plane).
pub fn distance_2d(ax: f32, az: f32, bx: f32, bz: f32) -> f32 {
    let dx = ax - bx;
    let dz = az - bz;
    (dx * dx + dz * dz).sqrt()
}

/// Whether residual fire should apply Burton residual path (sniper/knife honesty).
pub fn should_apply_burton_residual(is_burton: bool) -> bool {
    is_burton
}

/// Legal residual fire target.
pub fn is_legal_burton_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind
}

/// Maintain Burton stealth residual (FIRING_PRIMARY breaks cloak).
///
/// Returns `Some(desired_stealthed)` for honesty bookkeeping.
pub fn burton_stealth_desired(
    is_burton: bool,
    innate_stealth: bool,
    is_alive: bool,
    firing_primary: bool,
) -> Option<bool> {
    if !is_burton || !innate_stealth || !is_alive {
        return None;
    }
    if BURTON_STEALTH_BREAKS_ON_PRIMARY_FIRE && firing_primary {
        Some(false)
    } else {
        Some(true)
    }
}

/// Whether producer may plant another remote charge (MaxSpecialObjects residual).
pub fn can_plant_remote_charge(active_remote_count: u32) -> bool {
    active_remote_count < BURTON_MAX_REMOTE_CHARGES
}

/// Whether producer may plant another timed charge (MaxSpecialObjects residual).
pub fn can_plant_timed_charge(active_timed_count: u32) -> bool {
    active_timed_count < BURTON_MAX_TIMED_CHARGES
}

// --- Wave 57 residual honesty packs ---

/// Wave 57 residual honesty: sniper weapon residual.
pub fn honesty_burton_sniper_residual_ok() -> bool {
    (BURTON_SNIPER_DAMAGE - 40.0).abs() < 0.01
        && (BURTON_SNIPER_RANGE - 125.0).abs() < 0.01
        && BURTON_SNIPER_BASE_DELAY_MS == 100
        && BURTON_SNIPER_BASE_DELAY_FRAMES == burton_ms_to_frames(BURTON_SNIPER_BASE_DELAY_MS)
        && BURTON_CLIP_SIZE == 3
        && BURTON_CLIP_RELOAD_MS == 500
        && BURTON_CLIP_RELOAD_FRAMES == burton_ms_to_frames(BURTON_CLIP_RELOAD_MS)
        && BURTON_SNIPER_WEAPON == "ColonelBurtonSniperRifleWeapon"
        && BURTON_SNIPER_DAMAGE_TYPE == "SMALL_ARMS"
        && (BURTON_SNIPER_DAMAGE_RADIUS - 0.0).abs() < 0.01
        && BURTON_SNIPER_FIRE_AUDIO == "SentryDroneWeapon"
        && BURTON_SNIPER_FIRE_SOUND == "SentryDroneWeapon"
        && BURTON_SNIPER_FIRE_FX == "WeaponFX_GenericMachineGunFire"
}

/// Wave 57 residual honesty: knife weapon residual (PreAttackDelay 833ms + damage).
pub fn honesty_burton_knife_residual_ok() -> bool {
    (BURTON_KNIFE_DAMAGE - 10_000.0).abs() < 0.1
        && (BURTON_KNIFE_RANGE - 3.0).abs() < 0.01
        && BURTON_KNIFE_PRE_ATTACK_MS == 833
        && BURTON_KNIFE_PRE_ATTACK_FRAMES == burton_ms_to_frames(BURTON_KNIFE_PRE_ATTACK_MS)
        && BURTON_KNIFE_CLIP_RELOAD_MS == 1_367
        && BURTON_KNIFE_DELAY_FRAMES == burton_ms_to_frames(BURTON_KNIFE_CLIP_RELOAD_MS)
        && BURTON_KNIFE_DELAY_BETWEEN_SHOTS_MS == 0
        && BURTON_KNIFE_CLIP_SIZE == 1
        && BURTON_KNIFE_PRE_ATTACK_TYPE == "PER_ATTACK"
        && BURTON_KNIFE_DAMAGE_TYPE == "MELEE"
        && BURTON_KNIFE_DEATH_TYPE == "NORMAL"
        && BURTON_KNIFE_LEECH_RANGE_WEAPON
        && (BURTON_KNIFE_DAMAGE_RADIUS - 0.0).abs() < 0.01
        && BURTON_KNIFE_WEAPON == "ColonelBurtonKnifeWeapon"
        && BURTON_KNIFE_FIRE_AUDIO == "HeroUSAKnifeAttack"
        && should_apply_burton_knife_residual(true, true, true, 3.0)
        && !should_apply_burton_knife_residual(true, true, true, 3.1)
}

/// Wave 57 residual honesty: StealthUpdate residual.
pub fn honesty_burton_stealth_residual_ok() -> bool {
    BURTON_STEALTH_DELAY_MS == 2_000
        && BURTON_STEALTH_DELAY_FRAMES == burton_ms_to_frames(BURTON_STEALTH_DELAY_MS)
        && BURTON_INNATE_STEALTH
        && BURTON_STEALTH_BREAKS_ON_PRIMARY_FIRE
        && !BURTON_STEALTH_BREAKS_ON_KNIFE
        && (BURTON_FRIENDLY_OPACITY_MIN_PCT - 50.0).abs() < 0.01
        && (BURTON_FRIENDLY_OPACITY_MAX_PCT - 100.0).abs() < 0.01
        && BURTON_ORDER_IDLE_ENEMIES_ON_REVEAL
        && BURTON_ENEMY_DETECTION_EVA == "EnemyColonelBurtonDetected"
        && BURTON_OWN_DETECTION_EVA == "OwnColonelBurtonDetected"
        && BURTON_STEALTH_ON_AUDIO == "StealthOn"
        && BURTON_STEALTH_OFF_AUDIO == "StealthOff"
        && burton_stealth_desired(true, true, true, true) == Some(false)
        && burton_stealth_desired(true, true, true, false) == Some(true)
}

/// Wave 57 residual honesty: remote/timed charge residual peel.
pub fn honesty_burton_charge_residual_ok() -> bool {
    BURTON_SPECIAL_REMOTE_CHARGES == "SpecialAbilityColonelBurtonRemoteCharges"
        && BURTON_SPECIAL_TIMED_CHARGES == "SpecialAbilityColonelBurtonTimedCharges"
        && BURTON_REMOTE_CHARGE_OBJECT == "RemoteC4Charge"
        && BURTON_TIMED_CHARGE_OBJECT == "TimedC4Charge"
        && BURTON_MAX_REMOTE_CHARGES == 8
        && BURTON_MAX_TIMED_CHARGES == 10
        && BURTON_UNIQUE_CHARGE_TARGETS
        && BURTON_CHARGE_UNPACK_TIME_MS == 5_500
        && BURTON_CHARGE_UNPACK_TIME_FRAMES == burton_ms_to_frames(BURTON_CHARGE_UNPACK_TIME_MS)
        && (BURTON_FLEE_RANGE_AFTER_CHARGE - 100.0).abs() < 0.01
        && BURTON_LOSE_STEALTH_ON_CHARGE_TRIGGER
        && BURTON_PRE_TRIGGER_UNSTEALTH_MS == 5_000
        && BURTON_PRE_TRIGGER_UNSTEALTH_FRAMES
            == burton_ms_to_frames(BURTON_PRE_TRIGGER_UNSTEALTH_MS)
        && !BURTON_REMOTE_PERSIST_WHEN_OWNER_DIES
        && BURTON_TIMED_PERSIST_WHEN_OWNER_DIES
        && BURTON_PLANT_CHARGE_AUDIO == "ColonelBurtonPlantCharge"
        && BURTON_VOICE_PLANT_REMOTE == "ColonelBurtonVoicePlantRemoteCharge"
        && BURTON_VOICE_PLANT_TIMED == "ColonelBurtonVoicePlantTimedCharge"
        && (BURTON_TIMED_CHARGE_VIEW_OBJECT_RANGE - 100.0).abs() < 0.01
        && can_plant_remote_charge(7)
        && !can_plant_remote_charge(8)
        && can_plant_timed_charge(9)
        && !can_plant_timed_charge(10)
}

/// Wave 57 residual honesty: body / vision residual.
pub fn honesty_burton_body_residual_ok() -> bool {
    (BURTON_MAX_HEALTH - 200.0).abs() < 0.01
        && (BURTON_VISION_RANGE - 150.0).abs() < 0.01
        && (BURTON_SHROUD_CLEARING_RANGE - 500.0).abs() < 0.01
        && BURTON_BUILD_COST == 1_500
}

/// Combined Wave 57 Colonel Burton residual honesty pack.
pub fn honesty_colonel_burton_residual_pack_ok() -> bool {
    honesty_burton_sniper_residual_ok()
        && honesty_burton_knife_residual_ok()
        && honesty_burton_stealth_residual_ok()
        && honesty_burton_charge_residual_ok()
        && honesty_burton_body_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn burton_name_matrix() {
        assert!(is_colonel_burton_template("AmericaInfantryColonelBurton"));
        assert!(is_colonel_burton_template(
            "SupW_AmericaInfantryColonelBurton"
        ));
        assert!(is_colonel_burton_template(
            "CINE_AmericaInfantryColonelBurton"
        ));
        assert!(is_colonel_burton_template("TestBurton"));
        assert!(is_colonel_burton_template("USA_ColonelBurton"));
        assert!(!is_colonel_burton_template(
            "ColonelBurtonSniperRifleWeapon"
        ));
        assert!(!is_colonel_burton_template("ColonelBurtonKnifeWeapon"));
        assert!(!is_colonel_burton_template("ColonelBurtonSetDemoCharge"));
        assert!(!is_colonel_burton_template("ColonelBurtonSetRemoteCharge"));
        assert!(!is_colonel_burton_template("ColonelBurtonGroundLocomotor"));
        assert!(!is_colonel_burton_template("AmericaInfantryRanger"));
        assert!(!is_colonel_burton_template("ChinaInfantryBlackLotus"));
    }

    #[test]
    fn sniper_stats() {
        let (d, r, f) = burton_sniper_weapon_stats();
        assert!((d - 40.0).abs() < 0.01);
        assert!((r - 125.0).abs() < 0.01);
        assert_eq!(f, 3);
        let w = burton_sniper_weapon();
        assert!((w.damage - 40.0).abs() < 0.01);
        assert!((w.reload_time - (3.0 / 30.0)).abs() < 0.01);
        assert_eq!(w.ammo, Some(3));
        assert!(!w.can_target_air && w.can_target_ground);
    }

    #[test]
    fn knife_stats_and_gate() {
        let k = burton_knife_weapon();
        assert!((k.damage - 10_000.0).abs() < 0.1);
        assert!((k.range - 3.0).abs() < 0.01);
        assert!((k.pre_attack_delay - (25.0 / 30.0)).abs() < 0.05);
        assert!(should_apply_burton_knife_residual(true, true, true, 2.5));
        assert!(should_apply_burton_knife_residual(true, true, true, 3.0));
        assert!(!should_apply_burton_knife_residual(true, true, true, 3.1));
        assert!(!should_apply_burton_knife_residual(true, false, true, 1.0));
        assert!(!should_apply_burton_knife_residual(false, true, true, 1.0));
        assert!(!should_apply_burton_knife_residual(true, true, false, 1.0));
    }

    #[test]
    fn legal_and_apply() {
        assert!(should_apply_burton_residual(true));
        assert!(!should_apply_burton_residual(false));
        assert!(is_legal_burton_target(true, false, false, true));
        assert!(!is_legal_burton_target(false, false, false, true));
        assert!(!is_legal_burton_target(true, true, false, true));
    }

    #[test]
    fn colonel_burton_residual_pack_honesty() {
        assert!(honesty_colonel_burton_residual_pack_ok());
        assert_eq!(burton_ms_to_frames(833), 25);
        assert_eq!(burton_ms_to_frames(1_367), 41);
        assert_eq!(burton_ms_to_frames(100), 3);
        assert_eq!(burton_ms_to_frames(500), 15);
        assert_eq!(burton_ms_to_frames(2_000), 60);
        assert_eq!(burton_ms_to_frames(5_500), 165);
        assert_eq!(burton_ms_to_frames(5_000), 150);
        assert_eq!(burton_ms_to_frames(0), 0);
    }

    #[test]
    fn burton_stealth_and_charges() {
        assert_eq!(
            burton_stealth_desired(true, true, true, true),
            Some(false),
            "primary fire uncloaks"
        );
        assert_eq!(
            burton_stealth_desired(true, true, true, false),
            Some(true),
            "idle re-cloaks after delay residual"
        );
        assert_eq!(burton_stealth_desired(false, true, true, false), None);
        assert!(can_plant_remote_charge(0));
        assert!(!can_plant_remote_charge(BURTON_MAX_REMOTE_CHARGES));
        assert!(can_plant_timed_charge(0));
        assert!(!can_plant_timed_charge(BURTON_MAX_TIMED_CHARGES));
    }
}
