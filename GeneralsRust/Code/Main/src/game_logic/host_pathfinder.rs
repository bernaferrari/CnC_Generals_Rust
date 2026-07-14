//! Host America Pathfinder residual (innate stealth + stealth detector + sniper).
//!
//! Residual slice (playability):
//! - Pathfinder is always a stealth detector residual (`StealthDetectorUpdate`;
//!   DetectionRange unset → VisionRange = **200**).
//! - Innate stealth (`StealthUpdate InnateStealth = Yes`) from spawn.
//! - Stays stealthed while attacking (`StealthForbiddenConditions = MOVING` only;
//!   `stealth_breaks_on_attack = false`).
//! - Uncloaks while moving; re-cloaks immediately when stopped (StealthDelay = 0).
//! - PRIMARY `USAPathfinderSniperRifle` (100 dmg / 300 range / 2000 ms).
//!
//! Wave 54 residual pack (retail INI honesty):
//! - SCIENCE_Pathfinder prereq gate residual (SCIENCE_AMERICA + SCIENCE_Rank3)
//! - StealthUpdate: StealthDelay **0**, InnateStealth **Yes**, Forbidden **MOVING**,
//!   FriendlyOpacityMin **30%**, Max **80%**, PulseFrequency **500**ms → **15**f,
//!   MoveThresholdSpeed **3**, OrderIdleEnemiesToAttackMeUponReveal **Yes**
//! - StealthDetectorUpdate: DetectionRate **500**ms → **15**f,
//!   CanDetectWhileGarrisoned/Contained **No**, DetectionRange → VisionRange **200**
//! - VisionRange **200**, ShroudClearingRange **400**, BuildCost **600**, MaxHealth **120**
//! - Sniper AP upgrade WeaponBonus DAMAGE **125%** residual
//! - No pack/unpack (not a SpecialAbility unit residual)
//!
//! Fail-closed honesty:
//! - Not full StealthUpdate pulse / FriendlyOpacity drawable interleave
//! - Not full IR detector FX / CanDetectWhileGarrisoned matrix beyond residual flags
//! - Not full SCIENCE purchase UI / command-button prereq graph
//! - Not network detector / stealth replication (network deferred)

/// Logic frames per second (host fixed step).
pub const PATHFINDER_LOGIC_FPS: f32 = 30.0;

/// Retail primary weapon template name.
pub const PATHFINDER_SNIPER_WEAPON: &str = "USAPathfinderSniperRifle";

/// Retail VisionRange (used as DetectionRange when unset).
pub const PATHFINDER_DETECTION_RANGE: f32 = 200.0;
/// Retail VisionRange residual (alias).
pub const PATHFINDER_VISION_RANGE: f32 = 200.0;
/// Retail ShroudClearingRange residual.
pub const PATHFINDER_SHROUD_CLEARING_RANGE: f32 = 400.0;

/// Retail sniper PrimaryDamage.
pub const PATHFINDER_SNIPER_DAMAGE: f32 = 100.0;
/// Retail sniper AttackRange.
pub const PATHFINDER_SNIPER_RANGE: f32 = 300.0;
/// Retail DelayBetweenShots 2000 ms → 60 frames @ 30 FPS.
pub const PATHFINDER_SNIPER_DELAY_MS: u32 = 2_000;
pub const PATHFINDER_SNIPER_DELAY_FRAMES: u32 = 60;
/// Retail WeaponBonus PLAYER_UPGRADE DAMAGE 125% residual multiplier.
pub const PATHFINDER_AP_DAMAGE_MULT: f32 = 1.25;

/// Residual audio event name.
pub const PATHFINDER_WEAPON_AUDIO: &str = "PathfinderWeapon";

/// Retail SCIENCE_Pathfinder residual name.
pub const SCIENCE_PATHFINDER: &str = "SCIENCE_Pathfinder";
/// Retail prereq sciences residual markers.
pub const PATHFINDER_PREREQ_SCIENCES: &[&str] = &["SCIENCE_AMERICA", "SCIENCE_Rank3"];
/// Retail SciencePurchasePointCost residual.
pub const PATHFINDER_SCIENCE_POINT_COST: u32 = 1;

