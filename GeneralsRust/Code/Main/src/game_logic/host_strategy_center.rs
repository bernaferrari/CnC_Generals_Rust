//! Host USA Strategy Center battle-plan residual
//! (Bombardment / HoldTheLine / SearchAndDestroy).
//!
//! Residual slice (playability):
//! - `AmericaStrategyCenter` / *StrategyCenter selects a battle plan via
//!   `SpecialAbilityChangeBattlePlans` residual (`BattlePlanUpdate`).
//! - Selecting a plan applies army-wide residual bonuses to same-team
//!   legal members (ValidMemberKindOf = INFANTRY | CAN_ATTACK | VEHICLE;
//!   InvalidMemberKindOf = DOZER | STRUCTURE | AIRCRAFT | DRONE):
//!   - **Bombardment**: WeaponBonus BATTLEPLAN_BOMBARDMENT DAMAGE **120%**
//!   - **HoldTheLine**: HoldTheLinePlanArmorDamageScalar **0.9** (take 90% damage)
//!   - **SearchAndDestroy**: WeaponBonus RANGE **120%** + SightRangeScalar **1.2**
//! - Strategy Center building residuals (when plan active on the center):
//!   - HoldTheLine max-health scalar **2.0** (PRESERVE_RATIO residual)
//!   - SearchAndDestroy building sight scalar **2.0** + stealth detect residual
//!
//! - **BattlePlanChangeParalyzeTime residual**: on plan change (including first
//!   residual select), legal army members receive DISABLED_PARALYZED for
//!   **5000 ms → 150 frames** (retail BattlePlanChangeParalyzeTime).
//!
//! - **Bombardment turret residual**: when Bombardment plan is active, Strategy
//!   Center PRIMARY `StrategyCenterGun` residual auto-fires
//!   (PrimaryDamage **200** / r**25**, range **400**, min **100**,
//!   Delay **7000**ms → 210 frames). C++ `enableTurret(true)` residual path.
//!
//! - **StealthDetectorUpdate residual** (Strategy Center ModuleTag_16):
//!   DetectionRange **500**, DetectionRate **500**ms → **15** frames,
//!   InitiallyDisabled **Yes**. Enabled only while SearchAndDestroy is active
//!   (`setSDEnabled(true)` residual). DetectionRate residual: first scan
//!   immediate, then sleep **15** frames; `markAsDetected(rate+1=**16**)`.
//!   VisionObjectName createVisionObject is disabled in retail C++
//!   (ShroudRevealToAllRange path) — fail-closed.
//!
//! - **Pack/unpack door model-condition residual**: retail AnimationTime
//!   **7000**ms → **210** frames for all three plans; TransitionIdleTime **0**.
//!   Host tracks door residual OPENING → WAITING_TO_CLOSE → CLOSING per plan
//!   (DOOR_1 Bombardment / DOOR_2 HoldTheLine / DOOR_3 SearchAndDestroy).
//!
//! - **Delayed ACTIVE-after-unpack setBattlePlan residual**: army buffs,
//!   building bonuses, StealthDetector enable, and Bombardment turret equip
//!   apply only when door residual reaches ACTIVE (WAITING_TO_CLOSE). Plan
//!   switch begins PACKING → `setBattlePlan(NONE)` clears effects + paralyzes
//!   troops; new plan applies after pack+unpack completes.
//!
//! - **Bombardment turret recenter residual**: when leaving Bombardment while
//!   the gun residual is not "natural" (attacking / has target / recently
//!   fired / angles off natural), host delays pack by **TurretRecenterFrames**
//!   (or angle-based frames) before CLOSING.
//!
//! - **Turret natural-position pitch/yaw residual** (AIUpdateInterface Turret):
//!   NaturalTurretAngle **-90**, NaturalTurretPitch **45**, Turn/Pitch rate
//!   **60** deg/s → **2** deg/frame, FirePitch **45**. Fire aims residual
//!   angles at target; recenter steps toward natural each frame.
//!
//! - **TurretAI idle-scan residual** (Bombardment ACTIVE idle gun):
//!   MinIdleScanInterval **500**ms → **15** frames, MaxIdleScanInterval
//!   **1000**ms → **30** frames, MinIdleScanAngle **0**, MaxIdleScanAngle
//!   **60** deg off NaturalTurretAngle. Idle residual schedules scan, rotates
//!   toward natural ± offset. Deterministic host residual
//!   (alternate min/max interval + signed mid/max offset by scan index).
//!
//! - **TurretAI HoldTurret + idle-recenter residual**: after idle-scan completes,
//!   HOLD for RecenterTime (default **2** logic seconds → **60** frames; Strategy
//!   Center does not override RecenterTime) then RECENTER to natural pitch/yaw
//!   before scheduling the next idle scan (C++ IDLESCAN → HOLD → RECENTER → IDLE).
//!
//! Fail-closed honesty:
//! - Not full TurretAI mood-target scan / bone pitch matrix
//! - Not full VisionObjectName spawn residual (createVisionObject disabled retail)
//! - Not full ScatterRadius / ScaleWeaponSpeed artillery lob matrix
//! - Not network battle-plan replication (network deferred)

use super::{ObjectId, Weapon};
use serde::{Deserialize, Serialize};

/// Retail HoldTheLinePlanArmorDamageScalar (LESS is better — damage taken mult).
pub const HOLD_THE_LINE_ARMOR_DAMAGE_SCALAR: f32 = 0.9;

/// Retail SearchAndDestroyPlanSightRangeScalar.
pub const SEARCH_AND_DESTROY_SIGHT_RANGE_SCALAR: f32 = 1.2;

/// Retail GameData.ini WeaponBonus BATTLEPLAN_BOMBARDMENT DAMAGE 120%.
pub const BOMBARDMENT_DAMAGE_MULT: f32 = 1.20;

/// Retail GameData.ini WeaponBonus BATTLEPLAN_SEARCHANDDESTROY RANGE 120%.
pub const SEARCH_AND_DESTROY_RANGE_MULT: f32 = 1.20;

/// Retail StrategyCenterHoldTheLineMaxHealthScalar.
pub const STRATEGY_CENTER_HOLD_THE_LINE_MAX_HEALTH_SCALAR: f32 = 2.0;

/// Retail StrategyCenterSearchAndDestroySightRangeScalar.
pub const STRATEGY_CENTER_SEARCH_AND_DESTROY_SIGHT_SCALAR: f32 = 2.0;

/// Retail BattlePlanChangeParalyzeTime (ms).
pub const BATTLE_PLAN_PARALYZE_TIME_MS: u32 = 5000;

/// Logic frames per second (host fixed step).
pub const BATTLE_PLAN_LOGIC_FPS: f32 = 30.0;

/// Retail BattlePlanChangeParalyzeTime → frames at 30 FPS (5000 / (1000/30) = 150).
pub const BATTLE_PLAN_PARALYZE_FRAMES: u32 = 150;

/// Residual announcement audio (pack/unpack sounds fail-closed).
pub const BATTLE_PLAN_BOMBARDMENT_AUDIO: &str = "StrategyCenter_BombardmentPlanAnnouncement";
pub const BATTLE_PLAN_HOLD_THE_LINE_AUDIO: &str = "StrategyCenter_HoldTheLineAnnouncement";
pub const BATTLE_PLAN_SEARCH_AND_DESTROY_AUDIO: &str =
    "StrategyCenter_SearchAndDestroyAnnouncement";

// --- StrategyCenterGun Bombardment turret residual ---
/// Retail Strategy Center PRIMARY weapon.
pub const STRATEGY_CENTER_GUN_WEAPON: &str = "StrategyCenterGun";
/// Retail StrategyCenterGun PrimaryDamage.
pub const STRATEGY_CENTER_GUN_DAMAGE: f32 = 200.0;
/// Retail StrategyCenterGun PrimaryDamageRadius.
pub const STRATEGY_CENTER_GUN_PRIMARY_RADIUS: f32 = 25.0;
/// Retail StrategyCenterGun AttackRange.
pub const STRATEGY_CENTER_GUN_RANGE: f32 = 400.0;
/// Retail StrategyCenterGun MinimumAttackRange.
pub const STRATEGY_CENTER_GUN_MIN_RANGE: f32 = 100.0;
/// Retail StrategyCenterGun DelayBetweenShots 7000ms → 210 frames @ 30 FPS.
pub const STRATEGY_CENTER_GUN_DELAY_FRAMES: u32 = 210;
/// Residual projectile speed (WeaponSpeed 150 residual honesty).
pub const STRATEGY_CENTER_GUN_PROJECTILE_SPEED: f32 = 150.0;
/// Residual fire audio (retail FireSound).
pub const STRATEGY_CENTER_GUN_FIRE_AUDIO: &str = "StrategyCenter_ArtilleryRound";

// --- StealthDetectorUpdate residual (Strategy Center ModuleTag_16) ---

/// Retail StealthDetectorUpdate DetectionRange for Strategy Center.
pub const STRATEGY_CENTER_STEALTH_DETECTION_RANGE: f32 = 500.0;

/// Retail StealthDetectorUpdate DetectionRate (ms).
pub const STRATEGY_CENTER_STEALTH_DETECTION_RATE_MS: u32 = 500;

/// DetectionRate → frames at 30 FPS (500 / (1000/30) = 15).
pub const STRATEGY_CENTER_STEALTH_DETECTION_RATE_FRAMES: u32 = 15;

/// C++ `markAsDetected(updateRate + 1)` residual hold frames for Strategy Center.
/// Ensures detected status survives until the next DetectionRate wake.
pub const STRATEGY_CENTER_STEALTH_DETECTION_HOLD_FRAMES: u32 =
    STRATEGY_CENTER_STEALTH_DETECTION_RATE_FRAMES + 1;

/// Retail StrategyCenterSearchAndDestroyDetectsStealth.
pub const STRATEGY_CENTER_SEARCH_AND_DESTROY_DETECTS_STEALTH: bool = true;

