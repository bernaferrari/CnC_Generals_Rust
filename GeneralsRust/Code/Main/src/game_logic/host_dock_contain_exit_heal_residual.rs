//! Wave 98 residual peels: Dock residual peels / Contain residual deepen /
//! Exit residual peels / Heal residual deepen.
//!
//! Orthogonal to Waves 52 (repair dock TimeForFullHeal), 64/87 (tunnel/garrison/
//! transport contain), 71 (ambulance/heal pack), 81/usa_pilot (AutoFindHealing
//! retail scan), 83 (supply warehouse dock approach).
//! Host-testable packs for dock/contain/exit/heal residual honesty.
//!
//! Sources (retail ZH C++ / INI):
//! - DockUpdate.h DEFAULT_APPROACH_VECTOR_SIZE **10** / DYNAMIC_APPROACH_VECTOR_FLAG **-1**
//! - DockUpdate.cpp ModuleData defaults + DockUpdate ctor (dockOpen TRUE)
//! - AIDock.h AI_DOCK_* state enum residual (8 states)
//! - AIDock.cpp WaitForClearance timeout **30×LOGICFRAMES_PER_SECOND** = **900**f
//! - RepairDockUpdate TimeForFullHeal default **1.0**f + retail **5000**ms→**150**f /
//!   NumberApproachPositions **5** / isRallyPointAfterDock TRUE
//! - SupplyCenterDockUpdate GrantTemporaryStealth default **0**; GLA **20000**ms→**600**f
//!   NumberApproachPositions America **9** / China+GLA **-1**; AllowsPassthrough No (C/GLA)
//! - RailedTransportDockUpdate ToleranceDistance default **50** / UNLOAD_ALL **-1**;
//!   ferry Pull/Push **4500**ms→**135**f, Tolerance **400**, Approach **9**
//! - OpenContain defaults deepen (BurnedDeath **Yes**, WeaponBonus **No**, KickOutOnCapture **Yes**)
//! - ContainModule.h ObjectEnterExitType / EvacDisposition residual enums
//! - HealContain TimeForFullHeal default **0**; barracks HealContain **2000**ms→**60**f /
//!   ContainMax **10** / allies Yes / enemies+neutral No
//! - ExitDoorType residual (UpdateModule.h DOOR_1..4 / COUNT_MAX / NONE_AVAILABLE/NEEDED)
//! - QueueProductionExitUpdate ExitDelay default **0**; ChinaBarracks **300**ms→**9**f
//! - AutoHealBehavior ctor defaults (HealingDelay UINT_MAX, amount **0**, radius **0**)
//! - AutoFindHealingUpdate ctor defaults NeverHeal **0.95** / AlwaysHeal **0.25**
//!   (retail infantry override 0.85/0.25/Scan 1000ms/range 300 already Wave 81)
//! - ParkingPlaceBehavior HealAmountPerSecond default **0**; Airfield **10**/Rows**2**/Cols**2**
//! - RepairDock / HealContain / GarrisonContain heal sliver formulas
//!
//! Fail-closed:
//! - Not full DockUpdate bone load / approach path AI residual
//! - Not full OpenContain exit-door bone matrix / fire-point garrison residual
//! - Not full ExitInterface production door anim residual
//! - Not full AutoHealBehavior multi-healer exclusive / particle pulse residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Logic frames per second residual (host fixed step).
pub const DOCK_CONTAIN_EXIT_HEAL_LOGIC_FPS: f32 = 30.0;

/// Convert msec residual → logic frames @ 30 FPS (round, matches parseDuration for multiples).
#[inline]
pub fn residual_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * DOCK_CONTAIN_EXIT_HEAL_LOGIC_FPS / 1000.0).round() as u32
}

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

// ---------------------------------------------------------------------------
// 1. Dock residual peels
// ---------------------------------------------------------------------------

/// C++ `DEFAULT_APPROACH_VECTOR_SIZE` residual (DockUpdate.h).
pub const DEFAULT_APPROACH_VECTOR_SIZE_RESIDUAL: i32 = 10;
/// C++ `DYNAMIC_APPROACH_VECTOR_FLAG` residual (DockUpdate.h) — infinite boneless approach.
pub const DYNAMIC_APPROACH_VECTOR_FLAG_RESIDUAL: i32 = -1;

/// C++ DockUpdateModuleData default NumberApproachPositions residual.
pub const DOCK_DEFAULT_NUMBER_APPROACH_POSITIONS: i32 = 0;
/// C++ DockUpdateModuleData default AllowsPassthrough residual (TRUE).
pub const DOCK_DEFAULT_ALLOWS_PASSTHROUGH: bool = true;
/// C++ DockUpdate ctor m_dockOpen residual (TRUE).
pub const DOCK_CTOR_OPEN_RESIDUAL: bool = true;
/// C++ DockUpdate ctor m_dockerInside residual (FALSE).
pub const DOCK_CTOR_DOCKER_INSIDE_RESIDUAL: bool = false;
/// C++ DockUpdate ctor m_dockCrippled residual (FALSE).
pub const DOCK_CTOR_CRIPPLED_RESIDUAL: bool = false;
/// C++ DockUpdate ctor m_positionsLoaded residual (FALSE).
pub const DOCK_CTOR_POSITIONS_LOADED_RESIDUAL: bool = false;
/// C++ DockUpdate ctor m_numberApproachPositionBones residual (**-1** until load).
pub const DOCK_CTOR_APPROACH_BONES_UNLOADED: i32 = -1;
/// C++ DockUpdate default isRallyPointAfterDockType residual (FALSE majority).
pub const DOCK_DEFAULT_RALLY_AFTER_DOCK: bool = false;

/// AI_DOCK state count residual (AIDock.h enum 0..7).
pub const AI_DOCK_STATE_COUNT_RESIDUAL: usize = 8;