/// Retail BuildCost residual.
pub const PATHFINDER_BUILD_COST: u32 = 600;
/// Retail MaxHealth residual.
pub const PATHFINDER_MAX_HEALTH: f32 = 120.0;

// --- StealthUpdate residual ---

/// Retail StealthUpdate StealthDelay residual (msec) — 0 = re-cloak immediately.
pub const PATHFINDER_STEALTH_DELAY_MS: u32 = 0;
pub const PATHFINDER_STEALTH_DELAY_FRAMES: u32 = 0;
/// Retail InnateStealth residual.
pub const PATHFINDER_INNATE_STEALTH: bool = true;
/// Retail StealthForbiddenConditions = MOVING residual (breaks on move, not attack).
pub const PATHFINDER_STEALTH_BREAKS_ON_MOVE: bool = true;
/// Retail: does NOT break stealth while attacking.
pub const PATHFINDER_STEALTH_BREAKS_ON_ATTACK: bool = false;
/// Retail FriendlyOpacityMin residual (percent).
pub const PATHFINDER_FRIENDLY_OPACITY_MIN_PCT: f32 = 30.0;
/// Retail FriendlyOpacityMax residual (percent).
pub const PATHFINDER_FRIENDLY_OPACITY_MAX_PCT: f32 = 80.0;
/// Retail PulseFrequency residual (msec).
pub const PATHFINDER_PULSE_FREQUENCY_MS: u32 = 500;
/// PulseFrequency 500ms → 15 frames @ 30 FPS.
pub const PATHFINDER_PULSE_FREQUENCY_FRAMES: u32 = 15;
/// Retail MoveThresholdSpeed residual.
pub const PATHFINDER_MOVE_THRESHOLD_SPEED: f32 = 3.0;
/// Retail OrderIdleEnemiesToAttackMeUponReveal residual.
pub const PATHFINDER_ORDER_IDLE_ENEMIES_ON_REVEAL: bool = true;

// --- StealthDetectorUpdate residual ---

/// Retail StealthDetectorUpdate DetectionRate residual (msec).
pub const PATHFINDER_DETECTION_RATE_MS: u32 = 500;
/// DetectionRate 500ms → 15 frames @ 30 FPS.
pub const PATHFINDER_DETECTION_RATE_FRAMES: u32 = 15;
/// Retail CanDetectWhileGarrisoned residual.
pub const PATHFINDER_CAN_DETECT_WHILE_GARRISONED: bool = false;
/// Retail CanDetectWhileContained residual.
pub const PATHFINDER_CAN_DETECT_WHILE_CONTAINED: bool = false;

/// Residual: Pathfinder has no SpecialAbility pack/unpack residual.
pub const PATHFINDER_HAS_PACK_UNPACK: bool = false;

// --- Wave 81 residual deepen (AmericaInfantry.ini Pathfinder body / locomotor) ---

/// Retail SET_NORMAL locomotor residual (shared with Colonel Burton ground).
pub const PATHFINDER_LOCOMOTOR: &str = "ColonelBurtonGroundLocomotor";
/// Retail ColonelBurtonGroundLocomotor Speed residual (dist/sec).
pub const PATHFINDER_LOCOMOTOR_SPEED: f32 = 30.0;
/// Retail SpeedDamaged residual.
pub const PATHFINDER_LOCOMOTOR_SPEED_DAMAGED: f32 = 20.0;
/// Retail TurnRate residual (degrees/sec).
pub const PATHFINDER_LOCOMOTOR_TURN_RATE_DEG: f32 = 500.0;
/// Retail Acceleration residual (dist/sec²).
pub const PATHFINDER_LOCOMOTOR_ACCEL: f32 = 100.0;
/// Retail AccelerationDamaged residual.
pub const PATHFINDER_LOCOMOTOR_ACCEL_DAMAGED: f32 = 50.0;

/// Retail ArmorSet Conditions=None residual.
pub const PATHFINDER_ARMOR: &str = "HumanArmor";
/// Retail ArmorSet PLAYER_UPGRADE residual.
pub const PATHFINDER_ARMOR_CHEM_SUIT: &str = "ChemSuitHumanArmor";
/// Retail DamageFX residual.
pub const PATHFINDER_DAMAGE_FX: &str = "InfantryDamageFX";