/// Whether a StealthDetectorUpdate residual scan is due this frame.
///
/// C++ `setSDEnabled(true)` wakes with `UPDATE_SLEEP_NONE` (immediate first scan);
/// subsequent wakes sleep `DetectionRate` frames. Host residual: `next_scan_frame == 0`
/// or `frame >= next_scan_frame` is due. `rate_frames == 0` means continuous (legacy).
pub fn stealth_detector_scan_due(rate_frames: u32, next_scan_frame: u32, frame: u32) -> bool {
    if rate_frames == 0 {
        return true;
    }
    next_scan_frame == 0 || frame >= next_scan_frame
}

/// Absolute frame for the next DetectionRate residual scan after a scan at `frame`.
pub fn stealth_detector_next_scan_frame(rate_frames: u32, frame: u32) -> u32 {
    if rate_frames == 0 {
        return 0;
    }
    frame.saturating_add(rate_frames)
}

/// Detected-status hold frames residual: `markAsDetected(updateRate + 1)`.
///
/// When `rate_frames == 0` (legacy continuous detectors), host uses **30** frames
/// (~1 logic second) for fail-closed compatibility with non-rate residual detectors.
pub fn stealth_detector_hold_frames(rate_frames: u32) -> u32 {
    if rate_frames == 0 {
        30
    } else {
        rate_frames.saturating_add(1)
    }
}

// --- Pack/unpack door animation residual (BattlePlanUpdate) ---

/// Retail BombardmentPlanAnimationTime / HoldTheLine / SearchAndDestroy (ms).
pub const BATTLE_PLAN_ANIMATION_TIME_MS: u32 = 7000;

/// AnimationTime → frames at 30 FPS (7000 / (1000/30) = 210).
pub const BATTLE_PLAN_ANIMATION_FRAMES: u32 = 210;

/// Retail TransitionIdleTime (ms → frames; retail = 0).
pub const BATTLE_PLAN_TRANSITION_IDLE_FRAMES: u32 = 0;

/// Host residual Bombardment turret recenter wait before pack (1 second @ 30 FPS).
///
/// C++ waits for `isTurretInNaturalPosition` after `recenterTurret`. Host uses
/// angle-based frames when pitch/yaw are off natural; busy-only residual
/// (attacking / target / recent fire with natural angles) falls back to this.
pub const BATTLE_PLAN_TURRET_RECENTER_FRAMES: u32 = 30;

// --- Strategy Center Turret natural-position pitch/yaw residual ---

/// Retail NaturalTurretAngle (deg) — turret points backwards normally.
pub const STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG: f32 = -90.0;
/// Retail NaturalTurretPitch (deg) — half way between land and sky.
pub const STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG: f32 = 45.0;
/// Retail FirePitch (deg) — aim pitch when firing residual.
pub const STRATEGY_CENTER_FIRE_PITCH_DEG: f32 = 45.0;
/// Retail TurretTurnRate (deg/sec).
pub const STRATEGY_CENTER_TURRET_TURN_RATE_DEG_PER_SEC: f32 = 60.0;
/// Retail TurretPitchRate (deg/sec).
pub const STRATEGY_CENTER_TURRET_PITCH_RATE_DEG_PER_SEC: f32 = 60.0;
/// Turn rate residual → deg per logic frame @ 30 FPS.
pub const STRATEGY_CENTER_TURRET_TURN_DEG_PER_FRAME: f32 =
    STRATEGY_CENTER_TURRET_TURN_RATE_DEG_PER_SEC / BATTLE_PLAN_LOGIC_FPS;
/// Pitch rate residual → deg per logic frame @ 30 FPS.
pub const STRATEGY_CENTER_TURRET_PITCH_DEG_PER_FRAME: f32 =
    STRATEGY_CENTER_TURRET_PITCH_RATE_DEG_PER_SEC / BATTLE_PLAN_LOGIC_FPS;
/// Angle equality residual epsilon (deg).
pub const STRATEGY_CENTER_TURRET_ANGLE_EPS_DEG: f32 = 0.05;

// --- TurretAI idle-scan residual (Strategy Center AIUpdateInterface Turret) ---

/// Retail MinIdleScanInterval 500ms → 15 frames @ 30 FPS.
pub const STRATEGY_CENTER_MIN_IDLE_SCAN_INTERVAL_FRAMES: u32 = 15;
/// Retail MaxIdleScanInterval 1000ms → 30 frames @ 30 FPS.
pub const STRATEGY_CENTER_MAX_IDLE_SCAN_INTERVAL_FRAMES: u32 = 30;
/// Retail MinIdleScanAngle (deg off natural).
pub const STRATEGY_CENTER_MIN_IDLE_SCAN_ANGLE_DEG: f32 = 0.0;
/// Retail MaxIdleScanAngle (deg off natural).
pub const STRATEGY_CENTER_MAX_IDLE_SCAN_ANGLE_DEG: f32 = 60.0;

/// Default TurretAI RecenterTime residual (C++ `2 * LOGICFRAMES_PER_SECOND`).
///
/// Strategy Center Turret block does not set RecenterTime, so HoldTurret waits
/// **60** frames after idle-scan (or fire exit to HOLD) before RECENTER.
pub const STRATEGY_CENTER_RECENTER_TIME_FRAMES: u32 = 60;

/// Absolute frame when a HoldTurret residual started at `frame` should end.
pub fn hold_turret_until_frame(frame: u32) -> u32 {
    frame.saturating_add(STRATEGY_CENTER_RECENTER_TIME_FRAMES)
}

/// Whether HoldTurret residual has elapsed (`frame >= hold_until`).
pub fn hold_turret_elapsed(frame: u32, hold_until_frame: u32) -> bool {
    hold_until_frame > 0 && frame >= hold_until_frame
}

/// Deterministic residual idle-scan interval for scan index `n`.
///
/// C++ `GameLogicRandomValue(min, max)` residual; host alternates min/max.
pub fn idle_scan_interval_frames(scan_index: u32) -> u32 {
    if scan_index % 2 == 0 {
        STRATEGY_CENTER_MIN_IDLE_SCAN_INTERVAL_FRAMES
    } else {
        STRATEGY_CENTER_MAX_IDLE_SCAN_INTERVAL_FRAMES
    }
}

/// Deterministic residual idle-scan angle offset (deg off natural) for scan index `n`.
///
/// C++ picks `minA + rand(0, maxA-minA)` then random sign. Host residual uses
/// mid-span angle (**30**) with alternating sign so both directions exercise.
pub fn idle_scan_desired_offset_deg(scan_index: u32) -> f32 {
    let span = STRATEGY_CENTER_MAX_IDLE_SCAN_ANGLE_DEG - STRATEGY_CENTER_MIN_IDLE_SCAN_ANGLE_DEG;
    let mid = STRATEGY_CENTER_MIN_IDLE_SCAN_ANGLE_DEG + span * 0.5;
    if scan_index % 2 == 0 {
        mid
    } else {
        -mid
    }
}

/// Absolute residual idle-scan target yaw = NaturalTurretAngle + offset.
pub fn idle_scan_desired_angle_deg(scan_index: u32) -> f32 {
    normalize_angle_deg(
        STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG + idle_scan_desired_offset_deg(scan_index),
    )
}

/// Whether residual turret angles match a target pitch/yaw (eps gate).
pub fn turret_angles_at(
    angle_deg: f32,
    pitch_deg: f32,
    target_angle_deg: f32,
    target_pitch_deg: f32,
) -> bool {
    shortest_angle_delta_deg(angle_deg, target_angle_deg).abs()
        <= STRATEGY_CENTER_TURRET_ANGLE_EPS_DEG
        && (pitch_deg - target_pitch_deg).abs() <= STRATEGY_CENTER_TURRET_ANGLE_EPS_DEG
}

/// Step residual turret angles one frame toward an arbitrary pitch/yaw target.
///
/// Shared by recenter (natural) and idle-scan residual rotation.
pub fn step_turret_toward_angles(
    angle_deg: f32,
    pitch_deg: f32,
    target_angle_deg: f32,
    target_pitch_deg: f32,
) -> (f32, f32) {
    let da = shortest_angle_delta_deg(angle_deg, target_angle_deg);
    let step_a = da
        .abs()
        .min(STRATEGY_CENTER_TURRET_TURN_DEG_PER_FRAME)
        * da.signum();
    let new_angle = if da.abs() <= STRATEGY_CENTER_TURRET_ANGLE_EPS_DEG {
        normalize_angle_deg(target_angle_deg)
    } else {
        normalize_angle_deg(angle_deg + step_a)
    };

    let dp = target_pitch_deg - pitch_deg;
    let step_p = dp
        .abs()
        .min(STRATEGY_CENTER_TURRET_PITCH_DEG_PER_FRAME)
        * dp.signum();
    let new_pitch = if dp.abs() <= STRATEGY_CENTER_TURRET_ANGLE_EPS_DEG {
        target_pitch_deg
    } else {
        pitch_deg + step_p
    };
    (new_angle, new_pitch)
}

/// Residual pack/unpack audio event names (retail *PlanPack/UnpackSoundName).
pub const BATTLE_PLAN_BOMBARDMENT_UNPACK_AUDIO: &str = "StrategyCenter_BombardmentPlanUnpackSound";
pub const BATTLE_PLAN_BOMBARDMENT_PACK_AUDIO: &str = "StrategyCenter_BombardmentPlanPackSound";
pub const BATTLE_PLAN_HOLD_THE_LINE_UNPACK_AUDIO: &str = "StrategyCenter_HoldTheLinePlanUnpack";
pub const BATTLE_PLAN_HOLD_THE_LINE_PACK_AUDIO: &str = "StrategyCenter_HoldTheLinePlanPack";
pub const BATTLE_PLAN_SEARCH_AND_DESTROY_UNPACK_AUDIO: &str =
    "StrategyCenter_SearchAndDestroyPlanUnpack";
pub const BATTLE_PLAN_SEARCH_AND_DESTROY_PACK_AUDIO: &str =
    "StrategyCenter_SearchAndDestroyPlanPack";
pub const BATTLE_PLAN_SEARCH_AND_DESTROY_IDLE_AUDIO: &str =
    "StrategyCenter_SearchAndDestroyPlanIdleLoop";