/// Ordered C++ AI_DOCK_* residual names (index = discriminant).
pub const AI_DOCK_STATE_NAME_TABLE_RESIDUAL: &[&str] = &[
    "AI_DOCK_APPROACH",           // 0
    "AI_DOCK_WAIT_FOR_CLEARANCE", // 1
    "AI_DOCK_ADVANCE_POSITION",   // 2
    "AI_DOCK_MOVE_TO_ENTRY",      // 3
    "AI_DOCK_MOVE_TO_DOCK",       // 4
    "AI_DOCK_PROCESS_DOCK",       // 5
    "AI_DOCK_MOVE_TO_EXIT",       // 6
    "AI_DOCK_MOVE_TO_RALLY",      // 7
];

/// C++ AIDockWaitForClearanceState timeout residual: 30 * LOGICFRAMES_PER_SECOND.
pub const AI_DOCK_WAIT_CLEARANCE_TIMEOUT_FRAMES_RESIDUAL: u32 = 900;

/// C++ RepairDockUpdateModuleData default framesForFullHeal residual (1.0 → instant).
pub const REPAIR_DOCK_DEFAULT_FRAMES_FOR_FULL_HEAL: f32 = 1.0;
/// Retail RepairDockUpdate TimeForFullHeal residual (msec) — WarFactory / TechRepairPad.
pub const REPAIR_DOCK_TIME_FOR_FULL_HEAL_MS_RESIDUAL: u32 = 5000;
/// TimeForFullHeal 5000ms → 150 frames @ 30 FPS.
pub const REPAIR_DOCK_TIME_FOR_FULL_HEAL_FRAMES_RESIDUAL: u32 = 150;
/// Retail RepairDockUpdate NumberApproachPositions residual.
pub const REPAIR_DOCK_NUMBER_APPROACH_POSITIONS_RESIDUAL: i32 = 5;
/// C++ RepairDockUpdate isRallyPointAfterDockType residual (TRUE minority).
pub const REPAIR_DOCK_RALLY_AFTER_DOCK_RESIDUAL: bool = true;

/// C++ SupplyCenterDockUpdateModuleData default GrantTemporaryStealth residual (frames).
pub const SUPPLY_CENTER_DOCK_DEFAULT_GRANT_STEALTH_FRAMES: u32 = 0;
/// Retail GLA SupplyCenterDock GrantTemporaryStealth residual (msec).
pub const SUPPLY_CENTER_DOCK_GLA_GRANT_STEALTH_MS: u32 = 20000;
/// GLA GrantTemporaryStealth 20000ms → 600 frames.
pub const SUPPLY_CENTER_DOCK_GLA_GRANT_STEALTH_FRAMES: u32 = 600;
/// Retail America SupplyCenter NumberApproachPositions residual (bone count).
pub const SUPPLY_CENTER_DOCK_AMERICA_APPROACH_POSITIONS: i32 = 9;
/// Retail China/GLA SupplyCenter NumberApproachPositions residual (boneless infinite).
pub const SUPPLY_CENTER_DOCK_BONELESS_APPROACH_POSITIONS: i32 = -1;
/// Retail China/GLA SupplyCenter AllowsPassthrough residual (No).
pub const SUPPLY_CENTER_DOCK_CHINA_GLA_ALLOWS_PASSTHROUGH: bool = false;

/// C++ RailedTransportDockUpdateModuleData default ToleranceDistance residual.
pub const RAILED_DOCK_DEFAULT_TOLERANCE_DISTANCE: f32 = 50.0;
/// C++ RailedTransportDockUpdateModuleData default PullInsideDuration residual (frames).
pub const RAILED_DOCK_DEFAULT_PULL_INSIDE_FRAMES: u32 = 0;
/// C++ RailedTransportDockUpdateModuleData default PushOutsideDuration residual (frames).
pub const RAILED_DOCK_DEFAULT_PUSH_OUTSIDE_FRAMES: u32 = 0;
/// C++ RailedTransportDockUpdate UNLOAD_ALL residual sentinel.
pub const RAILED_DOCK_UNLOAD_ALL_RESIDUAL: i32 = -1;
/// Retail ferry RailedTransportDockUpdate PullInsideDuration residual (msec).
pub const RAILED_DOCK_FERRY_PULL_INSIDE_MS: u32 = 4500;
/// PullInside 4500ms → 135 frames.
pub const RAILED_DOCK_FERRY_PULL_INSIDE_FRAMES: u32 = 135;
/// Retail ferry PushOutsideDuration residual (msec).
pub const RAILED_DOCK_FERRY_PUSH_OUTSIDE_MS: u32 = 4500;
/// PushOutside 4500ms → 135 frames.
pub const RAILED_DOCK_FERRY_PUSH_OUTSIDE_FRAMES: u32 = 135;
/// Retail ferry ToleranceDistance residual.
pub const RAILED_DOCK_FERRY_TOLERANCE_DISTANCE: f32 = 400.0;
/// Retail ferry NumberApproachPositions residual.
pub const RAILED_DOCK_FERRY_APPROACH_POSITIONS: i32 = 9;

/// Repair-dock residual HP to add per frame:
/// (max_health − current_health) / frames_for_full_heal (C++ RepairDockUpdate::action).
pub fn repair_dock_health_to_add_per_frame(
    max_health: f32,
    current_health: f32,
    frames_for_full_heal: f32,
) -> f32 {
    if frames_for_full_heal <= 0.0 {
        return 0.0;
    }
    let missing = (max_health - current_health).max(0.0);
    missing / frames_for_full_heal
}

/// Whether approach capacity residual allows another docker.
///
/// Dynamic (**-1**) always allows; fixed capacity allows when `reserved < capacity`.
pub fn dock_approach_has_free_slot(reserved: i32, capacity: i32) -> bool {
    if capacity == DYNAMIC_APPROACH_VECTOR_FLAG_RESIDUAL {
        return true;
    }
    if capacity <= 0 {
        // Default 0 with no bones → treat as empty fixed vector (no free slots until bones).
        return false;
    }
    reserved < capacity
}