/// Retail BuildTime residual (seconds).
pub const PATHFINDER_BUILD_TIME_SEC: f32 = 10.0;
/// Retail ExperienceValue residual (Vet0..Heroic).
pub const PATHFINDER_EXPERIENCE_VALUE: [u32; 4] = [40, 40, 60, 80];
/// Retail ExperienceRequired residual (levels 0..3).
pub const PATHFINDER_EXPERIENCE_REQUIRED: [u32; 4] = [0, 50, 100, 200];
/// Retail IsTrainable residual.
pub const PATHFINDER_IS_TRAINABLE: bool = true;
/// Retail CrushableLevel residual (infantry = 0).
pub const PATHFINDER_CRUSHABLE_LEVEL: u32 = 0;
/// Retail TransportSlotCount residual.
pub const PATHFINDER_TRANSPORT_SLOT_COUNT: u32 = 1;

// AutoFindHealingUpdate residual (ModuleTag_04)
/// Retail AutoFindHealingUpdate ScanRate residual (msec).
pub const PATHFINDER_HEAL_SCAN_RATE_MS: u32 = 1_000;
/// ScanRate 1000 ms → 30 frames @ 30 FPS.
pub const PATHFINDER_HEAL_SCAN_RATE_FRAMES: u32 = 30;
/// Retail ScanRange residual.
pub const PATHFINDER_HEAL_SCAN_RANGE: f32 = 300.0;
/// Retail NeverHeal residual (skip heal above this health fraction).
pub const PATHFINDER_HEAL_NEVER_FRACTION: f32 = 0.85;
/// Retail AlwaysHeal residual (force heal below this health fraction).
pub const PATHFINDER_HEAL_ALWAYS_FRACTION: f32 = 0.25;

/// Retail AIUpdate MoodAttackCheckRate residual (msec).
pub const PATHFINDER_MOOD_ATTACK_CHECK_RATE_MS: u32 = 250;
/// MoodAttackCheckRate 250 ms → 8 frames (round half-up: 7.5 → 8).
pub const PATHFINDER_MOOD_ATTACK_CHECK_RATE_FRAMES: u32 = 8;
/// Retail PhysicsBehavior Mass residual.
pub const PATHFINDER_PHYSICS_MASS: f32 = 5.0;

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn pathfinder_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / PATHFINDER_LOGIC_FPS)).round() as u32
}

/// Whether template is a residual Pathfinder infantry.
///
/// Fail-closed: name residual (not full SCIENCE_Pathfinder prereq graph).
pub fn is_pathfinder_template(template_name: &str) -> bool {
    let n = template_name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect::<String>();
    if n.is_empty() {
        return false;
    }
    // Weapon / upgrade / science tokens are not the living unit.
    if n.contains("weapon")
        || n.contains("sniper")
        || n.contains("rifle")
        || n.starts_with("upgrade")
        || n.starts_with("science")
        || n.contains("command")
    {
        return false;
    }
    n.contains("pathfinder") || n == "usapathfinder"
}

/// Residual SCIENCE_Pathfinder gate name matcher.
pub fn is_pathfinder_science_name(name: &str) -> bool {
    let n = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect::<String>();
    n == "sciencepathfinder" || n == "science_pathfinder" || n.ends_with("sciencepathfinder")
}

/// Whether residual spawn is gated on SCIENCE_Pathfinder residual.
///
/// Fail-closed: host residual checks name/science flag; not full player science graph.
pub fn pathfinder_spawn_requires_science(has_science_pathfinder: bool) -> bool {
    has_science_pathfinder
}

/// Whether residual spawn should install detector + innate stealth fields.
pub fn pathfinder_spawn_is_detector(template_name: &str) -> bool {
    is_pathfinder_template(template_name)
}

/// Detection range residual for Pathfinder (retail VisionRange = 200).
pub fn pathfinder_detection_range(template_name: &str) -> Option<f32> {
    if is_pathfinder_template(template_name) {
        Some(PATHFINDER_DETECTION_RANGE)
    } else {
        None
    }
}