/// C++ TransitionStatus residual for BattlePlanUpdate door matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum HostBattlePlanTransition {
    /// TRANSITIONSTATUS_IDLE — no plan / between pack and unpack.
    #[default]
    Idle = 0,
    /// TRANSITIONSTATUS_UNPACKING — door OPENING residual (AnimationTime frames).
    Unpacking = 1,
    /// TRANSITIONSTATUS_ACTIVE — door WAITING_TO_CLOSE residual.
    Active = 2,
    /// TRANSITIONSTATUS_PACKING — door CLOSING residual (AnimationTime frames).
    Packing = 3,
}

/// C++ MODELCONDITION_DOOR_* residual for Strategy Center battle-plan doors.
///
/// DOOR_1 = Bombardment, DOOR_2 = HoldTheLine, DOOR_3 = SearchAndDestroy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum HostBattlePlanDoor {
    #[default]
    None = 0,
    Door1Opening,
    Door1WaitingToClose,
    Door1Closing,
    Door2Opening,
    Door2WaitingToClose,
    Door2Closing,
    Door3Opening,
    Door3WaitingToClose,
    Door3Closing,
}

/// Absolute frame when pack/unpack animation residual completes.
pub fn battle_plan_animation_ready_frame(current_frame: u32) -> u32 {
    current_frame.saturating_add(BATTLE_PLAN_ANIMATION_FRAMES.max(1))
}

/// Convert AnimationTime ms → logic frames (30 FPS residual).
pub fn battle_plan_animation_frames_from_ms(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / BATTLE_PLAN_LOGIC_FPS)).round() as u32
}

/// Whether residual StealthDetectorUpdate should be enabled for a plan.
pub fn strategy_center_stealth_detector_enabled_for_plan(plan: HostBattlePlan) -> bool {
    STRATEGY_CENTER_SEARCH_AND_DESTROY_DETECTS_STEALTH
        && matches!(plan, HostBattlePlan::SearchAndDestroy)
}

/// Host residual StealthDetector enable: DetectionRange when S&D enables module.
pub fn strategy_center_stealth_detection_range_when_enabled() -> f32 {
    STRATEGY_CENTER_STEALTH_DETECTION_RANGE
}

/// Door residual lifecycle event for delayed setBattlePlan / audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostBattlePlanDoorEvent {
    /// TRANSITIONSTATUS_ACTIVE — apply setBattlePlan(plan) residual.
    BecameActive {
        center_id: ObjectId,
        player_id: u32,
        plan: HostBattlePlan,
    },
    /// TRANSITIONSTATUS_PACKING — apply setBattlePlan(NONE) + paralyze residual.
    BeganPacking {
        center_id: ObjectId,
        player_id: u32,
    },
    /// Pack/unpack audio residual to queue.
    Audio {
        center_id: ObjectId,
        event: &'static str,
    },
    /// Bombardment turret recenter residual started (pack deferred).
    BeganRecenter {
        center_id: ObjectId,
        player_id: u32,
    },
}

/// Shortest signed delta from `from` to `to` in degrees (−180, 180].
pub fn shortest_angle_delta_deg(from: f32, to: f32) -> f32 {
    let mut d = to - from;
    while d > 180.0 {
        d -= 360.0;
    }
    while d <= -180.0 {
        d += 360.0;
    }
    d
}

/// Normalize angle to (−180, 180].
pub fn normalize_angle_deg(angle: f32) -> f32 {
    let mut a = angle % 360.0;
    if a > 180.0 {
        a -= 360.0;
    } else if a <= -180.0 {
        a += 360.0;
    }
    a
}

/// Whether residual turret pitch/yaw match NaturalTurretAngle / NaturalTurretPitch.
pub fn turret_angles_are_natural(angle_deg: f32, pitch_deg: f32) -> bool {
    shortest_angle_delta_deg(angle_deg, STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG).abs()
        <= STRATEGY_CENTER_TURRET_ANGLE_EPS_DEG
        && (pitch_deg - STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG).abs()
            <= STRATEGY_CENTER_TURRET_ANGLE_EPS_DEG
}

/// Step residual turret angles one frame toward natural (recenterTurret residual).
pub fn step_turret_toward_natural(angle_deg: f32, pitch_deg: f32) -> (f32, f32) {
    step_turret_toward_angles(
        angle_deg,
        pitch_deg,
        STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG,
        STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG,
    )
}

/// Frames needed to recenter residual angles at TurretTurn/PitchRate.
pub fn turret_recenter_frames_for_angles(angle_deg: f32, pitch_deg: f32) -> u32 {
    let da = shortest_angle_delta_deg(angle_deg, STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG).abs();
    let dp = (pitch_deg - STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG).abs();
    let fa = if da <= STRATEGY_CENTER_TURRET_ANGLE_EPS_DEG {
        0
    } else {
        (da / STRATEGY_CENTER_TURRET_TURN_DEG_PER_FRAME).ceil() as u32
    };
    let fp = if dp <= STRATEGY_CENTER_TURRET_ANGLE_EPS_DEG {
        0
    } else {
        (dp / STRATEGY_CENTER_TURRET_PITCH_DEG_PER_FRAME).ceil() as u32
    };
    fa.max(fp).max(1)
}

/// Residual aim angles when StrategyCenterGun fires at a world target.
///
/// Angle = atan2(dx, dz) degrees; pitch = FirePitch **45**.
pub fn strategy_center_turret_aim_at(
    center_x: f32,
    center_z: f32,
    target_x: f32,
    target_z: f32,
) -> (f32, f32) {
    let dx = target_x - center_x;
    let dz = target_z - center_z;
    let angle = normalize_angle_deg(dx.atan2(dz).to_degrees());
    (angle, STRATEGY_CENTER_FIRE_PITCH_DEG)
}

/// Whether Bombardment turret residual is in natural position for pack.
///
/// C++ `isTurretInNaturalPosition`: NaturalTurretAngle/Pitch equality.
/// Host residual also treats busy gun (attacking / target / recent fire) as
/// non-natural so pack waits for recenter (fail-closed busy coast).
pub fn strategy_center_turret_is_natural(
    is_attacking: bool,
    has_target: bool,
    last_fire_age_frames: Option<u32>,
) -> bool {
    strategy_center_turret_is_natural_with_angles(
        is_attacking,
        has_target,
        last_fire_age_frames,
        STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG,
        STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG,
    )
}

/// Natural-position residual with explicit pitch/yaw (C++ TurretAI angles).
pub fn strategy_center_turret_is_natural_with_angles(
    is_attacking: bool,
    has_target: bool,
    last_fire_age_frames: Option<u32>,
    angle_deg: f32,
    pitch_deg: f32,
) -> bool {
    if is_attacking || has_target {
        return false;
    }
    // Recently fired residual: within recenter window frames.
    if let Some(age) = last_fire_age_frames {
        if age < BATTLE_PLAN_TURRET_RECENTER_FRAMES {
            return false;
        }
    }
    turret_angles_are_natural(angle_deg, pitch_deg)
}

/// Frames for recenter residual: angle-based, or busy-coast fallback.
pub fn strategy_center_turret_recenter_frames(
    is_busy_non_natural: bool,
    angle_deg: f32,
    pitch_deg: f32,
) -> u32 {
    if turret_angles_are_natural(angle_deg, pitch_deg) {
        // Busy gate only (attacking/target/recent fire) — fixed coast residual.
        if is_busy_non_natural {
            BATTLE_PLAN_TURRET_RECENTER_FRAMES
        } else {
            1
        }
    } else {
        turret_recenter_frames_for_angles(angle_deg, pitch_deg)
    }
}

/// Per-Strategy-Center pack/unpack door residual bookkeeping.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostBattlePlanDoorState {
    pub center_id: ObjectId,
    pub player_id: u32,
    pub status: HostBattlePlanTransition,
    /// Plan whose door residual is currently animating / active.
    pub door_plan: Option<HostBattlePlan>,
    pub door: HostBattlePlanDoor,
    /// Frame when current OPENING/CLOSING/recenter residual completes.
    pub next_ready_frame: u32,
    /// Desired plan waiting after pack residual (plan switch residual).
    pub desired_plan: Option<HostBattlePlan>,
    /// Bombardment turret recenter residual in progress before pack.
    #[serde(default)]
    pub centering_turret: bool,
}

impl HostBattlePlanDoorState {
    pub fn new(center_id: ObjectId, player_id: u32) -> Self {
        Self {
            center_id,
            player_id,
            status: HostBattlePlanTransition::Idle,
            door_plan: None,
            door: HostBattlePlanDoor::None,
            next_ready_frame: 0,
            desired_plan: None,
            centering_turret: false,
        }
    }

    /// Begin UNPACKING residual for `plan` at `frame` (door OPENING).
    pub fn begin_unpack(&mut self, plan: HostBattlePlan, frame: u32) {
        self.status = HostBattlePlanTransition::Unpacking;
        self.door_plan = Some(plan);
        self.door = plan.door_opening();
        self.next_ready_frame = battle_plan_animation_ready_frame(frame);
        self.desired_plan = Some(plan);
        self.centering_turret = false;
    }

    /// Begin PACKING residual for current door plan at `frame` (door CLOSING).
    ///
    /// Returns true when packing actually started (caller should emit BeganPacking).
    pub fn begin_pack(&mut self, frame: u32, desired: Option<HostBattlePlan>) -> bool {
        self.centering_turret = false;
        if let Some(plan) = self.door_plan {
            self.status = HostBattlePlanTransition::Packing;
            self.door = plan.door_closing();
            self.next_ready_frame = battle_plan_animation_ready_frame(frame);
            self.desired_plan = desired;
            true
        } else {
            // No prior door residual — go idle then unpack desired.
            self.status = HostBattlePlanTransition::Idle;
            self.door = HostBattlePlanDoor::None;
            self.next_ready_frame = frame.saturating_add(BATTLE_PLAN_TRANSITION_IDLE_FRAMES);
            self.desired_plan = desired;
            false
        }
    }

    /// Begin Bombardment turret recenter residual (pack deferred).
    ///
    /// `frames` is angle-based or busy-coast residual (see
    /// `strategy_center_turret_recenter_frames`).
    pub fn begin_recenter(&mut self, frame: u32, desired: Option<HostBattlePlan>, frames: u32) {
        self.centering_turret = true;
        self.desired_plan = desired;
        self.next_ready_frame = frame.saturating_add(frames.max(1));
        // Stay ACTIVE / WAITING_TO_CLOSE while recentering.
        self.status = HostBattlePlanTransition::Active;
    }