/// Wave 98 honesty: dock residual pack.
pub fn honesty_dock_residual_pack_wave98() -> bool {
    DEFAULT_APPROACH_VECTOR_SIZE_RESIDUAL == 10
        && DYNAMIC_APPROACH_VECTOR_FLAG_RESIDUAL == -1
        && DOCK_DEFAULT_NUMBER_APPROACH_POSITIONS == 0
        && DOCK_DEFAULT_ALLOWS_PASSTHROUGH
        && DOCK_CTOR_OPEN_RESIDUAL
        && !DOCK_CTOR_DOCKER_INSIDE_RESIDUAL
        && !DOCK_CTOR_CRIPPLED_RESIDUAL
        && !DOCK_CTOR_POSITIONS_LOADED_RESIDUAL
        && DOCK_CTOR_APPROACH_BONES_UNLOADED == -1
        && !DOCK_DEFAULT_RALLY_AFTER_DOCK
        && AI_DOCK_STATE_COUNT_RESIDUAL == 8
        && AI_DOCK_STATE_NAME_TABLE_RESIDUAL.len() == 8
        && residual_name_index(AI_DOCK_STATE_NAME_TABLE_RESIDUAL, "AI_DOCK_APPROACH") == Some(0)
        && residual_name_index(
            AI_DOCK_STATE_NAME_TABLE_RESIDUAL,
            "AI_DOCK_WAIT_FOR_CLEARANCE",
        ) == Some(1)
        && residual_name_index(AI_DOCK_STATE_NAME_TABLE_RESIDUAL, "AI_DOCK_PROCESS_DOCK")
            == Some(5)
        && residual_name_index(AI_DOCK_STATE_NAME_TABLE_RESIDUAL, "AI_DOCK_MOVE_TO_RALLY")
            == Some(7)
        && AI_DOCK_WAIT_CLEARANCE_TIMEOUT_FRAMES_RESIDUAL == 900
        && AI_DOCK_WAIT_CLEARANCE_TIMEOUT_FRAMES_RESIDUAL
            == 30 * (DOCK_CONTAIN_EXIT_HEAL_LOGIC_FPS as u32)
        && (REPAIR_DOCK_DEFAULT_FRAMES_FOR_FULL_HEAL - 1.0).abs() < 0.001
        && REPAIR_DOCK_TIME_FOR_FULL_HEAL_MS_RESIDUAL == 5000
        && REPAIR_DOCK_TIME_FOR_FULL_HEAL_FRAMES_RESIDUAL
            == residual_ms_to_frames(REPAIR_DOCK_TIME_FOR_FULL_HEAL_MS_RESIDUAL)
        && REPAIR_DOCK_NUMBER_APPROACH_POSITIONS_RESIDUAL == 5
        && REPAIR_DOCK_RALLY_AFTER_DOCK_RESIDUAL
        && SUPPLY_CENTER_DOCK_DEFAULT_GRANT_STEALTH_FRAMES == 0
        && SUPPLY_CENTER_DOCK_GLA_GRANT_STEALTH_MS == 20000
        && SUPPLY_CENTER_DOCK_GLA_GRANT_STEALTH_FRAMES
            == residual_ms_to_frames(SUPPLY_CENTER_DOCK_GLA_GRANT_STEALTH_MS)
        && SUPPLY_CENTER_DOCK_AMERICA_APPROACH_POSITIONS == 9
        && SUPPLY_CENTER_DOCK_BONELESS_APPROACH_POSITIONS
            == DYNAMIC_APPROACH_VECTOR_FLAG_RESIDUAL
        && !SUPPLY_CENTER_DOCK_CHINA_GLA_ALLOWS_PASSTHROUGH
        && (RAILED_DOCK_DEFAULT_TOLERANCE_DISTANCE - 50.0).abs() < 0.001
        && RAILED_DOCK_DEFAULT_PULL_INSIDE_FRAMES == 0
        && RAILED_DOCK_DEFAULT_PUSH_OUTSIDE_FRAMES == 0
        && RAILED_DOCK_UNLOAD_ALL_RESIDUAL == -1
        && RAILED_DOCK_FERRY_PULL_INSIDE_MS == 4500
        && RAILED_DOCK_FERRY_PULL_INSIDE_FRAMES
            == residual_ms_to_frames(RAILED_DOCK_FERRY_PULL_INSIDE_MS)
        && RAILED_DOCK_FERRY_PUSH_OUTSIDE_FRAMES
            == residual_ms_to_frames(RAILED_DOCK_FERRY_PUSH_OUTSIDE_MS)
        && (RAILED_DOCK_FERRY_TOLERANCE_DISTANCE - 400.0).abs() < 0.001
        && RAILED_DOCK_FERRY_APPROACH_POSITIONS == 9
        // Approach free-slot residual matrix.
        && dock_approach_has_free_slot(0, DYNAMIC_APPROACH_VECTOR_FLAG_RESIDUAL)
        && dock_approach_has_free_slot(100, DYNAMIC_APPROACH_VECTOR_FLAG_RESIDUAL)
        && dock_approach_has_free_slot(0, 5)
        && dock_approach_has_free_slot(4, 5)
        && !dock_approach_has_free_slot(5, 5)
        && !dock_approach_has_free_slot(0, 0)
        // Repair dock heal residual formula.
        && (repair_dock_health_to_add_per_frame(1000.0, 700.0, 150.0) - 2.0).abs() < 0.0001
        && (repair_dock_health_to_add_per_frame(1000.0, 1000.0, 150.0) - 0.0).abs() < 0.0001
}

// ---------------------------------------------------------------------------
// 2. Contain residual deepen (beyond Wave 87 open/transport/garrison)
// ---------------------------------------------------------------------------