/// Sniper damage residual with optional AP upgrade.
pub fn pathfinder_sniper_damage(has_ap_upgrade: bool) -> f32 {
    if has_ap_upgrade {
        PATHFINDER_SNIPER_DAMAGE * PATHFINDER_AP_DAMAGE_MULT
    } else {
        PATHFINDER_SNIPER_DAMAGE
    }
}

/// Maintain Pathfinder move-forbidden stealth residual.
///
/// Returns `(should_be_stealthed, changed)` for honesty bookkeeping when cloak
/// state flips due to MOVING / stop.
pub fn pathfinder_stealth_desired(
    is_pathfinder: bool,
    innate_stealth: bool,
    stealth_breaks_on_move: bool,
    is_alive: bool,
    is_moving_state: bool,
) -> Option<bool> {
    if !is_pathfinder || !innate_stealth || !is_alive {
        return None;
    }
    if stealth_breaks_on_move && is_moving_state {
        Some(false)
    } else {
        Some(true)
    }
}

/// Stealth residual while attacking (retail Forbidden = MOVING only).
pub fn pathfinder_stealth_while_attacking(
    is_pathfinder: bool,
    innate_stealth: bool,
    is_alive: bool,
    is_attacking: bool,
    is_moving: bool,
) -> Option<bool> {
    if !is_pathfinder || !innate_stealth || !is_alive {
        return None;
    }
    // Attack does not break stealth; move does.
    let _ = is_attacking;
    if PATHFINDER_STEALTH_BREAKS_ON_MOVE && is_moving {
        Some(false)
    } else {
        Some(true)
    }
}

/// Wave 54 residual honesty: detect range / sniper residual.
pub fn honesty_pathfinder_detect_sniper_residual_ok() -> bool {
    (PATHFINDER_DETECTION_RANGE - 200.0).abs() < 0.01
        && (PATHFINDER_VISION_RANGE - 200.0).abs() < 0.01
        && (PATHFINDER_SHROUD_CLEARING_RANGE - 400.0).abs() < 0.01
        && (PATHFINDER_SNIPER_DAMAGE - 100.0).abs() < 0.01
        && (PATHFINDER_SNIPER_RANGE - 300.0).abs() < 0.01
        && PATHFINDER_SNIPER_DELAY_MS == 2_000
        && PATHFINDER_SNIPER_DELAY_FRAMES == pathfinder_ms_to_frames(PATHFINDER_SNIPER_DELAY_MS)
        && PATHFINDER_SNIPER_WEAPON == "USAPathfinderSniperRifle"
        && PATHFINDER_WEAPON_AUDIO == "PathfinderWeapon"
        && (pathfinder_sniper_damage(true) - 125.0).abs() < 0.01
}

/// Wave 54 residual honesty: StealthUpdate residual.
pub fn honesty_pathfinder_stealth_update_residual_ok() -> bool {
    PATHFINDER_STEALTH_DELAY_MS == 0
        && PATHFINDER_STEALTH_DELAY_FRAMES == 0
        && PATHFINDER_INNATE_STEALTH
        && PATHFINDER_STEALTH_BREAKS_ON_MOVE
        && !PATHFINDER_STEALTH_BREAKS_ON_ATTACK
        && (PATHFINDER_FRIENDLY_OPACITY_MIN_PCT - 30.0).abs() < 0.01
        && (PATHFINDER_FRIENDLY_OPACITY_MAX_PCT - 80.0).abs() < 0.01
        && PATHFINDER_PULSE_FREQUENCY_MS == 500
        && PATHFINDER_PULSE_FREQUENCY_FRAMES
            == pathfinder_ms_to_frames(PATHFINDER_PULSE_FREQUENCY_MS)
        && (PATHFINDER_MOVE_THRESHOLD_SPEED - 3.0).abs() < 0.01
        && PATHFINDER_ORDER_IDLE_ENEMIES_ON_REVEAL
        && !PATHFINDER_HAS_PACK_UNPACK
}