    /// Advance door residual when `frame >= next_ready_frame`.
    ///
    /// Returns residual events (BecameActive / BeganPacking / Audio).
    pub fn tick(&mut self, frame: u32) -> Vec<HostBattlePlanDoorEvent> {
        if frame < self.next_ready_frame {
            return Vec::new();
        }
        let mut events = Vec::new();
        match self.status {
            HostBattlePlanTransition::Unpacking => {
                // OPENING complete → ACTIVE / WAITING_TO_CLOSE + setBattlePlan(plan).
                if let Some(plan) = self.door_plan {
                    self.status = HostBattlePlanTransition::Active;
                    self.door = plan.door_waiting();
                    self.next_ready_frame = frame; // stay active until pack
                    self.centering_turret = false;
                    events.push(HostBattlePlanDoorEvent::BecameActive {
                        center_id: self.center_id,
                        player_id: self.player_id,
                        plan,
                    });
                }
            }
            HostBattlePlanTransition::Packing => {
                // CLOSING complete → IDLE (TransitionIdleTime = 0).
                self.status = HostBattlePlanTransition::Idle;
                self.door = HostBattlePlanDoor::None;
                self.door_plan = None;
                self.next_ready_frame =
                    frame.saturating_add(BATTLE_PLAN_TRANSITION_IDLE_FRAMES);
                // Immediately start unpack of desired if present (idle time 0).
                if let Some(desired) = self.desired_plan {
                    let audio = desired.unpack_audio();
                    self.begin_unpack(desired, frame);
                    events.push(HostBattlePlanDoorEvent::Audio {
                        center_id: self.center_id,
                        event: audio,
                    });
                }
            }
            HostBattlePlanTransition::Idle => {
                if let Some(desired) = self.desired_plan {
                    let audio = desired.unpack_audio();
                    self.begin_unpack(desired, frame);
                    events.push(HostBattlePlanDoorEvent::Audio {
                        center_id: self.center_id,
                        event: audio,
                    });
                }
            }
            HostBattlePlanTransition::Active => {
                // Recenter complete → begin PACKING residual.
                if self.centering_turret {
                    let desired = self.desired_plan;
                    let pack_audio = self.door_plan.map(|p| p.pack_audio());
                    if self.begin_pack(frame, desired) {
                        events.push(HostBattlePlanDoorEvent::BeganPacking {
                            center_id: self.center_id,
                            player_id: self.player_id,
                        });
                        if let Some(event) = pack_audio {
                            events.push(HostBattlePlanDoorEvent::Audio {
                                center_id: self.center_id,
                                event,
                            });
                        }
                    }
                }
            }
        }
        events
    }
}

/// Reload time seconds residual for delay frames @ 30 FPS.
pub fn strategy_center_gun_reload_secs() -> f32 {
    (STRATEGY_CENTER_GUN_DELAY_FRAMES.max(1) as f32) / 30.0
}

/// Build residual StrategyCenterGun Weapon (Bombardment turret residual).
pub fn strategy_center_gun_weapon() -> Weapon {
    Weapon {
        damage: STRATEGY_CENTER_GUN_DAMAGE,
        range: STRATEGY_CENTER_GUN_RANGE,
        min_range: STRATEGY_CENTER_GUN_MIN_RANGE,
        reload_time: strategy_center_gun_reload_secs(),
        last_fire_time: 0.0,
        ammo: None,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: STRATEGY_CENTER_GUN_PROJECTILE_SPEED,
        pre_attack_delay: 0.0,
    }
}

/// Residual damage at distance from impact (intended / primary ring).
pub fn strategy_center_gun_damage_at(distance_from_impact: f32) -> f32 {
    if distance_from_impact <= STRATEGY_CENTER_GUN_PRIMARY_RADIUS {
        STRATEGY_CENTER_GUN_DAMAGE
    } else {
        0.0
    }
}

/// Whether residual target is in StrategyCenterGun range band (min..=max).
pub fn strategy_center_gun_in_range(distance: f32) -> bool {
    distance >= STRATEGY_CENTER_GUN_MIN_RANGE && distance <= STRATEGY_CENTER_GUN_RANGE
}

/// Legal residual Strategy Center artillery target.
pub fn is_legal_strategy_center_gun_target(
    is_alive: bool,
    same_team: bool,
    is_neutral: bool,
    under_construction: bool,
    combat_kind: bool,
    is_air: bool,
) -> bool {
    is_alive
        && !same_team
        && !is_neutral
        && !under_construction
        && combat_kind
        && !is_air // ground residual only (can_target_air false)
}

/// Convert BattlePlanChangeParalyzeTime ms → logic frames (30 FPS residual).
pub fn battle_plan_paralyze_frames_from_ms(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / BATTLE_PLAN_LOGIC_FPS)).round() as u32
}

/// Absolute host frame when DISABLED_PARALYZED residual expires.
pub fn battle_plan_paralyze_until_frame(current_frame: u32) -> u32 {
    current_frame.saturating_add(BATTLE_PLAN_PARALYZE_FRAMES.max(1))
}

/// USA Strategy Center battle plan residual kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostBattlePlan {
    /// PLANSTATUS_BOMBARDMENT — army DAMAGE 120% residual.
    Bombardment = 1,
    /// PLANSTATUS_HOLDTHELINE — army armor damage scalar 0.9 residual.
    HoldTheLine = 2,
    /// PLANSTATUS_SEARCHANDDESTROY — army RANGE 120% + sight 1.2 residual.
    SearchAndDestroy = 3,
}

impl HostBattlePlan {
    /// Parse residual plan from 1..=3 (fail-closed: unknown → Bombardment).
    pub fn from_u8(v: u8) -> Self {
        match v {
            2 => HostBattlePlan::HoldTheLine,
            3 => HostBattlePlan::SearchAndDestroy,
            _ => HostBattlePlan::Bombardment,
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Retail WeaponBonus DAMAGE multiplier for army members (1.0 if not Bombardment).
    pub fn army_damage_multiplier(self) -> f32 {
        match self {
            HostBattlePlan::Bombardment => BOMBARDMENT_DAMAGE_MULT,
            _ => 1.0,
        }
    }

    /// Retail armor damage scalar for army members (1.0 if not HoldTheLine).
    /// LESS is better — multiplies incoming damage.
    pub fn army_armor_damage_scalar(self) -> f32 {
        match self {
            HostBattlePlan::HoldTheLine => HOLD_THE_LINE_ARMOR_DAMAGE_SCALAR,
            _ => 1.0,
        }
    }

    /// Retail WeaponBonus RANGE multiplier for army members (1.0 if not S&D).
    pub fn army_range_multiplier(self) -> f32 {
        match self {
            HostBattlePlan::SearchAndDestroy => SEARCH_AND_DESTROY_RANGE_MULT,
            _ => 1.0,
        }
    }

    /// Retail sight-range scalar for army members (1.0 if not S&D).
    pub fn army_sight_range_scalar(self) -> f32 {
        match self {
            HostBattlePlan::SearchAndDestroy => SEARCH_AND_DESTROY_SIGHT_RANGE_SCALAR,
            _ => 1.0,
        }
    }

    /// Residual announcement audio event name.
    pub fn activate_audio(self) -> &'static str {
        match self {
            HostBattlePlan::Bombardment => BATTLE_PLAN_BOMBARDMENT_AUDIO,
            HostBattlePlan::HoldTheLine => BATTLE_PLAN_HOLD_THE_LINE_AUDIO,
            HostBattlePlan::SearchAndDestroy => BATTLE_PLAN_SEARCH_AND_DESTROY_AUDIO,
        }
    }

    /// Door OPENING residual for this plan.
    pub fn door_opening(self) -> HostBattlePlanDoor {
        match self {
            HostBattlePlan::Bombardment => HostBattlePlanDoor::Door1Opening,
            HostBattlePlan::HoldTheLine => HostBattlePlanDoor::Door2Opening,
            HostBattlePlan::SearchAndDestroy => HostBattlePlanDoor::Door3Opening,
        }
    }

    /// Door WAITING_TO_CLOSE residual for this plan (ACTIVE).
    pub fn door_waiting(self) -> HostBattlePlanDoor {
        match self {
            HostBattlePlan::Bombardment => HostBattlePlanDoor::Door1WaitingToClose,
            HostBattlePlan::HoldTheLine => HostBattlePlanDoor::Door2WaitingToClose,
            HostBattlePlan::SearchAndDestroy => HostBattlePlanDoor::Door3WaitingToClose,
        }
    }

    /// Door CLOSING residual for this plan (PACKING).
    pub fn door_closing(self) -> HostBattlePlanDoor {
        match self {
            HostBattlePlan::Bombardment => HostBattlePlanDoor::Door1Closing,
            HostBattlePlan::HoldTheLine => HostBattlePlanDoor::Door2Closing,
            HostBattlePlan::SearchAndDestroy => HostBattlePlanDoor::Door3Closing,
        }
    }

    /// Residual unpack audio for this plan.
    pub fn unpack_audio(self) -> &'static str {
        match self {
            HostBattlePlan::Bombardment => BATTLE_PLAN_BOMBARDMENT_UNPACK_AUDIO,
            HostBattlePlan::HoldTheLine => BATTLE_PLAN_HOLD_THE_LINE_UNPACK_AUDIO,
            HostBattlePlan::SearchAndDestroy => BATTLE_PLAN_SEARCH_AND_DESTROY_UNPACK_AUDIO,
        }
    }

    /// Residual pack audio for this plan.
    pub fn pack_audio(self) -> &'static str {
        match self {
            HostBattlePlan::Bombardment => BATTLE_PLAN_BOMBARDMENT_PACK_AUDIO,
            HostBattlePlan::HoldTheLine => BATTLE_PLAN_HOLD_THE_LINE_PACK_AUDIO,
            HostBattlePlan::SearchAndDestroy => BATTLE_PLAN_SEARCH_AND_DESTROY_PACK_AUDIO,
        }
    }
}