/// C++ OpenContain CONTAIN_MAX_UNKNOWN residual.
pub const CONTAIN_MAX_UNKNOWN_RESIDUAL: i32 = -1;
/// C++ OpenContain default BurnedDeathToUnits residual (TRUE).
pub const OPEN_CONTAIN_DEFAULT_BURNED_DEATH_TO_UNITS: bool = true;
/// C++ OpenContain default PassengersInTurret residual (FALSE).
pub const OPEN_CONTAIN_DEFAULT_PASSENGERS_IN_TURRET: bool = false;
/// C++ OpenContain default WeaponBonusPassedToPassengers residual (FALSE).
pub const OPEN_CONTAIN_DEFAULT_WEAPON_BONUS_TO_PASSENGERS: bool = false;
/// C++ OpenContain isKickOutOnCapture residual default (TRUE).
pub const OPEN_CONTAIN_DEFAULT_KICK_OUT_ON_CAPTURE: bool = true;
/// C++ OpenContain isImmuneToClearBuildingAttacks residual default (TRUE).
pub const OPEN_CONTAIN_DEFAULT_IMMUNE_TO_CLEAR: bool = true;
/// C++ OpenContain isHealContain residual default (FALSE).
pub const OPEN_CONTAIN_DEFAULT_IS_HEAL_CONTAIN: bool = false;
/// C++ OpenContain isGarrisonable residual default (FALSE).
pub const OPEN_CONTAIN_DEFAULT_IS_GARRISONABLE: bool = false;
/// C++ OpenContain isBustable residual default (FALSE).
pub const OPEN_CONTAIN_DEFAULT_IS_BUSTABLE: bool = false;
/// C++ OpenContain isTunnelContain residual default (FALSE).
pub const OPEN_CONTAIN_DEFAULT_IS_TUNNEL: bool = false;
/// C++ OpenContain isDisplayedOnControlBar residual default (FALSE; Transport TRUE).
pub const OPEN_CONTAIN_DEFAULT_DISPLAYED_ON_CONTROL_BAR: bool = false;
/// C++ TransportContain isDisplayedOnControlBar residual (TRUE).
pub const TRANSPORT_CONTAIN_DISPLAYED_ON_CONTROL_BAR: bool = true;
/// C++ OpenContain isEnclosingContainerFor residual typical TRUE (Firebase No already Wave 87).
pub const OPEN_CONTAIN_DEFAULT_ENCLOSING: bool = true;

/// C++ ObjectEnterExitType residual (ContainModule.h).
pub const OBJECT_ENTER_EXIT_TYPE_COUNT_RESIDUAL: usize = 3;
/// Ordered ObjectEnterExitType residual names.
pub const OBJECT_ENTER_EXIT_TYPE_NAME_TABLE_RESIDUAL: &[&str] = &[
    "WANTS_TO_ENTER", // 0
    "WANTS_TO_EXIT",  // 1
    "WANTS_NEITHER",  // 2
];

/// C++ EvacDisposition residual (ContainModule.h).
pub const EVAC_DISPOSITION_COUNT_RESIDUAL: usize = 4;
/// Ordered EvacDisposition residual names.
pub const EVAC_DISPOSITION_NAME_TABLE_RESIDUAL: &[&str] = &[
    "EVAC_INVALID",           // 0
    "EVAC_TO_LEFT",           // 1
    "EVAC_TO_RIGHT",          // 2
    "EVAC_BURST_FROM_CENTER", // 3
];

/// C++ HealContain isHealContain residual (TRUE).
pub const HEAL_CONTAIN_IS_HEAL_CONTAIN_RESIDUAL: bool = true;
/// C++ HealContainModuleData default framesForFullHeal residual (0).
pub const HEAL_CONTAIN_DEFAULT_FRAMES_FOR_FULL_HEAL: u32 = 0;
/// Retail barracks/hospital HealContain TimeForFullHeal residual (msec).
pub const HEAL_CONTAIN_BARRACKS_TIME_FOR_FULL_HEAL_MS: u32 = 2000;
/// Barracks HealContain 2000ms → 60 frames.
pub const HEAL_CONTAIN_BARRACKS_TIME_FOR_FULL_HEAL_FRAMES: u32 = 60;
/// Retail HealContain ContainMax residual (barracks / hospital).
pub const HEAL_CONTAIN_BARRACKS_CONTAIN_MAX: i32 = 10;
/// Retail HealContain AllowAlliesInside residual.
pub const HEAL_CONTAIN_BARRACKS_ALLOW_ALLIES: bool = true;
/// Retail HealContain AllowNeutralInside residual.
pub const HEAL_CONTAIN_BARRACKS_ALLOW_NEUTRAL: bool = false;
/// Retail HealContain AllowEnemiesInside residual.
pub const HEAL_CONTAIN_BARRACKS_ALLOW_ENEMIES: bool = false;
/// Retail HealContain AllowInsideKindOf residual token.
pub const HEAL_CONTAIN_BARRACKS_ALLOW_KIND_OF: &str = "INFANTRY";

/// C++ GarrisonContainModuleData default HealObjects residual (FALSE).
pub const GARRISON_DEFAULT_HEAL_OBJECTS_RESIDUAL: bool = false;
/// C++ GarrisonContainModuleData default framesForFullHeal residual (1.0).
pub const GARRISON_DEFAULT_FRAMES_FOR_FULL_HEAL_RESIDUAL: f32 = 1.0;

/// Heal-contain residual sliver per frame (C++ HealContain::doHeal):
/// `max_health / frames_for_full_heal` until contained duration ≥ frames.
pub fn heal_contain_sliver_amount(max_health: f32, frames_for_full_heal: u32) -> f32 {
    if frames_for_full_heal == 0 || max_health <= 0.0 {
        return 0.0;
    }
    max_health / (frames_for_full_heal as f32)
}

/// Whether HealContain residual should force max-health completion this frame.
pub fn heal_contain_done_healing(contained_frames: u32, frames_for_full_heal: u32) -> bool {
    frames_for_full_heal > 0 && contained_frames >= frames_for_full_heal
}