/// Wave 54 residual honesty: StealthDetectorUpdate residual.
pub fn honesty_pathfinder_detector_residual_ok() -> bool {
    PATHFINDER_DETECTION_RATE_MS == 500
        && PATHFINDER_DETECTION_RATE_FRAMES == pathfinder_ms_to_frames(PATHFINDER_DETECTION_RATE_MS)
        && !PATHFINDER_CAN_DETECT_WHILE_GARRISONED
        && !PATHFINDER_CAN_DETECT_WHILE_CONTAINED
        && (PATHFINDER_BUILD_COST == 600)
        && (PATHFINDER_MAX_HEALTH - 120.0).abs() < 0.01
}

/// Wave 54 residual honesty: SCIENCE_Pathfinder gate residual.
pub fn honesty_pathfinder_science_gate_residual_ok() -> bool {
    SCIENCE_PATHFINDER == "SCIENCE_Pathfinder"
        && PATHFINDER_SCIENCE_POINT_COST == 1
        && PATHFINDER_PREREQ_SCIENCES.len() == 2
        && PATHFINDER_PREREQ_SCIENCES.contains(&"SCIENCE_AMERICA")
        && PATHFINDER_PREREQ_SCIENCES.contains(&"SCIENCE_Rank3")
        && is_pathfinder_science_name("SCIENCE_Pathfinder")
        && !is_pathfinder_science_name("SCIENCE_CashHack1")
        && pathfinder_spawn_requires_science(true)
        && !pathfinder_spawn_requires_science(false)
}

/// Combined Wave 54 Pathfinder residual honesty pack.
pub fn honesty_pathfinder_residual_pack_ok() -> bool {
    honesty_pathfinder_detect_sniper_residual_ok()
        && honesty_pathfinder_stealth_update_residual_ok()
        && honesty_pathfinder_detector_residual_ok()
        && honesty_pathfinder_science_gate_residual_ok()
}