/// Whether template is a residual USA Strategy Center building.
///
/// Fail-closed: name residual (not full BattlePlanUpdate module matrix).
pub fn is_strategy_center_template(template_name: &str) -> bool {
    let n = alnum_lower(template_name);
    if n.is_empty() {
        return false;
    }
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("crate")
        || n.contains("commandset")
        || n.contains("gun")
    {
        return false;
    }
    n.contains("strategycenter")
}

fn alnum_lower(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Whether residual unit can receive Strategy Center army battle-plan bonuses.
///
/// Retail BattlePlanUpdate ValidMemberKindOf / InvalidMemberKindOf residual:
/// - Valid: INFANTRY | CAN_ATTACK | VEHICLE
/// - Invalid: DOZER | STRUCTURE | AIRCRAFT | DRONE
/// Host residual also requires same-team + alive + not under construction.
pub fn is_legal_battle_plan_member(
    is_infantry: bool,
    is_vehicle: bool,
    can_attack: bool,
    is_structure: bool,
    is_aircraft: bool,
    is_dozer: bool,
    is_drone: bool,
    is_alive: bool,
    same_team: bool,
    under_construction: bool,
) -> bool {
    if !is_alive || !same_team || under_construction {
        return false;
    }
    if is_structure || is_aircraft || is_dozer || is_drone {
        return false;
    }
    // ValidMember residual: INFANTRY | CAN_ATTACK | VEHICLE
    is_infantry || is_vehicle || can_attack
}

/// Residual dozer name filter (InvalidMemberKindOf DOZER).
pub fn is_dozer_template_name(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    n.contains("dozer") || n.contains("constructionvehicle")
}

/// Residual drone name filter (InvalidMemberKindOf DRONE).
pub fn is_drone_template_name(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    n.contains("drone") || n.contains("spyplane")
}

/// One active residual battle-plan selection bookkeeping entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostBattlePlanSelection {
    pub id: u32,
    pub player_id: u32,
    pub plan: HostBattlePlan,
    pub activate_frame: u32,
    pub strategy_center_id: Option<ObjectId>,
    /// Same-team legal army members that received residual bonuses.
    pub buffs: u32,
    /// Strategy Center building residual applied (max-health / detect / sight).
    pub building_bonus: bool,
    /// Same-team legal army members that received DISABLED_PARALYZED residual.
    #[serde(default)]
    pub paralyzed: u32,
}

/// Host residual registry for Strategy Center battle plans.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostBattlePlanRegistry {
    next_id: u32,
    /// Per-player currently active residual plan (player_id → plan).
    active_by_player: Vec<(u32, HostBattlePlan)>,
    /// Recent selections (bookkeeping; unit flags live on objects).
    selections: Vec<HostBattlePlanSelection>,
    /// Per-center pack/unpack door residual states.
    #[serde(default)]
    door_states: Vec<HostBattlePlanDoorState>,
    /// Total plan selections (honesty).
    pub selection_count: u32,
    /// Total army member residual grants.
    pub buff_count: u32,
    /// Total building residual grants.
    pub building_bonus_count: u32,
    /// Total army members hit by BattlePlanChangeParalyze residual.
    #[serde(default)]
    pub paralyze_count: u32,
    /// Bombardment turret residual fires (StrategyCenterGun).
    #[serde(default)]
    pub turret_fire_count: u32,
    /// Units hit by StrategyCenterGun residual splash/intended.
    #[serde(default)]
    pub turret_units_hit: u32,
    /// StealthDetectorUpdate residual enables (SearchAndDestroy setSDEnabled).
    #[serde(default)]
    pub stealth_detector_enable_count: u32,
    /// StealthDetectorUpdate residual disables.
    #[serde(default)]
    pub stealth_detector_disable_count: u32,
    /// Pack/unpack door residual transitions started (OPENING or CLOSING).
    #[serde(default)]
    pub door_transition_count: u32,
    /// Door residual reached WAITING_TO_CLOSE (unpack complete residual).
    #[serde(default)]
    pub door_active_count: u32,
    /// Delayed setBattlePlan apply residuals (BecameActive → army/building buffs).
    #[serde(default)]
    pub delayed_active_apply_count: u32,
    /// setBattlePlan(NONE) clear residuals (BeganPacking).
    #[serde(default)]
    pub pack_clear_count: u32,
    /// Bombardment turret recenter residual starts before pack.
    #[serde(default)]
    pub turret_recenter_count: u32,
    /// TurretAI idle-scan residual starts (Bombardment ACTIVE idle).
    #[serde(default)]
    pub turret_idle_scan_start_count: u32,
    /// TurretAI idle-scan residual completions (angles reached desired).
    #[serde(default)]
    pub turret_idle_scan_complete_count: u32,
    /// TurretAI HoldTurret residual starts (after idle-scan complete).
    #[serde(default)]
    pub turret_hold_start_count: u32,
    /// TurretAI HoldTurret residual completions (elapsed → idle recenter).
    #[serde(default)]
    pub turret_hold_complete_count: u32,
    /// TurretAI idle-recenter residual starts (after Hold → RECENTER).
    #[serde(default)]
    pub turret_idle_recenter_start_count: u32,
    /// TurretAI idle-recenter residual completions (angles back to natural).
    #[serde(default)]
    pub turret_idle_recenter_complete_count: u32,
}