/// Whether residual HealContain admits an infantry ally (barracks matrix).
pub fn heal_contain_can_accept(
    is_infantry: bool,
    is_ally: bool,
    is_enemy: bool,
    is_neutral: bool,
    current_count: i32,
    contain_max: i32,
) -> bool {
    if !is_infantry {
        return false;
    }
    if is_enemy && !HEAL_CONTAIN_BARRACKS_ALLOW_ENEMIES {
        return false;
    }
    if is_neutral && !HEAL_CONTAIN_BARRACKS_ALLOW_NEUTRAL {
        return false;
    }
    if is_ally && !HEAL_CONTAIN_BARRACKS_ALLOW_ALLIES {
        return false;
    }
    if !is_ally && !is_enemy && !is_neutral {
        return false;
    }
    if contain_max == CONTAIN_MAX_UNKNOWN_RESIDUAL {
        return true;
    }
    current_count < contain_max
}

/// Wave 98 honesty: contain residual deepen pack.
pub fn honesty_contain_residual_deepen_pack_wave98() -> bool {
    CONTAIN_MAX_UNKNOWN_RESIDUAL == -1
        && OPEN_CONTAIN_DEFAULT_BURNED_DEATH_TO_UNITS
        && !OPEN_CONTAIN_DEFAULT_PASSENGERS_IN_TURRET
        && !OPEN_CONTAIN_DEFAULT_WEAPON_BONUS_TO_PASSENGERS
        && OPEN_CONTAIN_DEFAULT_KICK_OUT_ON_CAPTURE
        && OPEN_CONTAIN_DEFAULT_IMMUNE_TO_CLEAR
        && !OPEN_CONTAIN_DEFAULT_IS_HEAL_CONTAIN
        && !OPEN_CONTAIN_DEFAULT_IS_GARRISONABLE
        && !OPEN_CONTAIN_DEFAULT_IS_BUSTABLE
        && !OPEN_CONTAIN_DEFAULT_IS_TUNNEL
        && !OPEN_CONTAIN_DEFAULT_DISPLAYED_ON_CONTROL_BAR
        && TRANSPORT_CONTAIN_DISPLAYED_ON_CONTROL_BAR
        && OPEN_CONTAIN_DEFAULT_ENCLOSING
        && OBJECT_ENTER_EXIT_TYPE_COUNT_RESIDUAL == 3
        && OBJECT_ENTER_EXIT_TYPE_NAME_TABLE_RESIDUAL.len() == 3
        && residual_name_index(
            OBJECT_ENTER_EXIT_TYPE_NAME_TABLE_RESIDUAL,
            "WANTS_TO_ENTER",
        ) == Some(0)
        && residual_name_index(
            OBJECT_ENTER_EXIT_TYPE_NAME_TABLE_RESIDUAL,
            "WANTS_TO_EXIT",
        ) == Some(1)
        && residual_name_index(
            OBJECT_ENTER_EXIT_TYPE_NAME_TABLE_RESIDUAL,
            "WANTS_NEITHER",
        ) == Some(2)
        && EVAC_DISPOSITION_COUNT_RESIDUAL == 4
        && EVAC_DISPOSITION_NAME_TABLE_RESIDUAL.len() == 4
        && residual_name_index(EVAC_DISPOSITION_NAME_TABLE_RESIDUAL, "EVAC_INVALID")
            == Some(0)
        && residual_name_index(
            EVAC_DISPOSITION_NAME_TABLE_RESIDUAL,
            "EVAC_BURST_FROM_CENTER",
        ) == Some(3)
        && HEAL_CONTAIN_IS_HEAL_CONTAIN_RESIDUAL
        && HEAL_CONTAIN_DEFAULT_FRAMES_FOR_FULL_HEAL == 0
        && HEAL_CONTAIN_BARRACKS_TIME_FOR_FULL_HEAL_MS == 2000
        && HEAL_CONTAIN_BARRACKS_TIME_FOR_FULL_HEAL_FRAMES
            == residual_ms_to_frames(HEAL_CONTAIN_BARRACKS_TIME_FOR_FULL_HEAL_MS)
        && HEAL_CONTAIN_BARRACKS_CONTAIN_MAX == 10
        && HEAL_CONTAIN_BARRACKS_ALLOW_ALLIES
        && !HEAL_CONTAIN_BARRACKS_ALLOW_NEUTRAL
        && !HEAL_CONTAIN_BARRACKS_ALLOW_ENEMIES
        && HEAL_CONTAIN_BARRACKS_ALLOW_KIND_OF == "INFANTRY"
        && !GARRISON_DEFAULT_HEAL_OBJECTS_RESIDUAL
        && (GARRISON_DEFAULT_FRAMES_FOR_FULL_HEAL_RESIDUAL - 1.0).abs() < 0.001
        // Sliver + done residual.
        && (heal_contain_sliver_amount(120.0, 60) - 2.0).abs() < 0.0001
        && heal_contain_sliver_amount(100.0, 0) == 0.0
        && !heal_contain_done_healing(59, 60)
        && heal_contain_done_healing(60, 60)
        && heal_contain_done_healing(61, 60)
        // Admission residual matrix.
        && heal_contain_can_accept(true, true, false, false, 0, 10)
        && heal_contain_can_accept(true, true, false, false, 9, 10)
        && !heal_contain_can_accept(true, true, false, false, 10, 10)
        && !heal_contain_can_accept(false, true, false, false, 0, 10)
        && !heal_contain_can_accept(true, false, true, false, 0, 10)
        && !heal_contain_can_accept(true, false, false, true, 0, 10)
}

// ---------------------------------------------------------------------------
// 3. Exit residual peels
// ---------------------------------------------------------------------------

/// C++ ExitDoorType residual (UpdateModule.h).
pub const EXIT_DOOR_1_RESIDUAL: i32 = 0;
pub const EXIT_DOOR_2_RESIDUAL: i32 = 1;
pub const EXIT_DOOR_3_RESIDUAL: i32 = 2;
pub const EXIT_DOOR_4_RESIDUAL: i32 = 3;
/// C++ DOOR_COUNT_MAX residual.
pub const EXIT_DOOR_COUNT_MAX_RESIDUAL: i32 = 4;
/// C++ DOOR_NONE_AVAILABLE residual.
pub const EXIT_DOOR_NONE_AVAILABLE_RESIDUAL: i32 = -1;
/// C++ DOOR_NONE_NEEDED residual.
pub const EXIT_DOOR_NONE_NEEDED_RESIDUAL: i32 = -2;