/// Wave 81 residual honesty: Pathfinder body / locomotor / heal residual deepen.
///
/// AmericaInfantry.ini: ColonelBurtonGroundLocomotor, HumanArmor, BuildTime 10,
/// ExperienceValue/Required, AutoFindHealingUpdate, Physics mass.
/// Fail-closed: not full AIUpdate AutoAcquire Stealthed matrix / W3D model draw.
pub fn honesty_pathfinder_residual_pack_wave81() -> bool {
    PATHFINDER_LOCOMOTOR == "ColonelBurtonGroundLocomotor"
        && (PATHFINDER_LOCOMOTOR_SPEED - 30.0).abs() < 0.01
        && (PATHFINDER_LOCOMOTOR_SPEED_DAMAGED - 20.0).abs() < 0.01
        && (PATHFINDER_LOCOMOTOR_TURN_RATE_DEG - 500.0).abs() < 0.01
        && (PATHFINDER_LOCOMOTOR_ACCEL - 100.0).abs() < 0.01
        && (PATHFINDER_LOCOMOTOR_ACCEL_DAMAGED - 50.0).abs() < 0.01
        && PATHFINDER_LOCOMOTOR_SPEED > PATHFINDER_LOCOMOTOR_SPEED_DAMAGED
        && PATHFINDER_ARMOR == "HumanArmor"
        && PATHFINDER_ARMOR_CHEM_SUIT == "ChemSuitHumanArmor"
        && PATHFINDER_DAMAGE_FX == "InfantryDamageFX"
        && (PATHFINDER_BUILD_TIME_SEC - 10.0).abs() < 0.01
        && PATHFINDER_EXPERIENCE_VALUE == [40, 40, 60, 80]
        && PATHFINDER_EXPERIENCE_REQUIRED == [0, 50, 100, 200]
        && PATHFINDER_IS_TRAINABLE
        && PATHFINDER_CRUSHABLE_LEVEL == 0
        && PATHFINDER_TRANSPORT_SLOT_COUNT == 1
        && PATHFINDER_HEAL_SCAN_RATE_MS == 1_000
        && PATHFINDER_HEAL_SCAN_RATE_FRAMES
            == pathfinder_ms_to_frames(PATHFINDER_HEAL_SCAN_RATE_MS)
        && (PATHFINDER_HEAL_SCAN_RANGE - 300.0).abs() < 0.01
        && (PATHFINDER_HEAL_NEVER_FRACTION - 0.85).abs() < 0.001
        && (PATHFINDER_HEAL_ALWAYS_FRACTION - 0.25).abs() < 0.001
        && PATHFINDER_HEAL_ALWAYS_FRACTION < PATHFINDER_HEAL_NEVER_FRACTION
        && PATHFINDER_MOOD_ATTACK_CHECK_RATE_MS == 250
        && PATHFINDER_MOOD_ATTACK_CHECK_RATE_FRAMES
            == pathfinder_ms_to_frames(PATHFINDER_MOOD_ATTACK_CHECK_RATE_MS)
        && (PATHFINDER_PHYSICS_MASS - 5.0).abs() < 0.01
        // Wave 54 base pack still holds under deepen.
        && honesty_pathfinder_residual_pack_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pathfinder_name_matrix() {
        assert!(is_pathfinder_template("AmericaInfantryPathfinder"));
        assert!(is_pathfinder_template("USA_Pathfinder"));
        assert!(is_pathfinder_template("AirF_AmericaInfantryPathfinder"));
        assert!(is_pathfinder_template("SupW_AmericaInfantryPathfinder"));
        assert!(is_pathfinder_template("TestPathfinder"));
        assert!(!is_pathfinder_template("USA_Ranger"));
        assert!(!is_pathfinder_template("USAPathfinderSniperRifle"));
        assert!(!is_pathfinder_template("SciencePathfinder"));
        assert!(!is_pathfinder_template("AmericaVehicleSentryDrone"));
    }

    #[test]
    fn pathfinder_detect_and_stealth_desired() {
        assert!(pathfinder_spawn_is_detector("AmericaInfantryPathfinder"));
        assert_eq!(
            pathfinder_detection_range("AmericaInfantryPathfinder"),
            Some(PATHFINDER_DETECTION_RANGE)
        );
        assert_eq!(
            pathfinder_stealth_desired(true, true, true, true, true),
            Some(false),
            "moving pathfinder uncloaks"
        );
        assert_eq!(
            pathfinder_stealth_desired(true, true, true, true, false),
            Some(true),
            "idle pathfinder re-cloaks"
        );
        assert_eq!(
            pathfinder_stealth_desired(false, true, true, true, false),
            None
        );
    }

    #[test]
    fn pathfinder_stealth_while_attacking_residual() {
        assert_eq!(
            pathfinder_stealth_while_attacking(true, true, true, true, false),
            Some(true),
            "attacking idle stays stealthed"
        );
        assert_eq!(
            pathfinder_stealth_while_attacking(true, true, true, true, true),
            Some(false),
            "attacking while moving uncloaks"
        );
        assert!(!PATHFINDER_STEALTH_BREAKS_ON_ATTACK);
        assert!(PATHFINDER_STEALTH_BREAKS_ON_MOVE);
    }

    #[test]
    fn pathfinder_residual_pack_honesty() {
        assert!(honesty_pathfinder_residual_pack_ok());
        assert_eq!(pathfinder_ms_to_frames(2_000), 60);
        assert_eq!(pathfinder_ms_to_frames(500), 15);
        assert_eq!(pathfinder_ms_to_frames(0), 0);
        assert!((pathfinder_sniper_damage(false) - 100.0).abs() < 0.01);
        assert!((pathfinder_sniper_damage(true) - 125.0).abs() < 0.01);
    }

    #[test]
    fn pathfinder_residual_pack_wave81_honesty() {
        assert!(honesty_pathfinder_residual_pack_wave81());
        assert_eq!(PATHFINDER_LOCOMOTOR, "ColonelBurtonGroundLocomotor");
        assert_eq!(PATHFINDER_ARMOR, "HumanArmor");
        assert_eq!(pathfinder_ms_to_frames(1_000), 30);
        assert_eq!(pathfinder_ms_to_frames(250), 8);
    }

    #[test]
    fn pathfinder_science_gate() {
        assert!(is_pathfinder_science_name("SCIENCE_Pathfinder"));
        assert!(pathfinder_spawn_requires_science(true));
        assert!(!pathfinder_spawn_requires_science(false));
        assert!(!PATHFINDER_HAS_PACK_UNPACK);
    }
}