impl HostBattlePlanRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Drop door residual for a destroyed Strategy Center.
    pub fn clear_door_for_center(&mut self, center_id: ObjectId) {
        self.door_states.retain(|s| s.center_id != center_id);
    }

    pub fn selection_count(&self) -> u32 {
        self.selection_count
    }

    pub fn buff_count(&self) -> u32 {
        self.buff_count
    }

    pub fn building_bonus_count(&self) -> u32 {
        self.building_bonus_count
    }

    pub fn paralyze_count(&self) -> u32 {
        self.paralyze_count
    }

    pub fn turret_fire_count(&self) -> u32 {
        self.turret_fire_count
    }

    pub fn turret_units_hit(&self) -> u32 {
        self.turret_units_hit
    }

    pub fn stealth_detector_enable_count(&self) -> u32 {
        self.stealth_detector_enable_count
    }

    pub fn stealth_detector_disable_count(&self) -> u32 {
        self.stealth_detector_disable_count
    }

    pub fn door_transition_count(&self) -> u32 {
        self.door_transition_count
    }

    pub fn door_active_count(&self) -> u32 {
        self.door_active_count
    }

    pub fn delayed_active_apply_count(&self) -> u32 {
        self.delayed_active_apply_count
    }

    pub fn pack_clear_count(&self) -> u32 {
        self.pack_clear_count
    }

    pub fn turret_recenter_count(&self) -> u32 {
        self.turret_recenter_count
    }

    pub fn turret_idle_scan_start_count(&self) -> u32 {
        self.turret_idle_scan_start_count
    }

    pub fn turret_idle_scan_complete_count(&self) -> u32 {
        self.turret_idle_scan_complete_count
    }

    pub fn turret_hold_start_count(&self) -> u32 {
        self.turret_hold_start_count
    }

    pub fn turret_hold_complete_count(&self) -> u32 {
        self.turret_hold_complete_count
    }

    pub fn turret_idle_recenter_start_count(&self) -> u32 {
        self.turret_idle_recenter_start_count
    }

    pub fn turret_idle_recenter_complete_count(&self) -> u32 {
        self.turret_idle_recenter_complete_count
    }

    pub fn record_turret_fire(&mut self, units_hit: u32) {
        self.turret_fire_count = self.turret_fire_count.saturating_add(1);
        self.turret_units_hit = self.turret_units_hit.saturating_add(units_hit);
    }

    pub fn record_turret_idle_scan_start(&mut self) {
        self.turret_idle_scan_start_count = self.turret_idle_scan_start_count.saturating_add(1);
    }

    pub fn record_turret_idle_scan_complete(&mut self) {
        self.turret_idle_scan_complete_count =
            self.turret_idle_scan_complete_count.saturating_add(1);
    }

    pub fn record_turret_hold_start(&mut self) {
        self.turret_hold_start_count = self.turret_hold_start_count.saturating_add(1);
    }

    pub fn record_turret_hold_complete(&mut self) {
        self.turret_hold_complete_count = self.turret_hold_complete_count.saturating_add(1);
    }

    pub fn record_turret_idle_recenter_start(&mut self) {
        self.turret_idle_recenter_start_count =
            self.turret_idle_recenter_start_count.saturating_add(1);
    }

    pub fn record_turret_idle_recenter_complete(&mut self) {
        self.turret_idle_recenter_complete_count =
            self.turret_idle_recenter_complete_count.saturating_add(1);
    }

    /// Residual honesty: TurretAI idle-scan residual started at least once.
    pub fn honesty_turret_idle_scan_ok(&self) -> bool {
        self.turret_idle_scan_start_count > 0
    }

    /// Residual honesty: TurretAI HoldTurret residual started at least once.
    pub fn honesty_turret_hold_ok(&self) -> bool {
        self.turret_hold_start_count > 0
    }

    /// Residual honesty: TurretAI idle-recenter residual completed at least once.
    pub fn honesty_turret_idle_recenter_ok(&self) -> bool {
        self.turret_idle_recenter_complete_count > 0
    }

    pub fn record_stealth_detector_enable(&mut self) {
        self.stealth_detector_enable_count = self.stealth_detector_enable_count.saturating_add(1);
    }

    pub fn record_stealth_detector_disable(&mut self) {
        self.stealth_detector_disable_count =
            self.stealth_detector_disable_count.saturating_add(1);
    }

    pub fn record_delayed_active_apply(&mut self) {
        self.delayed_active_apply_count = self.delayed_active_apply_count.saturating_add(1);
    }

    pub fn record_pack_clear(&mut self) {
        self.pack_clear_count = self.pack_clear_count.saturating_add(1);
    }

    pub fn record_turret_recenter(&mut self) {
        self.turret_recenter_count = self.turret_recenter_count.saturating_add(1);
    }

    /// Record army buff / building / paralyze honesty after delayed ACTIVE apply.
    pub fn record_effect_application(&mut self, buffs: u32, building_bonus: bool, paralyzed: u32) {
        self.buff_count = self.buff_count.saturating_add(buffs);
        if building_bonus {
            self.building_bonus_count = self.building_bonus_count.saturating_add(1);
        }
        self.paralyze_count = self.paralyze_count.saturating_add(paralyzed);
    }

    pub fn selections(&self) -> &[HostBattlePlanSelection] {
        &self.selections
    }

    pub fn door_states(&self) -> &[HostBattlePlanDoorState] {
        &self.door_states
    }

    /// Door residual state for a Strategy Center object id.
    pub fn door_state_for_center(&self, center_id: ObjectId) -> Option<&HostBattlePlanDoorState> {
        self.door_states.iter().find(|s| s.center_id == center_id)
    }

    /// Start pack/unpack door residual for a plan selection on a center.
    ///
    /// First select → UNPACKING (OPENING). Plan switch from Active → PACKING
    /// (or recenter then PACKING for Bombardment) then UNPACKING of desired
    /// (TransitionIdleTime 0 residual).
    ///
    /// `turret_natural`: when leaving Bombardment Active, if false host starts
    /// recenter residual before pack (C++ isTurretInNaturalPosition gate).
    /// `recenter_frames`: angle-based / busy-coast frames when non-natural.
    ///
    /// Returns residual events emitted immediately (Audio / BeganPacking / BeganRecenter).
    pub fn begin_door_residual(
        &mut self,
        center_id: ObjectId,
        player_id: u32,
        plan: HostBattlePlan,
        frame: u32,
        turret_natural: bool,
        recenter_frames: u32,
    ) -> Vec<HostBattlePlanDoorEvent> {
        let existing = self
            .door_states
            .iter()
            .position(|s| s.center_id == center_id);
        let mut events = Vec::new();
        match existing {
            Some(idx) => {
                let state = &mut self.door_states[idx];
                if state.status == HostBattlePlanTransition::Active
                    && state.door_plan.is_some()
                    && state.door_plan != Some(plan)
                {
                    // Leaving Bombardment with non-natural turret → recenter first.
                    if state.door_plan == Some(HostBattlePlan::Bombardment) && !turret_natural {
                        state.begin_recenter(frame, Some(plan), recenter_frames);
                        self.turret_recenter_count =
                            self.turret_recenter_count.saturating_add(1);
                        events.push(HostBattlePlanDoorEvent::BeganRecenter {
                            center_id,
                            player_id,
                        });
                    } else {
                        // Pack current door residual, then unpack desired after 210 frames.
                        let pack_audio = state.door_plan.map(|p| p.pack_audio());
                        if state.begin_pack(frame, Some(plan)) {
                            self.door_transition_count =
                                self.door_transition_count.saturating_add(1);
                            events.push(HostBattlePlanDoorEvent::BeganPacking {
                                center_id,
                                player_id,
                            });
                            if let Some(event) = pack_audio {
                                events.push(HostBattlePlanDoorEvent::Audio { center_id, event });
                            }
                        }
                    }
                } else if state.status == HostBattlePlanTransition::Active
                    && state.door_plan == Some(plan)
                {
                    // Same plan re-select residual: no door change.
                } else if state.centering_turret {
                    // Mid-recenter: update desired plan residual only.
                    state.desired_plan = Some(plan);
                } else {
                    // Idle / mid-transition fail-closed: begin unpack of plan.
                    state.begin_unpack(plan, frame);
                    self.door_transition_count = self.door_transition_count.saturating_add(1);
                    events.push(HostBattlePlanDoorEvent::Audio {
                        center_id,
                        event: plan.unpack_audio(),
                    });
                }
            }
            None => {
                let mut state = HostBattlePlanDoorState::new(center_id, player_id);
                state.begin_unpack(plan, frame);
                self.door_states.push(state);
                self.door_transition_count = self.door_transition_count.saturating_add(1);
                events.push(HostBattlePlanDoorEvent::Audio {
                    center_id,
                    event: plan.unpack_audio(),
                });
            }
        }
        events
    }

    /// Tick all door residuals; returns lifecycle events (apply/clear/audio).
    pub fn tick_door_residuals(&mut self, frame: u32) -> Vec<HostBattlePlanDoorEvent> {
        let mut events = Vec::new();
        for state in &mut self.door_states {
            let before = state.status;
            let tick_events = state.tick(frame);
            for ev in &tick_events {
                match ev {
                    HostBattlePlanDoorEvent::Audio { .. } => {
                        self.door_transition_count =
                            self.door_transition_count.saturating_add(1);
                    }
                    HostBattlePlanDoorEvent::BecameActive { .. } => {
                        self.door_active_count = self.door_active_count.saturating_add(1);
                    }
                    HostBattlePlanDoorEvent::BeganPacking { .. } => {
                        self.door_transition_count =
                            self.door_transition_count.saturating_add(1);
                    }
                    HostBattlePlanDoorEvent::BeganRecenter { .. } => {
                        self.turret_recenter_count =
                            self.turret_recenter_count.saturating_add(1);
                    }
                }
            }
            // Also count Unpacking→Active if BecameActive was emitted.
            if before == HostBattlePlanTransition::Unpacking
                && state.status == HostBattlePlanTransition::Active
                && !tick_events
                    .iter()
                    .any(|e| matches!(e, HostBattlePlanDoorEvent::BecameActive { .. }))
            {
                self.door_active_count = self.door_active_count.saturating_add(1);
            }
            events.extend(tick_events);
        }
        events
    }

    pub fn active_plan_for_player(&self, player_id: u32) -> Option<HostBattlePlan> {
        self.active_by_player
            .iter()
            .find(|(pid, _)| *pid == player_id)
            .map(|(_, p)| *p)
    }

    pub fn set_active_plan(&mut self, player_id: u32, plan: HostBattlePlan) {
        if let Some(entry) = self
            .active_by_player
            .iter_mut()
            .find(|(pid, _)| *pid == player_id)
        {
            entry.1 = plan;
        } else {
            self.active_by_player.push((player_id, plan));
        }
    }

    pub fn clear_active_plan(&mut self, player_id: u32) {
        self.active_by_player.retain(|(pid, _)| *pid != player_id);
    }

    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    /// Record a residual battle-plan selection intent.
    ///
    /// Delayed ACTIVE residual: pass buffs/building/paralyzed = 0 here; call
    /// `record_effect_application` + `set_active_plan` when door becomes ACTIVE.
    /// Immediate residual (no center): pass applied counts and set_active=true.
    pub fn record_selection(
        &mut self,
        selection: HostBattlePlanSelection,
        set_active: bool,
    ) {
        self.selection_count = self.selection_count.saturating_add(1);
        self.buff_count = self.buff_count.saturating_add(selection.buffs);
        if selection.building_bonus {
            self.building_bonus_count = self.building_bonus_count.saturating_add(1);
        }
        self.paralyze_count = self.paralyze_count.saturating_add(selection.paralyzed);
        if set_active {
            self.set_active_plan(selection.player_id, selection.plan);
        }
        self.selections.push(selection);
        // Keep bookkeeping bounded (residual, not full history Xfer).
        if self.selections.len() > 32 {
            let drain = self.selections.len() - 32;
            self.selections.drain(0..drain);
        }
    }

    /// Residual honesty: at least one battle plan selected.
    pub fn honesty_select_ok(&self) -> bool {
        self.selection_count > 0
    }

    /// Residual honesty: at least one unit received army residual bonuses.
    pub fn honesty_buff_ok(&self) -> bool {
        self.buff_count > 0
    }

    /// Residual honesty: BattlePlanChangeParalyze applied to at least one unit.
    pub fn honesty_paralyze_ok(&self) -> bool {
        self.paralyze_count > 0
    }

    /// Residual honesty: Bombardment turret StrategyCenterGun fired at least once.
    pub fn honesty_turret_fire_ok(&self) -> bool {
        self.turret_fire_count > 0
    }

    /// Residual honesty: StealthDetectorUpdate enabled at least once (S&D residual).
    pub fn honesty_stealth_detector_ok(&self) -> bool {
        self.stealth_detector_enable_count > 0
    }

    /// Residual honesty: pack/unpack door residual started at least once.
    pub fn honesty_door_residual_ok(&self) -> bool {
        self.door_transition_count > 0
    }

    /// Residual honesty: door residual reached ACTIVE / WAITING_TO_CLOSE.
    pub fn honesty_door_active_ok(&self) -> bool {
        self.door_active_count > 0
    }

    /// Residual honesty: delayed setBattlePlan applied at least once after unpack.
    pub fn honesty_delayed_active_apply_ok(&self) -> bool {
        self.delayed_active_apply_count > 0
    }

    /// Residual honesty: setBattlePlan(NONE) pack-clear residual fired at least once.
    pub fn honesty_pack_clear_ok(&self) -> bool {
        self.pack_clear_count > 0
    }

    /// Residual honesty: Bombardment turret recenter residual started at least once.
    pub fn honesty_turret_recenter_ok(&self) -> bool {
        self.turret_recenter_count > 0
    }

    /// Combined host path: selected and applied at least one army buff.
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_select_ok() && self.honesty_buff_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn battle_plan_constants_match_retail_residual() {
        assert!((BOMBARDMENT_DAMAGE_MULT - 1.20).abs() < 0.001);
        assert!((HOLD_THE_LINE_ARMOR_DAMAGE_SCALAR - 0.9).abs() < 0.001);
        assert!((SEARCH_AND_DESTROY_RANGE_MULT - 1.20).abs() < 0.001);
        assert!((SEARCH_AND_DESTROY_SIGHT_RANGE_SCALAR - 1.2).abs() < 0.001);
        assert!((STRATEGY_CENTER_HOLD_THE_LINE_MAX_HEALTH_SCALAR - 2.0).abs() < 0.001);
        assert!((STRATEGY_CENTER_SEARCH_AND_DESTROY_SIGHT_SCALAR - 2.0).abs() < 0.001);
        assert_eq!(BATTLE_PLAN_PARALYZE_TIME_MS, 5000);
        assert_eq!(BATTLE_PLAN_PARALYZE_FRAMES, 150);
        assert!((STRATEGY_CENTER_GUN_DAMAGE - 200.0).abs() < 0.001);
        assert!((STRATEGY_CENTER_GUN_PRIMARY_RADIUS - 25.0).abs() < 0.001);
        assert!((STRATEGY_CENTER_GUN_RANGE - 400.0).abs() < 0.001);
        assert!((STRATEGY_CENTER_GUN_MIN_RANGE - 100.0).abs() < 0.001);
        assert_eq!(STRATEGY_CENTER_GUN_DELAY_FRAMES, 210);
        let gun = strategy_center_gun_weapon();
        assert!((gun.damage - 200.0).abs() < 0.001);
        assert!((gun.range - 400.0).abs() < 0.001);
        assert!((gun.min_range - 100.0).abs() < 0.001);
        assert!((gun.reload_time - 7.0).abs() < 0.001);
        assert!(strategy_center_gun_in_range(150.0));
        assert!(!strategy_center_gun_in_range(50.0));
        assert!(!strategy_center_gun_in_range(500.0));
        assert!((strategy_center_gun_damage_at(10.0) - 200.0).abs() < 0.001);
        assert!((strategy_center_gun_damage_at(30.0) - 0.0).abs() < 0.001);
        assert_eq!(battle_plan_paralyze_frames_from_ms(5000), 150);
        assert_eq!(battle_plan_paralyze_until_frame(10), 160);
        assert!(!BATTLE_PLAN_BOMBARDMENT_AUDIO.is_empty());
        assert!(!BATTLE_PLAN_HOLD_THE_LINE_AUDIO.is_empty());
        assert!(!BATTLE_PLAN_SEARCH_AND_DESTROY_AUDIO.is_empty());
        // StealthDetectorUpdate residual (Strategy Center ModuleTag_16).
        assert!((STRATEGY_CENTER_STEALTH_DETECTION_RANGE - 500.0).abs() < 0.001);
        assert_eq!(STRATEGY_CENTER_STEALTH_DETECTION_RATE_MS, 500);
        assert_eq!(STRATEGY_CENTER_STEALTH_DETECTION_RATE_FRAMES, 15);
        assert_eq!(STRATEGY_CENTER_STEALTH_DETECTION_HOLD_FRAMES, 16);
        assert!(STRATEGY_CENTER_SEARCH_AND_DESTROY_DETECTS_STEALTH);
        assert!(strategy_center_stealth_detector_enabled_for_plan(
            HostBattlePlan::SearchAndDestroy
        ));
        assert!(!strategy_center_stealth_detector_enabled_for_plan(
            HostBattlePlan::Bombardment
        ));
        assert!(!strategy_center_stealth_detector_enabled_for_plan(
            HostBattlePlan::HoldTheLine
        ));
        assert!(
            (strategy_center_stealth_detection_range_when_enabled() - 500.0).abs() < 0.001
        );
        // DetectionRate residual: immediate first scan, then rate-gated; hold = rate+1.
        assert!(stealth_detector_scan_due(15, 0, 0));
        assert!(stealth_detector_scan_due(15, 0, 100));
        assert!(stealth_detector_scan_due(15, 10, 10));
        assert!(stealth_detector_scan_due(15, 10, 11));
        assert!(!stealth_detector_scan_due(15, 10, 9));
        assert!(stealth_detector_scan_due(0, 99, 0)); // continuous legacy
        assert_eq!(stealth_detector_next_scan_frame(15, 10), 25);
        assert_eq!(stealth_detector_next_scan_frame(0, 10), 0);
        assert_eq!(stealth_detector_hold_frames(15), 16);
        assert_eq!(stealth_detector_hold_frames(0), 30);
        // Pack/unpack door animation residual (AnimationTime 7000ms → 210 frames).
        assert_eq!(BATTLE_PLAN_ANIMATION_TIME_MS, 7000);
        assert_eq!(BATTLE_PLAN_ANIMATION_FRAMES, 210);
        assert_eq!(BATTLE_PLAN_TRANSITION_IDLE_FRAMES, 0);
        assert_eq!(BATTLE_PLAN_TURRET_RECENTER_FRAMES, 30);
        assert_eq!(battle_plan_animation_frames_from_ms(7000), 210);
        assert_eq!(battle_plan_animation_ready_frame(10), 220);
        assert_eq!(
            HostBattlePlan::Bombardment.door_opening(),
            HostBattlePlanDoor::Door1Opening
        );
        assert_eq!(
            HostBattlePlan::HoldTheLine.door_waiting(),
            HostBattlePlanDoor::Door2WaitingToClose
        );
        assert_eq!(
            HostBattlePlan::SearchAndDestroy.door_closing(),
            HostBattlePlanDoor::Door3Closing
        );
        // Turret natural-position residual gate.
        assert!(strategy_center_turret_is_natural(false, false, None));
        assert!(strategy_center_turret_is_natural(false, false, Some(30)));
        assert!(!strategy_center_turret_is_natural(true, false, None));
        assert!(!strategy_center_turret_is_natural(false, true, None));
        assert!(!strategy_center_turret_is_natural(false, false, Some(5)));
        // Pitch/yaw natural-position residual (NaturalTurretAngle -90 / Pitch 45).
        assert!((STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG - (-90.0)).abs() < 0.001);
        assert!((STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG - 45.0).abs() < 0.001);
        assert!((STRATEGY_CENTER_FIRE_PITCH_DEG - 45.0).abs() < 0.001);
        assert!((STRATEGY_CENTER_TURRET_TURN_DEG_PER_FRAME - 2.0).abs() < 0.001);
        assert!((STRATEGY_CENTER_TURRET_PITCH_DEG_PER_FRAME - 2.0).abs() < 0.001);
        assert!(turret_angles_are_natural(-90.0, 45.0));
        assert!(!turret_angles_are_natural(0.0, 45.0));
        assert!(!turret_angles_are_natural(-90.0, 0.0));
        assert!(!strategy_center_turret_is_natural_with_angles(
            false, false, None, 0.0, 45.0
        ));
        // 60° off natural → 30 frames at 2 deg/frame.
        assert_eq!(turret_recenter_frames_for_angles(-30.0, 45.0), 30);
        let (a1, p1) = step_turret_toward_natural(-30.0, 45.0);
        assert!((a1 - (-32.0)).abs() < 0.001);
        assert!((p1 - 45.0).abs() < 0.001);
        let (aim_a, aim_p) = strategy_center_turret_aim_at(0.0, 0.0, 100.0, 0.0);
        assert!((aim_p - 45.0).abs() < 0.001);
        assert!(!turret_angles_are_natural(aim_a, aim_p) || aim_a.abs() < 0.001);
        // Busy-only non-natural with natural angles → fixed 30 coast.
        assert_eq!(
            strategy_center_turret_recenter_frames(true, -90.0, 45.0),
            BATTLE_PLAN_TURRET_RECENTER_FRAMES
        );
        assert_eq!(strategy_center_turret_recenter_frames(false, -30.0, 45.0), 30);

        // TurretAI idle-scan residual matrix (Min/MaxIdleScanAngle/Interval).
        assert_eq!(STRATEGY_CENTER_MIN_IDLE_SCAN_INTERVAL_FRAMES, 15);
        assert_eq!(STRATEGY_CENTER_MAX_IDLE_SCAN_INTERVAL_FRAMES, 30);
        assert!((STRATEGY_CENTER_MIN_IDLE_SCAN_ANGLE_DEG - 0.0).abs() < 0.001);
        assert!((STRATEGY_CENTER_MAX_IDLE_SCAN_ANGLE_DEG - 60.0).abs() < 0.001);
        assert_eq!(idle_scan_interval_frames(0), 15);
        assert_eq!(idle_scan_interval_frames(1), 30);
        assert_eq!(idle_scan_interval_frames(2), 15);
        assert!((idle_scan_desired_offset_deg(0) - 30.0).abs() < 0.001);
        assert!((idle_scan_desired_offset_deg(1) - (-30.0)).abs() < 0.001);
        // Desired absolute yaw = natural (-90) + offset.
        assert!((idle_scan_desired_angle_deg(0) - (-60.0)).abs() < 0.001);
        assert!((idle_scan_desired_angle_deg(1) - (-120.0)).abs() < 0.001);
        // Idle-scan step toward desired: from natural toward -60 at 2 deg/frame.
        let (sa, sp) = step_turret_toward_angles(-90.0, 45.0, -60.0, 45.0);
        assert!((sa - (-88.0)).abs() < 0.001);
        assert!((sp - 45.0).abs() < 0.001);
        assert!(turret_angles_at(-60.0, 45.0, -60.0, 45.0));
        assert!(!turret_angles_at(-90.0, 45.0, -60.0, 45.0));
        // HoldTurret residual: default RecenterTime 60 frames.
        assert_eq!(STRATEGY_CENTER_RECENTER_TIME_FRAMES, 60);
        assert_eq!(hold_turret_until_frame(10), 70);
        assert!(!hold_turret_elapsed(69, 70));
        assert!(hold_turret_elapsed(70, 70));
        assert!(hold_turret_elapsed(71, 70));
        assert!(!hold_turret_elapsed(100, 0));
    }

    #[test]
    fn door_residual_unpack_then_active_matrix() {
        let mut state = HostBattlePlanDoorState::new(ObjectId(1), 0);
        state.begin_unpack(HostBattlePlan::Bombardment, 0);
        assert_eq!(state.status, HostBattlePlanTransition::Unpacking);
        assert_eq!(state.door, HostBattlePlanDoor::Door1Opening);
        assert_eq!(state.next_ready_frame, BATTLE_PLAN_ANIMATION_FRAMES);
        // Mid-animation: no transition.
        assert!(state.tick(100).is_empty());
        assert_eq!(state.status, HostBattlePlanTransition::Unpacking);
        // Animation complete → ACTIVE / WAITING_TO_CLOSE + BecameActive.
        let events = state.tick(BATTLE_PLAN_ANIMATION_FRAMES);
        assert_eq!(state.status, HostBattlePlanTransition::Active);
        assert_eq!(state.door, HostBattlePlanDoor::Door1WaitingToClose);
        assert!(
            events.iter().any(|e| matches!(
                e,
                HostBattlePlanDoorEvent::BecameActive {
                    plan: HostBattlePlan::Bombardment,
                    ..
                }
            )),
            "unpack complete must emit BecameActive residual"
        );
    }

    #[test]
    fn door_residual_pack_then_unpack_switch_matrix() {
        let mut reg = HostBattlePlanRegistry::new();
        let cid = ObjectId(7);
        let events = reg.begin_door_residual(
            cid,
            0,
            HostBattlePlan::Bombardment,
            0,
            true,
            BATTLE_PLAN_TURRET_RECENTER_FRAMES,
        );
        assert!(
            events.iter().any(|e| matches!(
                e,
                HostBattlePlanDoorEvent::Audio {
                    event: BATTLE_PLAN_BOMBARDMENT_UNPACK_AUDIO,
                    ..
                }
            ))
        );
        assert!(reg.honesty_door_residual_ok());
        // Complete unpack residual.
        let events = reg.tick_door_residuals(BATTLE_PLAN_ANIMATION_FRAMES);
        assert!(reg.honesty_door_active_ok());
        assert!(
            events.iter().any(|e| matches!(
                e,
                HostBattlePlanDoorEvent::BecameActive {
                    plan: HostBattlePlan::Bombardment,
                    ..
                }
            ))
        );
        let state = reg.door_state_for_center(cid).unwrap();
        assert_eq!(state.status, HostBattlePlanTransition::Active);
        assert_eq!(state.door, HostBattlePlanDoor::Door1WaitingToClose);
        // Switch to HoldTheLine with natural turret → PACKING door1 CLOSING.
        let pack_events = reg.begin_door_residual(
            cid,
            0,
            HostBattlePlan::HoldTheLine,
            300,
            true,
            BATTLE_PLAN_TURRET_RECENTER_FRAMES,
        );
        assert!(
            pack_events
                .iter()
                .any(|e| matches!(e, HostBattlePlanDoorEvent::BeganPacking { .. }))
        );
        assert!(
            pack_events.iter().any(|e| matches!(
                e,
                HostBattlePlanDoorEvent::Audio {
                    event: BATTLE_PLAN_BOMBARDMENT_PACK_AUDIO,
                    ..
                }
            ))
        );
        let state = reg.door_state_for_center(cid).unwrap();
        assert_eq!(state.status, HostBattlePlanTransition::Packing);
        assert_eq!(state.door, HostBattlePlanDoor::Door1Closing);
        // Pack complete → idle → unpack HoldTheLine (TransitionIdleTime 0).
        let events = reg.tick_door_residuals(300 + BATTLE_PLAN_ANIMATION_FRAMES);
        assert!(
            events.iter().any(|e| matches!(
                e,
                HostBattlePlanDoorEvent::Audio {
                    event: BATTLE_PLAN_HOLD_THE_LINE_UNPACK_AUDIO,
                    ..
                }
            ))
        );
        let state = reg.door_state_for_center(cid).unwrap();
        assert_eq!(state.status, HostBattlePlanTransition::Unpacking);
        assert_eq!(state.door, HostBattlePlanDoor::Door2Opening);
    }

    #[test]
    fn door_residual_bombardment_recenter_before_pack_matrix() {
        let mut reg = HostBattlePlanRegistry::new();
        let cid = ObjectId(9);
        let _ = reg.begin_door_residual(
            cid,
            0,
            HostBattlePlan::Bombardment,
            0,
            true,
            BATTLE_PLAN_TURRET_RECENTER_FRAMES,
        );
        let _ = reg.tick_door_residuals(BATTLE_PLAN_ANIMATION_FRAMES);
        // Non-natural turret → recenter residual (pack deferred).
        let events = reg.begin_door_residual(
            cid,
            0,
            HostBattlePlan::HoldTheLine,
            300,
            false,
            BATTLE_PLAN_TURRET_RECENTER_FRAMES,
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, HostBattlePlanDoorEvent::BeganRecenter { .. }))
        );
        assert!(reg.honesty_turret_recenter_ok());
        let state = reg.door_state_for_center(cid).unwrap();
        assert!(state.centering_turret);
        assert_eq!(state.status, HostBattlePlanTransition::Active);
        assert_eq!(
            state.next_ready_frame,
            300 + BATTLE_PLAN_TURRET_RECENTER_FRAMES
        );
        // Recenter complete → PACKING.
        let events = reg.tick_door_residuals(300 + BATTLE_PLAN_TURRET_RECENTER_FRAMES);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, HostBattlePlanDoorEvent::BeganPacking { .. }))
        );
        let state = reg.door_state_for_center(cid).unwrap();
        assert_eq!(state.status, HostBattlePlanTransition::Packing);
        assert!(!state.centering_turret);
    }

    #[test]
    fn stealth_detector_honesty_counters() {
        let mut reg = HostBattlePlanRegistry::new();
        assert!(!reg.honesty_stealth_detector_ok());
        reg.record_stealth_detector_enable();
        assert!(reg.honesty_stealth_detector_ok());
        assert_eq!(reg.stealth_detector_enable_count(), 1);
        reg.record_stealth_detector_disable();
        assert_eq!(reg.stealth_detector_disable_count(), 1);
    }

    #[test]
    fn strategy_center_name_matrix() {
        assert!(is_strategy_center_template("AmericaStrategyCenter"));
        assert!(is_strategy_center_template("USA_StrategyCenter"));
        assert!(is_strategy_center_template("SupW_AmericaStrategyCenter"));
        assert!(is_strategy_center_template("Lazr_AmericaStrategyCenter"));
        assert!(is_strategy_center_template("AirF_AmericaStrategyCenter"));
        assert!(is_strategy_center_template("TestStrategyCenter"));
        assert!(!is_strategy_center_template("AmericaCommandCenter"));
        assert!(!is_strategy_center_template("StrategyCenterGun"));
        assert!(!is_strategy_center_template("USA_Ranger"));
        assert!(!is_strategy_center_template("TestTank"));
    }

    #[test]
    fn legal_battle_plan_member_matrix() {
        // infantry, vehicle, can_attack, structure, aircraft, dozer, drone, alive, same_team, under_construction
        assert!(is_legal_battle_plan_member(
            true, false, true, false, false, false, false, true, true, false
        ));
        assert!(is_legal_battle_plan_member(
            false, true, true, false, false, false, false, true, true, false
        ));
        assert!(is_legal_battle_plan_member(
            false, false, true, false, false, false, false, true, true, false
        ));
        // Invalid: structure
        assert!(!is_legal_battle_plan_member(
            false, false, true, true, false, false, false, true, true, false
        ));
        // Invalid: aircraft
        assert!(!is_legal_battle_plan_member(
            false, false, true, false, true, false, false, true, true, false
        ));
        // Invalid: dozer
        assert!(!is_legal_battle_plan_member(
            false, true, true, false, false, true, false, true, true, false
        ));
        // Invalid: drone
        assert!(!is_legal_battle_plan_member(
            false, true, true, false, false, false, true, true, true, false
        ));
        // Invalid: enemy / dead / construction
        assert!(!is_legal_battle_plan_member(
            true, false, true, false, false, false, false, true, false, false
        ));
        assert!(!is_legal_battle_plan_member(
            true, false, true, false, false, false, false, false, true, false
        ));
        assert!(!is_legal_battle_plan_member(
            true, false, true, false, false, false, false, true, true, true
        ));
    }

    #[test]
    fn plan_multipliers_and_parse() {
        assert_eq!(HostBattlePlan::from_u8(1), HostBattlePlan::Bombardment);
        assert_eq!(HostBattlePlan::from_u8(2), HostBattlePlan::HoldTheLine);
        assert_eq!(HostBattlePlan::from_u8(3), HostBattlePlan::SearchAndDestroy);
        assert_eq!(HostBattlePlan::from_u8(99), HostBattlePlan::Bombardment); // fail-closed
        assert!((HostBattlePlan::Bombardment.army_damage_multiplier() - 1.20).abs() < 0.001);
        assert!((HostBattlePlan::HoldTheLine.army_armor_damage_scalar() - 0.9).abs() < 0.001);
        assert!((HostBattlePlan::SearchAndDestroy.army_range_multiplier() - 1.20).abs() < 0.001);
        assert!((HostBattlePlan::SearchAndDestroy.army_sight_range_scalar() - 1.2).abs() < 0.001);
        assert!((HostBattlePlan::Bombardment.army_armor_damage_scalar() - 1.0).abs() < f32::EPSILON);
        assert!((HostBattlePlan::HoldTheLine.army_damage_multiplier() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn honesty_registry_records_buffs() {
        let mut reg = HostBattlePlanRegistry::new();
        assert!(!reg.honesty_host_path_ok());
        let id = reg.alloc_id();
        reg.record_selection(
            HostBattlePlanSelection {
                id,
                player_id: 0,
                plan: HostBattlePlan::Bombardment,
                activate_frame: 0,
                strategy_center_id: None,
                buffs: 2,
                building_bonus: true,
                paralyzed: 2,
            },
            true,
        );
        assert!(reg.honesty_select_ok());
        assert!(reg.honesty_buff_ok());
        assert!(reg.honesty_paralyze_ok());
        assert!(reg.honesty_host_path_ok());
        assert_eq!(reg.selection_count(), 1);
        assert_eq!(reg.buff_count(), 2);
        assert_eq!(reg.building_bonus_count(), 1);
        assert_eq!(reg.paralyze_count(), 2);
        assert_eq!(
            reg.active_plan_for_player(0),
            Some(HostBattlePlan::Bombardment)
        );
    }
    #[test]
    fn strategy_center_gun_target_gate() {
        assert!(is_legal_strategy_center_gun_target(
            true, false, false, false, true, false
        ));
        assert!(!is_legal_strategy_center_gun_target(
            true, true, false, false, true, false
        )); // same team
        assert!(!is_legal_strategy_center_gun_target(
            true, false, false, false, true, true
        )); // air
        assert!(!is_legal_strategy_center_gun_target(
            true, false, true, false, true, false
        )); // neutral
    }

    #[test]
    fn turret_fire_honesty() {
        let mut reg = HostBattlePlanRegistry::new();
        assert!(!reg.honesty_turret_fire_ok());
        reg.record_turret_fire(2);
        assert!(reg.honesty_turret_fire_ok());
        assert_eq!(reg.turret_fire_count(), 1);
        assert_eq!(reg.turret_units_hit(), 2);
    }

}