/// Ordered ExitDoorType residual names for doors 1..4.
pub const EXIT_DOOR_NAME_TABLE_RESIDUAL: &[&str] = &["DOOR_1", "DOOR_2", "DOOR_3", "DOOR_4"];

/// C++ OpenContain default NumberOfExitPaths residual.
pub const EXIT_DEFAULT_NUMBER_OF_EXIT_PATHS: i32 = 1;
/// C++ OpenContain default DoorOpenTime residual (frames).
pub const EXIT_DEFAULT_DOOR_OPEN_TIME_FRAMES: u32 = 1;
/// C++ OpenContain default reserveDoorForExit residual (DOOR_1).
pub const EXIT_OPEN_CONTAIN_DEFAULT_RESERVE_DOOR: i32 = EXIT_DOOR_1_RESIDUAL;
/// C++ OpenContain isExitBusy residual default (FALSE).
pub const EXIT_OPEN_CONTAIN_DEFAULT_IS_BUSY: bool = false;

/// C++ QueueProductionExitUpdateModuleData default ExitDelay residual (frames).
pub const QUEUE_EXIT_DEFAULT_EXIT_DELAY_FRAMES: u32 = 0;
/// C++ QueueProductionExitUpdateModuleData default AllowAirborneCreation residual.
pub const QUEUE_EXIT_DEFAULT_ALLOW_AIRBORNE: bool = false;
/// C++ QueueProductionExitUpdateModuleData default InitialBurst residual.
pub const QUEUE_EXIT_DEFAULT_INITIAL_BURST: u32 = 0;
/// Retail ChinaBarracks QueueProductionExitUpdate ExitDelay residual (msec).
pub const QUEUE_EXIT_CHINA_BARRACKS_EXIT_DELAY_MS: u32 = 300;
/// ChinaBarracks ExitDelay 300ms → 9 frames.
pub const QUEUE_EXIT_CHINA_BARRACKS_EXIT_DELAY_FRAMES: u32 = 9;

/// Transport residual ExitDelay samples deepen (beyond Wave 87 Humvee/Chinook):
/// TroopCrawler-style **500**ms residual used on some GLA/China vehicles.
pub const TRANSPORT_EXIT_DELAY_SAMPLE_MS: u32 = 500;
/// 500ms → 15 frames.
pub const TRANSPORT_EXIT_DELAY_SAMPLE_FRAMES: u32 = 15;

/// NUM_MODELCONDITION_DOOR_STATES residual (ModelState.h) — 4 phases per door.
pub const NUM_MODELCONDITION_DOOR_STATES_RESIDUAL: u32 = 4;

/// Transport isExitBusy residual: busy while exit countdown > 0.
pub fn transport_exit_is_busy(exit_countdown_frames: u32) -> bool {
    exit_countdown_frames > 0
}

/// Advance exit countdown residual one logic frame (floor at 0).
pub fn transport_exit_countdown_tick(exit_countdown_frames: u32) -> u32 {
    exit_countdown_frames.saturating_sub(1)
}

/// Whether ExitDoorType residual is a real door slot (0..COUNT_MAX-1).
pub fn exit_door_is_real(door: i32) -> bool {
    door >= EXIT_DOOR_1_RESIDUAL && door < EXIT_DOOR_COUNT_MAX_RESIDUAL
}

/// Wave 98 honesty: exit residual pack.
pub fn honesty_exit_residual_pack_wave98() -> bool {
    EXIT_DOOR_1_RESIDUAL == 0
        && EXIT_DOOR_2_RESIDUAL == 1
        && EXIT_DOOR_3_RESIDUAL == 2
        && EXIT_DOOR_4_RESIDUAL == 3
        && EXIT_DOOR_COUNT_MAX_RESIDUAL == 4
        && EXIT_DOOR_NONE_AVAILABLE_RESIDUAL == -1
        && EXIT_DOOR_NONE_NEEDED_RESIDUAL == -2
        && EXIT_DOOR_NAME_TABLE_RESIDUAL.len() == 4
        && residual_name_index(EXIT_DOOR_NAME_TABLE_RESIDUAL, "DOOR_1") == Some(0)
        && residual_name_index(EXIT_DOOR_NAME_TABLE_RESIDUAL, "DOOR_4") == Some(3)
        && EXIT_DEFAULT_NUMBER_OF_EXIT_PATHS == 1
        && EXIT_DEFAULT_DOOR_OPEN_TIME_FRAMES == 1
        && EXIT_OPEN_CONTAIN_DEFAULT_RESERVE_DOOR == 0
        && !EXIT_OPEN_CONTAIN_DEFAULT_IS_BUSY
        && QUEUE_EXIT_DEFAULT_EXIT_DELAY_FRAMES == 0
        && !QUEUE_EXIT_DEFAULT_ALLOW_AIRBORNE
        && QUEUE_EXIT_DEFAULT_INITIAL_BURST == 0
        && QUEUE_EXIT_CHINA_BARRACKS_EXIT_DELAY_MS == 300
        && QUEUE_EXIT_CHINA_BARRACKS_EXIT_DELAY_FRAMES
            == residual_ms_to_frames(QUEUE_EXIT_CHINA_BARRACKS_EXIT_DELAY_MS)
        && TRANSPORT_EXIT_DELAY_SAMPLE_MS == 500
        && TRANSPORT_EXIT_DELAY_SAMPLE_FRAMES
            == residual_ms_to_frames(TRANSPORT_EXIT_DELAY_SAMPLE_MS)
        && NUM_MODELCONDITION_DOOR_STATES_RESIDUAL == 4
        && exit_door_is_real(0)
        && exit_door_is_real(3)
        && !exit_door_is_real(-1)
        && !exit_door_is_real(-2)
        && !exit_door_is_real(4)
        && !transport_exit_is_busy(0)
        && transport_exit_is_busy(1)
        && transport_exit_is_busy(9)
        && transport_exit_countdown_tick(9) == 8
        && transport_exit_countdown_tick(0) == 0
}

// ---------------------------------------------------------------------------
// 4. Heal residual deepen (beyond Wave 71 ambulance + Wave 81 AutoFindHealing)
// ---------------------------------------------------------------------------

/// C++ AutoHealBehaviorModuleData defaults residual.
pub const AUTO_HEAL_DEFAULT_INITIALLY_ACTIVE: bool = false;
pub const AUTO_HEAL_DEFAULT_SINGLE_BURST: bool = false;
pub const AUTO_HEAL_DEFAULT_HEALING_AMOUNT: i32 = 0;
/// C++ default HealingDelay residual (UINT_MAX → never until INI sets).
pub const AUTO_HEAL_DEFAULT_HEALING_DELAY_FRAMES: u32 = u32::MAX;
pub const AUTO_HEAL_DEFAULT_START_HEALING_DELAY_FRAMES: u32 = 0;
pub const AUTO_HEAL_DEFAULT_RADIUS: f32 = 0.0;
pub const AUTO_HEAL_DEFAULT_AFFECTS_WHOLE_PLAYER: bool = false;
pub const AUTO_HEAL_DEFAULT_SKIP_SELF: bool = false;

/// C++ AutoFindHealingUpdateModuleData ctor defaults residual (pre-INI).
pub const AUTO_FIND_HEALING_CTOR_SCAN_FRAMES: u32 = 0;
pub const AUTO_FIND_HEALING_CTOR_SCAN_RANGE: f32 = 0.0;
/// C++ default NeverHeal residual (**0.95** — retail infantry override **0.85**).
pub const AUTO_FIND_HEALING_CTOR_NEVER_HEAL: f32 = 0.95;
/// C++ default AlwaysHeal residual (**0.25** — matches retail).
pub const AUTO_FIND_HEALING_CTOR_ALWAYS_HEAL: f32 = 0.25;

/// Retail infantry AutoFindHealing residual (already host-seeded; re-anchored here).
pub const AUTO_FIND_HEALING_RETAIL_SCAN_MS: u32 = 1000;
pub const AUTO_FIND_HEALING_RETAIL_SCAN_FRAMES: u32 = 30;
pub const AUTO_FIND_HEALING_RETAIL_SCAN_RANGE: f32 = 300.0;
pub const AUTO_FIND_HEALING_RETAIL_NEVER_HEAL: f32 = 0.85;
pub const AUTO_FIND_HEALING_RETAIL_ALWAYS_HEAL: f32 = 0.25;

/// C++ ParkingPlaceBehaviorModuleData default HealAmountPerSecond residual.
pub const PARKING_PLACE_DEFAULT_HEAL_AMOUNT_PER_SEC: f32 = 0.0;
/// Retail AmericaAirfield ParkingPlaceBehavior HealAmountPerSecond residual.
pub const PARKING_PLACE_AIRFIELD_HEAL_AMOUNT_PER_SEC: f32 = 10.0;
/// Retail AmericaAirfield NumRows residual.
pub const PARKING_PLACE_AIRFIELD_NUM_ROWS: i32 = 2;
/// Retail AmericaAirfield NumCols residual.
pub const PARKING_PLACE_AIRFIELD_NUM_COLS: i32 = 2;
/// Retail AmericaAirfield HasRunways residual.
pub const PARKING_PLACE_AIRFIELD_HAS_RUNWAYS: bool = true;
/// Retail AmericaAirfield ApproachHeight residual.
pub const PARKING_PLACE_AIRFIELD_APPROACH_HEIGHT: f32 = 50.0;

/// Retail airfield runway count residual (HasRunways ? NumCols : 0).
pub fn airfield_runway_count(has_runways: bool, num_cols: i32) -> usize {
    if !has_runways {
        return 0;
    }
    num_cols.max(0) as usize
}

/// C++ ParkingPlaceBehavior APPROACH_DIST residual (runway length scalar past end).
pub const PARKING_PLACE_RUNWAY_APPROACH_DIST: f32 = 0.75;

/// Runway prep offset residual along +X from airfield center (host simplified bone).
pub const PARKING_PLACE_RUNWAY_PREP_SPACING: f32 = 40.0;

/// Host runway takeoff clear distance residual (release reservation past this 2D range).
pub const PARKING_PLACE_RUNWAY_CLEAR_DIST: f32 = 120.0;

/// Helix-style ParkingPlace HealAmountPerSecond residual sample (**20**).
pub const PARKING_PLACE_HELIX_HEAL_AMOUNT_PER_SEC: f32 = 20.0;

/// DamageType residual token for heal attempt (Damage.h DAMAGE_HEALING).
pub const HEAL_DAMAGE_TYPE_NAME_RESIDUAL: &str = "HEALING";
/// DeathType residual for heal attempt (DEATH_NONE).
pub const HEAL_DEATH_TYPE_NAME_RESIDUAL: &str = "NONE";

/// Retail AmericaAirfield hangar capacity residual (NumRows × NumCols = **4**).
pub fn airfield_parking_place_capacity() -> u32 {
    (PARKING_PLACE_AIRFIELD_NUM_ROWS.max(0) * PARKING_PLACE_AIRFIELD_NUM_COLS.max(0)) as u32
}

/// True when airfield parking is full for another aircraft residual.
pub fn airfield_parking_places_full(occupied_or_queued: u32) -> bool {
    occupied_or_queued >= airfield_parking_place_capacity()
}

/// Parking-place residual HP/sec → per-frame amount.
pub fn parking_place_heal_per_frame(heal_amount_per_sec: f32) -> f32 {
    heal_amount_per_sec / DOCK_CONTAIN_EXIT_HEAL_LOGIC_FPS
}

/// AutoFindHealing residual: whether unit should seek heal based on health fraction.
///
/// Retail NeverHeal: do not seek when health_frac > never_heal.
/// AlwaysHeal constant retained (C++ AlwaysHeal busy-interrupt is dead code; host idle-only).
pub fn auto_find_healing_should_seek(
    health_frac: f32,
    never_heal: f32,
    _always_heal: f32,
    is_ai: bool,
    is_idle: bool,
) -> bool {
    if !is_ai || !is_idle {
        return false;
    }
    if health_frac <= 0.0 {
        return false;
    }
    health_frac <= never_heal
}

/// Wave 98 honesty: heal residual deepen pack.
pub fn honesty_heal_residual_deepen_pack_wave98() -> bool {
    !AUTO_HEAL_DEFAULT_INITIALLY_ACTIVE
        && !AUTO_HEAL_DEFAULT_SINGLE_BURST
        && AUTO_HEAL_DEFAULT_HEALING_AMOUNT == 0
        && AUTO_HEAL_DEFAULT_HEALING_DELAY_FRAMES == u32::MAX
        && AUTO_HEAL_DEFAULT_START_HEALING_DELAY_FRAMES == 0
        && (AUTO_HEAL_DEFAULT_RADIUS - 0.0).abs() < 0.001
        && !AUTO_HEAL_DEFAULT_AFFECTS_WHOLE_PLAYER
        && !AUTO_HEAL_DEFAULT_SKIP_SELF
        && AUTO_FIND_HEALING_CTOR_SCAN_FRAMES == 0
        && (AUTO_FIND_HEALING_CTOR_SCAN_RANGE - 0.0).abs() < 0.001
        && (AUTO_FIND_HEALING_CTOR_NEVER_HEAL - 0.95).abs() < 0.001
        && (AUTO_FIND_HEALING_CTOR_ALWAYS_HEAL - 0.25).abs() < 0.001
        && AUTO_FIND_HEALING_RETAIL_SCAN_MS == 1000
        && AUTO_FIND_HEALING_RETAIL_SCAN_FRAMES
            == residual_ms_to_frames(AUTO_FIND_HEALING_RETAIL_SCAN_MS)
        && (AUTO_FIND_HEALING_RETAIL_SCAN_RANGE - 300.0).abs() < 0.001
        && (AUTO_FIND_HEALING_RETAIL_NEVER_HEAL - 0.85).abs() < 0.001
        && (AUTO_FIND_HEALING_RETAIL_ALWAYS_HEAL - 0.25).abs() < 0.001
        // Ctor default NeverHeal is stricter than retail infantry NeverHeal.
        && AUTO_FIND_HEALING_CTOR_NEVER_HEAL > AUTO_FIND_HEALING_RETAIL_NEVER_HEAL
        && (PARKING_PLACE_DEFAULT_HEAL_AMOUNT_PER_SEC - 0.0).abs() < 0.001
        && (PARKING_PLACE_AIRFIELD_HEAL_AMOUNT_PER_SEC - 10.0).abs() < 0.001
        && PARKING_PLACE_AIRFIELD_NUM_ROWS == 2
        && PARKING_PLACE_AIRFIELD_NUM_COLS == 2
        && PARKING_PLACE_AIRFIELD_HAS_RUNWAYS
        && (PARKING_PLACE_AIRFIELD_APPROACH_HEIGHT - 50.0).abs() < 0.001
        && (PARKING_PLACE_HELIX_HEAL_AMOUNT_PER_SEC - 20.0).abs() < 0.001
        && HEAL_DAMAGE_TYPE_NAME_RESIDUAL == "HEALING"
        && HEAL_DEATH_TYPE_NAME_RESIDUAL == "NONE"
        && (parking_place_heal_per_frame(10.0) - (10.0 / 30.0)).abs() < 0.0001
        && (parking_place_heal_per_frame(20.0) - (20.0 / 30.0)).abs() < 0.0001
        // AutoFindHealing residual decision matrix.
        && auto_find_healing_should_seek(0.5, 0.85, 0.25, true, true)
        && auto_find_healing_should_seek(0.85, 0.85, 0.25, true, true)
        && !auto_find_healing_should_seek(0.86, 0.85, 0.25, true, true)
        && !auto_find_healing_should_seek(0.5, 0.85, 0.25, false, true)
        && !auto_find_healing_should_seek(0.5, 0.85, 0.25, true, false)
        // Cross-link repair-dock / heal-contain formulas from packs 1–2.
        && (repair_dock_health_to_add_per_frame(500.0, 0.0, 150.0) - (500.0 / 150.0)).abs()
            < 0.0001
        && (heal_contain_sliver_amount(200.0, 60) - (200.0 / 60.0)).abs() < 0.0001
}

// ---------------------------------------------------------------------------
// Combined Wave 98 pack
// ---------------------------------------------------------------------------

/// Combined honesty pack for Wave 98 dock / contain / exit / heal residual peels.
pub fn honesty_dock_contain_exit_heal_residual_pack_wave98() -> bool {
    honesty_dock_residual_pack_wave98()
        && honesty_contain_residual_deepen_pack_wave98()
        && honesty_exit_residual_pack_wave98()
        && honesty_heal_residual_deepen_pack_wave98()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dock_residual_wave98_honesty() {
        assert!(honesty_dock_residual_pack_wave98());
    }

    #[test]
    fn contain_residual_deepen_wave98_honesty() {
        assert!(honesty_contain_residual_deepen_pack_wave98());
    }

    #[test]
    fn exit_residual_wave98_honesty() {
        assert!(honesty_exit_residual_pack_wave98());
    }

    #[test]
    fn heal_residual_deepen_wave98_honesty() {
        assert!(honesty_heal_residual_deepen_pack_wave98());
    }

    #[test]
    fn dock_contain_exit_heal_residual_pack_wave98_honesty() {
        assert!(honesty_dock_contain_exit_heal_residual_pack_wave98());
    }
}
