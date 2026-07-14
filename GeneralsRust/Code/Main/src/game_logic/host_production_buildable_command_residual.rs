//! Wave 99 residual peels: Production residual deepen / Buildable residual peels /
//! Prerequisite residual peels / CommandButton residual deepen / ControlBar residual deepen.
//!
//! Orthogonal to Waves 76 (ControlBar window/font), 80 (superweapon CommandButton labels),
//! 83 (production queue energy/refund/CC door), and 94 (CommandSet superweapon names).
//! Host-testable packs for production / buildable / prereq / command-button / control-bar residual.
//!
//! Sources (retail ZH C++ / INI):
//! - ProductionUpdate.h/.cpp ProductionType / MaxQueueEntries **9** / Door residual / QuantityModifier
//! - BuildAssistant.h CanMakeType / LegalBuildCode / TOTAL_FRAMES_TO_SELL_OBJECT **90**
//! - ThingTemplate.h BuildableStatus / BuildCompletionType residual tables
//! - ProductionPrerequisite.h MAX_PREREQ **32** / UNIT_OR_WITH_PREV **0x01**
//! - ControlBar.h GUICommandType / CommandOption / CommandButtonMappedBorderType /
//!   ControlBarContext / MAX_COMMANDS_PER_SET **18** / MAX_BUILD_QUEUE_BUTTONS **9**
//! - GameData.ini SellPercentage **50%** (override of C++ ctor 1.0)
//!
//! Fail-closed:
//! - Not full ProductionUpdate door-anim / parking-place live queue residual
//! - Not full BuildAssistant isLocationLegalToBuild terrain/shroud graph
//! - Not full ProductionPrerequisite live player-owned unit scan residual
//! - Not full CommandButton INI parse / science-swap cameo matrix residual
//! - Not full ControlBar DrawCallback / windowed W3D retail UI residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Logic frames per second residual (host fixed step).
pub const PROD_BUILD_CMD_LOGIC_FPS: f32 = 30.0;

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

// ---------------------------------------------------------------------------
// 1. Production residual deepen (beyond Wave 83 queue energy/refund/door times)
// ---------------------------------------------------------------------------

/// C++ `ProductionType` residual: PRODUCTION_INVALID **0**.
pub const PRODUCTION_TYPE_INVALID: u32 = 0;
/// C++ PRODUCTION_UNIT residual.
pub const PRODUCTION_TYPE_UNIT: u32 = 1;
/// C++ PRODUCTION_UPGRADE residual.
pub const PRODUCTION_TYPE_UPGRADE: u32 = 2;

/// Ordered C++ `ProductionType` residual names.
pub const PRODUCTION_TYPE_NAME_TABLE_RESIDUAL: &[&str] = &[
    "PRODUCTION_INVALID",
    "PRODUCTION_UNIT",
    "PRODUCTION_UPGRADE",
];

/// C++ ProductionUpdateModuleData default MaxQueueEntries residual.
pub const PRODUCTION_MAX_QUEUE_ENTRIES_DEEPEN: usize = 9;

/// C++ ProductionUpdateModuleData default NumDoorAnimations residual.
pub const PRODUCTION_NUM_DOOR_ANIMATIONS_DEFAULT: i32 = 0;
/// C++ door open/wait/close / construction-complete duration defaults residual (frames).
pub const PRODUCTION_DOOR_OPENING_TIME_DEFAULT: u32 = 0;
pub const PRODUCTION_DOOR_WAIT_OPEN_TIME_DEFAULT: u32 = 0;
pub const PRODUCTION_DOOR_CLOSING_TIME_DEFAULT: u32 = 0;
pub const PRODUCTION_CONSTRUCTION_COMPLETE_DURATION_DEFAULT: u32 = 0;

/// C++ `ExitDoorType` residual: DOOR_1..DOOR_4 / DOOR_COUNT_MAX / sentinels.
pub const EXIT_DOOR_1: i32 = 0;
pub const EXIT_DOOR_2: i32 = 1;
pub const EXIT_DOOR_3: i32 = 2;
pub const EXIT_DOOR_4: i32 = 3;
pub const EXIT_DOOR_COUNT_MAX: i32 = 4;
pub const EXIT_DOOR_NONE_AVAILABLE: i32 = -1;
pub const EXIT_DOOR_NONE_NEEDED: i32 = -2;

/// Ordered ExitDoor residual names (for door indices 0..3).
pub const EXIT_DOOR_NAME_TABLE_RESIDUAL: &[&str] = &["DOOR_1", "DOOR_2", "DOOR_3", "DOOR_4"];

/// C++ ProductionID invalid residual.
pub const PRODUCTION_ID_INVALID: u32 = 0;

/// C++ ProductionEntry ctor residual: productionID starts at **1**.
pub const PRODUCTION_ENTRY_ID_CTOR_RESIDUAL: u32 = 1;
/// C++ ProductionEntry ctor residual: percentComplete **0.0**.
pub const PRODUCTION_ENTRY_PERCENT_COMPLETE_CTOR: f32 = 0.0;
/// C++ ProductionEntry ctor residual: framesUnderConstruction **0**.
pub const PRODUCTION_ENTRY_FRAMES_UNDER_CONSTRUCTION_CTOR: i32 = 0;
/// C++ ProductionEntry ctor residual: productionQuantityTotal/Produced **0**.
pub const PRODUCTION_ENTRY_QUANTITY_CTOR: i32 = 0;

/// C++ ProductionUpdateModuleData default DisabledTypesToProcess residual bit:
/// MAKE_DISABLED_MASK(DISABLED_HELD) — DISABLED_HELD ordinal **3** → bit **0x08**.
pub const PRODUCTION_DISABLED_HELD_ORDINAL: u32 = 3;
pub const PRODUCTION_DISABLED_TYPES_TO_PROCESS_HELD_BIT: u32 =
    1 << PRODUCTION_DISABLED_HELD_ORDINAL;

/// ProductionUpdate INI field residual names (buildFieldParse).
pub const PRODUCTION_UPDATE_INI_FIELD_NAMES: &[&str] = &[
    "MaxQueueEntries",
    "NumDoorAnimations",
    "DoorOpeningTime",
    "DoorWaitOpenTime",
    "DoorCloseTime",
    "ConstructionCompleteDuration",
    "QuantityModifier",
    "DisabledTypesToProcess",
];

/// Retail QuantityModifier sample residual (China barracks: Red Guard ×2).
pub const QUANTITY_MODIFIER_SAMPLE_TEMPLATE: &str = "ChinaInfantryRedguard";
pub const QUANTITY_MODIFIER_SAMPLE_COUNT: i32 = 2;
/// QuantityModifier parse default count residual when count token absent.
pub const QUANTITY_MODIFIER_DEFAULT_COUNT: i32 = 1;

/// C++ `BuildCompletionType` residual count (BC_NUM_TYPES).
pub const BUILD_COMPLETION_NUM_TYPES: usize = 3;
/// Ordered BuildCompletionNames residual.
pub const BUILD_COMPLETION_NAME_TABLE_RESIDUAL: &[&str] = &[
    "INVALID",                // 0 BC_INVALID
    "APPEARS_AT_RALLY_POINT", // 1
    "PLACED_BY_PLAYER",       // 2
];
pub const BC_INVALID: u32 = 0;
pub const BC_APPEARS_AT_RALLY_POINT: u32 = 1;
pub const BC_PLACED_BY_PLAYER: u32 = 2;

/// C++ CONSTRUCTION_COMPLETE residual (BuildAssistant.h / Object construction percent).
pub const CONSTRUCTION_COMPLETE_PERCENT: i32 = -1;

/// calcTimeToBuild residual: buildTime seconds × LOGIC_FPS, then / power_factor.
#[inline]
pub fn production_calc_time_to_build_frames_residual(
    build_time_seconds: f32,
    energy_ratio: f32,
    min_low_energy_speed: f32,
    max_low_energy_speed: f32,
    low_energy_penalty_modifier: f32,
) -> u32 {
    let mut build_time = (build_time_seconds * PROD_BUILD_CMD_LOGIC_FPS).round() as f32;
    let ratio = energy_ratio.clamp(0.0, 1.0);
    if ratio < 1.0 {
        let energy_short = (1.0 - ratio) * low_energy_penalty_modifier;
        let mut penalty_rate = (1.0 - energy_short).max(min_low_energy_speed);
        penalty_rate = penalty_rate.min(max_low_energy_speed);
        if penalty_rate <= 0.0 {
            penalty_rate = 0.01;
        }
        build_time /= penalty_rate;
    }
    build_time.round() as u32
}

/// Production percent residual from framesUnderConstruction / totalFrames.
#[inline]
pub fn production_percent_complete_residual(frames: i32, total_frames: i32) -> f32 {
    if total_frames <= 0 {
        return 100.0;
    }
    ((frames as f32) / (total_frames as f32) * 100.0).min(100.0)
}

/// Wave 99 honesty: production residual deepen pack.
pub fn honesty_production_residual_deepen_pack_wave99() -> bool {
    PRODUCTION_TYPE_INVALID == 0
        && PRODUCTION_TYPE_UNIT == 1
        && PRODUCTION_TYPE_UPGRADE == 2
        && PRODUCTION_TYPE_NAME_TABLE_RESIDUAL.len() == 3
        && residual_name_index(PRODUCTION_TYPE_NAME_TABLE_RESIDUAL, "PRODUCTION_UNIT") == Some(1)
        && residual_name_index(PRODUCTION_TYPE_NAME_TABLE_RESIDUAL, "PRODUCTION_UPGRADE")
            == Some(2)
        && PRODUCTION_MAX_QUEUE_ENTRIES_DEEPEN == 9
        && PRODUCTION_NUM_DOOR_ANIMATIONS_DEFAULT == 0
        && PRODUCTION_DOOR_OPENING_TIME_DEFAULT == 0
        && PRODUCTION_DOOR_WAIT_OPEN_TIME_DEFAULT == 0
        && PRODUCTION_DOOR_CLOSING_TIME_DEFAULT == 0
        && PRODUCTION_CONSTRUCTION_COMPLETE_DURATION_DEFAULT == 0
        && EXIT_DOOR_1 == 0
        && EXIT_DOOR_4 == 3
        && EXIT_DOOR_COUNT_MAX == 4
        && EXIT_DOOR_NONE_AVAILABLE == -1
        && EXIT_DOOR_NONE_NEEDED == -2
        && EXIT_DOOR_NAME_TABLE_RESIDUAL.len() == 4
        && PRODUCTION_ID_INVALID == 0
        && PRODUCTION_ENTRY_ID_CTOR_RESIDUAL == 1
        && (PRODUCTION_ENTRY_PERCENT_COMPLETE_CTOR - 0.0).abs() < 1e-6
        && PRODUCTION_ENTRY_FRAMES_UNDER_CONSTRUCTION_CTOR == 0
        && PRODUCTION_ENTRY_QUANTITY_CTOR == 0
        && PRODUCTION_DISABLED_HELD_ORDINAL == 3
        && PRODUCTION_DISABLED_TYPES_TO_PROCESS_HELD_BIT == 0x08
        && PRODUCTION_UPDATE_INI_FIELD_NAMES.len() == 8
        && residual_name_index(PRODUCTION_UPDATE_INI_FIELD_NAMES, "MaxQueueEntries") == Some(0)
        && residual_name_index(PRODUCTION_UPDATE_INI_FIELD_NAMES, "QuantityModifier")
            == Some(6)
        && QUANTITY_MODIFIER_SAMPLE_TEMPLATE == "ChinaInfantryRedguard"
        && QUANTITY_MODIFIER_SAMPLE_COUNT == 2
        && QUANTITY_MODIFIER_DEFAULT_COUNT == 1
        && BUILD_COMPLETION_NUM_TYPES == 3
        && BUILD_COMPLETION_NAME_TABLE_RESIDUAL.len() == 3
        && residual_name_index(BUILD_COMPLETION_NAME_TABLE_RESIDUAL, "APPEARS_AT_RALLY_POINT")
            == Some(1)
        && residual_name_index(BUILD_COMPLETION_NAME_TABLE_RESIDUAL, "PLACED_BY_PLAYER")
            == Some(2)
        && BC_APPEARS_AT_RALLY_POINT == 1
        && BC_PLACED_BY_PLAYER == 2
        && CONSTRUCTION_COMPLETE_PERCENT == -1
        // 5s build, full energy → 150 frames
        && production_calc_time_to_build_frames_residual(5.0, 1.0, 0.5, 0.8, 1.0) == 150
        // 5s build, 0 energy → rate 0.5 → 300 frames
        && production_calc_time_to_build_frames_residual(5.0, 0.0, 0.5, 0.8, 1.0) == 300
        && (production_percent_complete_residual(45, 150) - 30.0).abs() < 1e-3
        && (production_percent_complete_residual(150, 150) - 100.0).abs() < 1e-3
        && (production_percent_complete_residual(0, 0) - 100.0).abs() < 1e-3
}

// ---------------------------------------------------------------------------
// 2. Buildable residual peels (BuildableStatus / CanMakeType / LegalBuildCode)
// ---------------------------------------------------------------------------

/// C++ `BuildableStatus` residual count (BSTATUS_NUM_TYPES).
pub const BUILDABLE_STATUS_NUM_TYPES: usize = 4;

/// Ordered C++ BuildableStatusNames residual.
pub const BUILDABLE_STATUS_NAME_TABLE_RESIDUAL: &[&str] = &[
    "Yes",                  // 0 BSTATUS_YES
    "Ignore_Prerequisites", // 1
    "No",                   // 2
    "Only_By_AI",           // 3
];

pub const BSTATUS_YES: u32 = 0;
pub const BSTATUS_IGNORE_PREREQUISITES: u32 = 1;
pub const BSTATUS_NO: u32 = 2;
pub const BSTATUS_ONLY_BY_AI: u32 = 3;

/// C++ `CanMakeType` residual ordered names.
pub const CAN_MAKE_TYPE_NAME_TABLE_RESIDUAL: &[&str] = &[
    "CANMAKE_OK",                   // 0
    "CANMAKE_NO_PREREQ",            // 1
    "CANMAKE_NO_MONEY",             // 2
    "CANMAKE_FACTORY_IS_DISABLED",  // 3
    "CANMAKE_QUEUE_FULL",           // 4
    "CANMAKE_PARKING_PLACES_FULL",  // 5
    "CANMAKE_MAXED_OUT_FOR_PLAYER", // 6
];

pub const CANMAKE_OK: u32 = 0;
pub const CANMAKE_NO_PREREQ: u32 = 1;
pub const CANMAKE_NO_MONEY: u32 = 2;
pub const CANMAKE_FACTORY_IS_DISABLED: u32 = 3;
pub const CANMAKE_QUEUE_FULL: u32 = 4;
pub const CANMAKE_PARKING_PLACES_FULL: u32 = 5;
pub const CANMAKE_MAXED_OUT_FOR_PLAYER: u32 = 6;

/// C++ `LegalBuildCode` residual ordered names.
pub const LEGAL_BUILD_CODE_NAME_TABLE_RESIDUAL: &[&str] = &[
    "LBC_OK",                    // 0
    "LBC_RESTRICTED_TERRAIN",    // 1
    "LBC_NOT_FLAT_ENOUGH",       // 2
    "LBC_OBJECTS_IN_THE_WAY",    // 3
    "LBC_NO_CLEAR_PATH",         // 4
    "LBC_SHROUD",                // 5
    "LBC_TOO_CLOSE_TO_SUPPLIES", // 6
    "LBC_GENERIC_FAILURE",       // 7
];

pub const LBC_OK: u32 = 0;
pub const LBC_RESTRICTED_TERRAIN: u32 = 1;
pub const LBC_NOT_FLAT_ENOUGH: u32 = 2;
pub const LBC_OBJECTS_IN_THE_WAY: u32 = 3;
pub const LBC_NO_CLEAR_PATH: u32 = 4;
pub const LBC_SHROUD: u32 = 5;
pub const LBC_TOO_CLOSE_TO_SUPPLIES: u32 = 6;
pub const LBC_GENERIC_FAILURE: u32 = 7;

/// C++ BuildAssistant LocalLegalToBuildOptions residual bits.
pub const LOCAL_LEGAL_TERRAIN_RESTRICTIONS: u32 = 0x0000_0001;
pub const LOCAL_LEGAL_CLEAR_PATH: u32 = 0x0000_0002;
pub const LOCAL_LEGAL_NO_OBJECT_OVERLAP: u32 = 0x0000_0004;
pub const LOCAL_LEGAL_USE_QUICK_PATHFIND: u32 = 0x0000_0008;
pub const LOCAL_LEGAL_SHROUD_REVEALED: u32 = 0x0000_0010;
pub const LOCAL_LEGAL_NO_ENEMY_OBJECT_OVERLAP: u32 = 0x0000_0020;
pub const LOCAL_LEGAL_IGNORE_STEALTHED: u32 = 0x0000_0040;
pub const LOCAL_LEGAL_FAIL_STEALTHED_WITHOUT_FEEDBACK: u32 = 0x0000_0080;

/// C++ TOTAL_FRAMES_TO_SELL_OBJECT residual (LOGICFRAMES_PER_SECOND * 3.0 = 90).
pub const TOTAL_FRAMES_TO_SELL_OBJECT_RESIDUAL: u32 = 90;
/// Retail GameData.ini SellPercentage residual (50%; C++ GlobalData ctor default 1.0).
pub const SELL_PERCENTAGE_RESIDUAL: f32 = 0.5;

/// Whether buildable status allows human player residual (Player / ControlBar).
#[inline]
pub fn buildable_status_allows_human_residual(status: u32) -> bool {
    matches!(status, BSTATUS_YES | BSTATUS_IGNORE_PREREQUISITES)
}

/// Whether buildable status skips prerequisite residual.
#[inline]
pub fn buildable_status_ignores_prereq_residual(status: u32) -> bool {
    status == BSTATUS_IGNORE_PREREQUISITES
}

/// Sell refund residual (cost × SellPercentage, floor).
#[inline]
pub fn sell_refund_residual(cost: u32) -> u32 {
    ((cost as f32) * SELL_PERCENTAGE_RESIDUAL).floor() as u32
}

/// Wave 99 honesty: buildable residual pack.
pub fn honesty_buildable_residual_pack_wave99() -> bool {
    BUILDABLE_STATUS_NUM_TYPES == 4
        && BUILDABLE_STATUS_NAME_TABLE_RESIDUAL.len() == 4
        && residual_name_index(BUILDABLE_STATUS_NAME_TABLE_RESIDUAL, "Yes") == Some(0)
        && residual_name_index(BUILDABLE_STATUS_NAME_TABLE_RESIDUAL, "Ignore_Prerequisites")
            == Some(1)
        && residual_name_index(BUILDABLE_STATUS_NAME_TABLE_RESIDUAL, "No") == Some(2)
        && residual_name_index(BUILDABLE_STATUS_NAME_TABLE_RESIDUAL, "Only_By_AI") == Some(3)
        && BSTATUS_YES == 0
        && BSTATUS_IGNORE_PREREQUISITES == 1
        && BSTATUS_NO == 2
        && BSTATUS_ONLY_BY_AI == 3
        && buildable_status_allows_human_residual(BSTATUS_YES)
        && buildable_status_allows_human_residual(BSTATUS_IGNORE_PREREQUISITES)
        && !buildable_status_allows_human_residual(BSTATUS_NO)
        && !buildable_status_allows_human_residual(BSTATUS_ONLY_BY_AI)
        && buildable_status_ignores_prereq_residual(BSTATUS_IGNORE_PREREQUISITES)
        && !buildable_status_ignores_prereq_residual(BSTATUS_YES)
        && CAN_MAKE_TYPE_NAME_TABLE_RESIDUAL.len() == 7
        && residual_name_index(CAN_MAKE_TYPE_NAME_TABLE_RESIDUAL, "CANMAKE_OK") == Some(0)
        && residual_name_index(CAN_MAKE_TYPE_NAME_TABLE_RESIDUAL, "CANMAKE_NO_PREREQ") == Some(1)
        && residual_name_index(CAN_MAKE_TYPE_NAME_TABLE_RESIDUAL, "CANMAKE_NO_MONEY") == Some(2)
        && residual_name_index(CAN_MAKE_TYPE_NAME_TABLE_RESIDUAL, "CANMAKE_QUEUE_FULL") == Some(4)
        && residual_name_index(
            CAN_MAKE_TYPE_NAME_TABLE_RESIDUAL,
            "CANMAKE_MAXED_OUT_FOR_PLAYER",
        ) == Some(6)
        && CANMAKE_OK == 0
        && CANMAKE_QUEUE_FULL == 4
        && CANMAKE_PARKING_PLACES_FULL == 5
        && LEGAL_BUILD_CODE_NAME_TABLE_RESIDUAL.len() == 8
        && residual_name_index(LEGAL_BUILD_CODE_NAME_TABLE_RESIDUAL, "LBC_OK") == Some(0)
        && residual_name_index(LEGAL_BUILD_CODE_NAME_TABLE_RESIDUAL, "LBC_SHROUD") == Some(5)
        && residual_name_index(
            LEGAL_BUILD_CODE_NAME_TABLE_RESIDUAL,
            "LBC_TOO_CLOSE_TO_SUPPLIES",
        ) == Some(6)
        && residual_name_index(LEGAL_BUILD_CODE_NAME_TABLE_RESIDUAL, "LBC_GENERIC_FAILURE")
            == Some(7)
        && LOCAL_LEGAL_TERRAIN_RESTRICTIONS == 0x01
        && LOCAL_LEGAL_CLEAR_PATH == 0x02
        && LOCAL_LEGAL_NO_OBJECT_OVERLAP == 0x04
        && LOCAL_LEGAL_SHROUD_REVEALED == 0x10
        && LOCAL_LEGAL_IGNORE_STEALTHED == 0x40
        && LOCAL_LEGAL_FAIL_STEALTHED_WITHOUT_FEEDBACK == 0x80
        && TOTAL_FRAMES_TO_SELL_OBJECT_RESIDUAL == 90
        && (SELL_PERCENTAGE_RESIDUAL - 0.5).abs() < 1e-6
        && sell_refund_residual(2000) == 1000
        && sell_refund_residual(101) == 50
}

// ---------------------------------------------------------------------------
// 3. Prerequisite residual peels (ProductionPrerequisite)
// ---------------------------------------------------------------------------

/// C++ ProductionPrerequisite MAX_PREREQ residual.
pub const PREREQ_MAX_UNITS_RESIDUAL: usize = 32;
/// C++ UNIT_OR_WITH_PREV residual flag bit.
pub const PREREQ_UNIT_OR_WITH_PREV: u32 = 0x01;

/// ThingTemplate Prerequisites INI block residual field names.
pub const PREREQ_INI_FIELD_NAMES: &[&str] = &["Object", "Science"];

/// Sample residual prereq rows (retail INI Object / Science blocks).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrereqSampleRow {
    pub unit: &'static str,
    pub prereq_objects: &'static [&'static str],
    /// When true, objects after the first are OR-with-previous residual.
    pub or_chain: bool,
}

/// Retail prereq sample residual rows (faction tech tree anchors).
pub const PREREQ_SAMPLE_TABLE_RESIDUAL: &[PrereqSampleRow] = &[
    PrereqSampleRow {
        unit: "AmericaWarFactory",
        prereq_objects: &["AmericaSupplyCenter"],
        or_chain: false,
    },
    PrereqSampleRow {
        unit: "AmericaBarracks",
        prereq_objects: &["AmericaCommandCenter"],
        or_chain: false,
    },
    PrereqSampleRow {
        unit: "ChinaWarFactory",
        prereq_objects: &["ChinaSupplyCenter"],
        or_chain: false,
    },
    PrereqSampleRow {
        unit: "GLABarracks",
        prereq_objects: &["GLACommandCenter"],
        or_chain: false,
    },
    PrereqSampleRow {
        unit: "AmericaStrategyCenter",
        prereq_objects: &["AmericaWarFactory", "AmericaAirfield"],
        or_chain: true, // OR residual sample (either facility unlocks)
    },
];

/// Residual satisfaction for a simple unit-ownership map residual.
///
/// `owned` is a list of owned template names. `prereq_objects` with `or_chain`
/// treats the list as (A) or (A|B|C…) residual groups; without or_chain every
/// entry is required (AND). Science residual is always AND (caller passes
/// `sciences_ok`).
pub fn prereq_is_satisfied_residual(
    prereq_objects: &[&str],
    or_chain: bool,
    owned: &[&str],
    sciences_ok: bool,
) -> bool {
    if !sciences_ok {
        return false;
    }
    if prereq_objects.is_empty() {
        return true;
    }
    if or_chain {
        prereq_objects
            .iter()
            .any(|p| owned.iter().any(|o| *o == *p))
    } else {
        prereq_objects
            .iter()
            .all(|p| owned.iter().any(|o| *o == *p))
    }
}

/// Wave 99 honesty: prerequisite residual pack.
pub fn honesty_prerequisite_residual_pack_wave99() -> bool {
    PREREQ_MAX_UNITS_RESIDUAL == 32
        && PREREQ_UNIT_OR_WITH_PREV == 0x01
        && PREREQ_INI_FIELD_NAMES.len() == 2
        && residual_name_index(PREREQ_INI_FIELD_NAMES, "Object") == Some(0)
        && residual_name_index(PREREQ_INI_FIELD_NAMES, "Science") == Some(1)
        && PREREQ_SAMPLE_TABLE_RESIDUAL.len() == 5
        && PREREQ_SAMPLE_TABLE_RESIDUAL[0].unit == "AmericaWarFactory"
        && PREREQ_SAMPLE_TABLE_RESIDUAL[0].prereq_objects == &["AmericaSupplyCenter"]
        && !PREREQ_SAMPLE_TABLE_RESIDUAL[0].or_chain
        && PREREQ_SAMPLE_TABLE_RESIDUAL[4].or_chain
        && PREREQ_SAMPLE_TABLE_RESIDUAL[4].prereq_objects.len() == 2
        // AND residual: need CommandCenter for barracks
        && prereq_is_satisfied_residual(
            &["AmericaCommandCenter"],
            false,
            &["AmericaCommandCenter", "AmericaPowerPlant"],
            true,
        )
        && !prereq_is_satisfied_residual(
            &["AmericaCommandCenter"],
            false,
            &["AmericaPowerPlant"],
            true,
        )
        // OR residual: either WarFactory or Airfield
        && prereq_is_satisfied_residual(
            &["AmericaWarFactory", "AmericaAirfield"],
            true,
            &["AmericaAirfield"],
            true,
        )
        && prereq_is_satisfied_residual(
            &["AmericaWarFactory", "AmericaAirfield"],
            true,
            &["AmericaWarFactory"],
            true,
        )
        && !prereq_is_satisfied_residual(
            &["AmericaWarFactory", "AmericaAirfield"],
            true,
            &["AmericaBarracks"],
            true,
        )
        // Science residual gate
        && !prereq_is_satisfied_residual(&[], false, &[], false)
        && prereq_is_satisfied_residual(&[], false, &[], true)
}

// ---------------------------------------------------------------------------
// 4. CommandButton residual deepen (beyond Wave 80 superweapon labels)
// ---------------------------------------------------------------------------

/// C++ GUICommandType residual count (GUI_COMMAND_NUM_COMMANDS, no ALLOW_SURRENDER).
pub const GUI_COMMAND_NUM_COMMANDS_RESIDUAL: usize = 35;

/// Ordered C++ TheGuiCommandNames residual (ALLOW_SURRENDER off retail ZH).
pub const GUI_COMMAND_NAME_TABLE_RESIDUAL: &[&str] = &[
    "NONE",                                  // 0
    "DOZER_CONSTRUCT",                       // 1
    "DOZER_CONSTRUCT_CANCEL",                // 2
    "UNIT_BUILD",                            // 3
    "CANCEL_UNIT_BUILD",                     // 4
    "PLAYER_UPGRADE",                        // 5
    "OBJECT_UPGRADE",                        // 6
    "CANCEL_UPGRADE",                        // 7
    "ATTACK_MOVE",                           // 8
    "GUARD",                                 // 9
    "GUARD_WITHOUT_PURSUIT",                 // 10
    "GUARD_FLYING_UNITS_ONLY",               // 11
    "STOP",                                  // 12
    "WAYPOINTS",                             // 13
    "EXIT_CONTAINER",                        // 14
    "EVACUATE",                              // 15
    "EXECUTE_RAILED_TRANSPORT",              // 16
    "BEACON_DELETE",                         // 17
    "SET_RALLY_POINT",                       // 18
    "SELL",                                  // 19
    "FIRE_WEAPON",                           // 20
    "SPECIAL_POWER",                         // 21
    "PURCHASE_SCIENCE",                      // 22
    "HACK_INTERNET",                         // 23
    "TOGGLE_OVERCHARGE",                     // 24
    "COMBATDROP",                            // 25
    "SWITCH_WEAPON",                         // 26
    "HIJACK_VEHICLE",                        // 27
    "CONVERT_TO_CARBOMB",                    // 28
    "SABOTAGE_BUILDING",                     // 29
    "PLACE_BEACON",                          // 30
    "SPECIAL_POWER_FROM_SHORTCUT",           // 31
    "SPECIAL_POWER_CONSTRUCT",               // 32
    "SPECIAL_POWER_CONSTRUCT_FROM_SHORTCUT", // 33
    "SELECT_ALL_UNITS_OF_TYPE",              // 34
];

/// C++ CommandOption residual ordered bit-names (TheCommandOptionNames, bit index).
/// Bit 3 is "unused-reserved" when ALLOW_SURRENDER is off.
pub const COMMAND_OPTION_NAME_TABLE_RESIDUAL: &[&str] = &[
    "NEED_TARGET_ENEMY_OBJECT",     // bit 0  0x00000001
    "NEED_TARGET_NEUTRAL_OBJECT",   // bit 1  0x00000002
    "NEED_TARGET_ALLY_OBJECT",      // bit 2  0x00000004
    "unused-reserved",              // bit 3  0x00000008
    "ALLOW_SHRUBBERY_TARGET",       // bit 4  0x00000010
    "NEED_TARGET_POS",              // bit 5  0x00000020
    "NEED_UPGRADE",                 // bit 6  0x00000040
    "NEED_SPECIAL_POWER_SCIENCE",   // bit 7  0x00000080
    "OK_FOR_MULTI_SELECT",          // bit 8  0x00000100
    "CONTEXTMODE_COMMAND",          // bit 9  0x00000200
    "CHECK_LIKE",                   // bit 10 0x00000400
    "ALLOW_MINE_TARGET",            // bit 11 0x00000800
    "ATTACK_OBJECTS_POSITION",      // bit 12 0x00001000
    "OPTION_ONE",                   // bit 13 0x00002000
    "OPTION_TWO",                   // bit 14 0x00004000
    "OPTION_THREE",                 // bit 15 0x00008000
    "NOT_QUEUEABLE",                // bit 16 0x00010000
    "SINGLE_USE_COMMAND",           // bit 17 0x00020000
    "---DO-NOT-USE---",             // bit 18 0x00040000 COMMAND_FIRED_BY_SCRIPT
    "SCRIPT_ONLY",                  // bit 19 0x00080000
    "IGNORES_UNDERPOWERED",         // bit 20 0x00100000
    "USES_MINE_CLEARING_WEAPONSET", // bit 21 0x00200000
    "CAN_USE_WAYPOINTS",            // bit 22 0x00400000
    "MUST_BE_STOPPED",              // bit 23 0x00800000
];

pub const COMMAND_OPTION_NEED_TARGET_ENEMY_OBJECT: u32 = 0x0000_0001;
pub const COMMAND_OPTION_NEED_TARGET_POS: u32 = 0x0000_0020;
pub const COMMAND_OPTION_NEED_UPGRADE: u32 = 0x0000_0040;
pub const COMMAND_OPTION_NEED_SPECIAL_POWER_SCIENCE: u32 = 0x0000_0080;
pub const COMMAND_OPTION_OK_FOR_MULTI_SELECT: u32 = 0x0000_0100;
pub const COMMAND_OPTION_CONTEXTMODE_COMMAND: u32 = 0x0000_0200;
pub const COMMAND_OPTION_NOT_QUEUEABLE: u32 = 0x0001_0000;
pub const COMMAND_OPTION_SINGLE_USE_COMMAND: u32 = 0x0002_0000;
pub const COMMAND_OPTION_SCRIPT_ONLY: u32 = 0x0008_0000;
pub const COMMAND_OPTION_IGNORES_UNDERPOWERED: u32 = 0x0010_0000;
pub const COMMAND_OPTION_MUST_BE_STOPPED: u32 = 0x0080_0000;

/// C++ COMMAND_OPTION_NEED_TARGET composite residual.
pub const COMMAND_OPTION_NEED_TARGET_MASK: u32 = 0x0000_0001 // enemy
    | 0x0000_0002 // neutral
    | 0x0000_0004 // ally
    | 0x0000_0020 // pos
    | 0x0000_0200; // contextmode
/// C++ COMMAND_OPTION_NEED_OBJECT_TARGET composite residual.
pub const COMMAND_OPTION_NEED_OBJECT_TARGET_MASK: u32 = 0x0000_0001 | 0x0000_0002 | 0x0000_0004;

/// C++ CommandButtonMappedBorderType residual count.
pub const COMMAND_BUTTON_BORDER_COUNT: usize = 5;
/// Ordered border residual names.
pub const COMMAND_BUTTON_BORDER_NAME_TABLE_RESIDUAL: &[&str] =
    &["NONE", "BUILD", "UPGRADE", "ACTION", "SYSTEM"];

pub const COMMAND_BUTTON_BORDER_NONE: u32 = 0;
pub const COMMAND_BUTTON_BORDER_BUILD: u32 = 1;
pub const COMMAND_BUTTON_BORDER_UPGRADE: u32 = 2;
pub const COMMAND_BUTTON_BORDER_ACTION: u32 = 3;
pub const COMMAND_BUTTON_BORDER_SYSTEM: u32 = 4;

/// CommandButton INI field residual names (ControlBar.cpp parse table subset).
pub const COMMAND_BUTTON_INI_FIELD_NAMES: &[&str] = &[
    "Command",
    "Options",
    "TextLabel",
    "ButtonImage",
    "CursorName",
    "InvalidCursorName",
    "RadiusCursorType",
    "Border",
    "DescriptionLabel",
    "PurchasedLabel",
    "ConflictingLabel",
    "MaxShotsToFire",
    "Science",
    "WeaponSlot",
    "UnitSpecificSound",
];

/// Whether options residual requires a target (object/pos/context).
#[inline]
pub fn command_option_needs_target_residual(options: u32) -> bool {
    (options & COMMAND_OPTION_NEED_TARGET_MASK) != 0
}

/// Wave 99 honesty: command button residual deepen pack.
pub fn honesty_command_button_residual_deepen_pack_wave99() -> bool {
    GUI_COMMAND_NUM_COMMANDS_RESIDUAL == 35
        && GUI_COMMAND_NAME_TABLE_RESIDUAL.len() == 35
        && residual_name_index(GUI_COMMAND_NAME_TABLE_RESIDUAL, "NONE") == Some(0)
        && residual_name_index(GUI_COMMAND_NAME_TABLE_RESIDUAL, "DOZER_CONSTRUCT") == Some(1)
        && residual_name_index(GUI_COMMAND_NAME_TABLE_RESIDUAL, "UNIT_BUILD") == Some(3)
        && residual_name_index(GUI_COMMAND_NAME_TABLE_RESIDUAL, "SPECIAL_POWER") == Some(21)
        && residual_name_index(GUI_COMMAND_NAME_TABLE_RESIDUAL, "PURCHASE_SCIENCE") == Some(22)
        && residual_name_index(
            GUI_COMMAND_NAME_TABLE_RESIDUAL,
            "SPECIAL_POWER_FROM_SHORTCUT",
        ) == Some(31)
        && residual_name_index(GUI_COMMAND_NAME_TABLE_RESIDUAL, "SELECT_ALL_UNITS_OF_TYPE")
            == Some(34)
        && COMMAND_OPTION_NAME_TABLE_RESIDUAL.len() == 24
        && residual_name_index(
            COMMAND_OPTION_NAME_TABLE_RESIDUAL,
            "NEED_TARGET_ENEMY_OBJECT",
        ) == Some(0)
        && residual_name_index(COMMAND_OPTION_NAME_TABLE_RESIDUAL, "NEED_TARGET_POS") == Some(5)
        && residual_name_index(COMMAND_OPTION_NAME_TABLE_RESIDUAL, "NOT_QUEUEABLE") == Some(16)
        && residual_name_index(COMMAND_OPTION_NAME_TABLE_RESIDUAL, "MUST_BE_STOPPED") == Some(23)
        && COMMAND_OPTION_NEED_TARGET_ENEMY_OBJECT == 0x1
        && COMMAND_OPTION_NEED_TARGET_POS == 0x20
        && COMMAND_OPTION_NOT_QUEUEABLE == 0x1_0000
        && COMMAND_OPTION_MUST_BE_STOPPED == 0x80_0000
        && COMMAND_OPTION_NEED_TARGET_MASK == 0x227
        && COMMAND_OPTION_NEED_OBJECT_TARGET_MASK == 0x7
        && command_option_needs_target_residual(COMMAND_OPTION_NEED_TARGET_POS)
        && command_option_needs_target_residual(COMMAND_OPTION_NEED_TARGET_ENEMY_OBJECT)
        && !command_option_needs_target_residual(COMMAND_OPTION_NEED_UPGRADE)
        && COMMAND_BUTTON_BORDER_COUNT == 5
        && COMMAND_BUTTON_BORDER_NAME_TABLE_RESIDUAL.len() == 5
        && residual_name_index(COMMAND_BUTTON_BORDER_NAME_TABLE_RESIDUAL, "BUILD") == Some(1)
        && residual_name_index(COMMAND_BUTTON_BORDER_NAME_TABLE_RESIDUAL, "SYSTEM") == Some(4)
        && COMMAND_BUTTON_INI_FIELD_NAMES.len() == 15
        && residual_name_index(COMMAND_BUTTON_INI_FIELD_NAMES, "Command") == Some(0)
        && residual_name_index(COMMAND_BUTTON_INI_FIELD_NAMES, "RadiusCursorType") == Some(6)
}

// ---------------------------------------------------------------------------
// 5. ControlBar residual deepen (beyond Wave 76 window count / fonts)
// ---------------------------------------------------------------------------

/// C++ MAX_COMMANDS_PER_SET residual (UI shows 14; internal 18 for script-only).
pub const MAX_COMMANDS_PER_SET_RESIDUAL: usize = 18;
/// User-visible command button count residual (ButtonCommand01..14).
pub const CONTROL_BAR_VISIBLE_COMMAND_BUTTONS: usize = 14;
/// C++ MAX_BUILD_QUEUE_BUTTONS residual.
pub const MAX_BUILD_QUEUE_BUTTONS_RESIDUAL: usize = 9;
/// C++ MAX_STRUCTURE_INVENTORY_BUTTONS residual.
pub const MAX_STRUCTURE_INVENTORY_BUTTONS_RESIDUAL: usize = 10;
/// C++ MAX_SPECIAL_POWER_SHORTCUTS residual.
pub const MAX_SPECIAL_POWER_SHORTCUTS_RESIDUAL: usize = 11;
/// C++ MAX_RIGHT_HUD_UPGRADE_CAMEOS residual.
pub const MAX_RIGHT_HUD_UPGRADE_CAMEOS_RESIDUAL: usize = 5;
/// C++ MAX_PURCHASE_SCIENCE_RANK_1 residual.
pub const MAX_PURCHASE_SCIENCE_RANK_1_RESIDUAL: usize = 4;
/// C++ MAX_PURCHASE_SCIENCE_RANK_3 residual.
pub const MAX_PURCHASE_SCIENCE_RANK_3_RESIDUAL: usize = 15;
/// C++ MAX_PURCHASE_SCIENCE_RANK_8 residual.
pub const MAX_PURCHASE_SCIENCE_RANK_8_RESIDUAL: usize = 4;

/// C++ ControlBarContext residual count (NUM_CB_CONTEXTS).
pub const NUM_CB_CONTEXTS_RESIDUAL: usize = 9;

/// Ordered ControlBarContext residual names.
pub const CONTROL_BAR_CONTEXT_NAME_TABLE_RESIDUAL: &[&str] = &[
    "CB_CONTEXT_NONE",                // 0
    "CB_CONTEXT_COMMAND",             // 1
    "CB_CONTEXT_STRUCTURE_INVENTORY", // 2
    "CB_CONTEXT_BEACON",              // 3
    "CB_CONTEXT_UNDER_CONSTRUCTION",  // 4
    "CB_CONTEXT_MULTI_SELECT",        // 5
    "CB_CONTEXT_OBSERVER_INFO",       // 6
    "CB_CONTEXT_OBSERVER_LIST",       // 7
    "CB_CONTEXT_OCL_TIMER",           // 8
];

/// Visible ButtonCommand residual names 01..14.
pub const CONTROL_BAR_BUTTON_COMMAND_NAMES: &[&str] = &[
    "ButtonCommand01",
    "ButtonCommand02",
    "ButtonCommand03",
    "ButtonCommand04",
    "ButtonCommand05",
    "ButtonCommand06",
    "ButtonCommand07",
    "ButtonCommand08",
    "ButtonCommand09",
    "ButtonCommand10",
    "ButtonCommand11",
    "ButtonCommand12",
    "ButtonCommand13",
    "ButtonCommand14",
];

/// Production queue button residual names (Queue01..09) residual.
pub const CONTROL_BAR_QUEUE_BUTTON_NAMES: &[&str] = &[
    "ButtonQueue01",
    "ButtonQueue02",
    "ButtonQueue03",
    "ButtonQueue04",
    "ButtonQueue05",
    "ButtonQueue06",
    "ButtonQueue07",
    "ButtonQueue08",
    "ButtonQueue09",
];

/// Wave 76 window-count cross-link residual.
pub const CONTROL_BAR_RETAIL_WINDOW_COUNT_CROSSLINK: usize = 98;

/// Wave 99 honesty: control bar residual deepen pack.
pub fn honesty_control_bar_residual_deepen_pack_wave99() -> bool {
    MAX_COMMANDS_PER_SET_RESIDUAL == 18
        && CONTROL_BAR_VISIBLE_COMMAND_BUTTONS == 14
        && MAX_BUILD_QUEUE_BUTTONS_RESIDUAL == 9
        && MAX_STRUCTURE_INVENTORY_BUTTONS_RESIDUAL == 10
        && MAX_SPECIAL_POWER_SHORTCUTS_RESIDUAL == 11
        && MAX_RIGHT_HUD_UPGRADE_CAMEOS_RESIDUAL == 5
        && MAX_PURCHASE_SCIENCE_RANK_1_RESIDUAL == 4
        && MAX_PURCHASE_SCIENCE_RANK_3_RESIDUAL == 15
        && MAX_PURCHASE_SCIENCE_RANK_8_RESIDUAL == 4
        && NUM_CB_CONTEXTS_RESIDUAL == 9
        && CONTROL_BAR_CONTEXT_NAME_TABLE_RESIDUAL.len() == 9
        && residual_name_index(CONTROL_BAR_CONTEXT_NAME_TABLE_RESIDUAL, "CB_CONTEXT_NONE")
            == Some(0)
        && residual_name_index(CONTROL_BAR_CONTEXT_NAME_TABLE_RESIDUAL, "CB_CONTEXT_COMMAND")
            == Some(1)
        && residual_name_index(
            CONTROL_BAR_CONTEXT_NAME_TABLE_RESIDUAL,
            "CB_CONTEXT_UNDER_CONSTRUCTION",
        ) == Some(4)
        && residual_name_index(
            CONTROL_BAR_CONTEXT_NAME_TABLE_RESIDUAL,
            "CB_CONTEXT_MULTI_SELECT",
        ) == Some(5)
        && residual_name_index(
            CONTROL_BAR_CONTEXT_NAME_TABLE_RESIDUAL,
            "CB_CONTEXT_OCL_TIMER",
        ) == Some(8)
        && CONTROL_BAR_BUTTON_COMMAND_NAMES.len() == 14
        && CONTROL_BAR_BUTTON_COMMAND_NAMES[0] == "ButtonCommand01"
        && CONTROL_BAR_BUTTON_COMMAND_NAMES[13] == "ButtonCommand14"
        && CONTROL_BAR_QUEUE_BUTTON_NAMES.len() == 9
        && CONTROL_BAR_QUEUE_BUTTON_NAMES[0] == "ButtonQueue01"
        && CONTROL_BAR_QUEUE_BUTTON_NAMES[8] == "ButtonQueue09"
        && CONTROL_BAR_RETAIL_WINDOW_COUNT_CROSSLINK == 98
        // Visible buttons ≤ internal max; build queue matches MaxQueueEntries residual.
        && CONTROL_BAR_VISIBLE_COMMAND_BUTTONS < MAX_COMMANDS_PER_SET_RESIDUAL
        && MAX_BUILD_QUEUE_BUTTONS_RESIDUAL == PRODUCTION_MAX_QUEUE_ENTRIES_DEEPEN
}

// ---------------------------------------------------------------------------
// Combined Wave 99 residual pack
// ---------------------------------------------------------------------------

/// Combined Wave 99 residual honesty pack.
pub fn honesty_production_buildable_command_residual_pack_wave99() -> bool {
    honesty_production_residual_deepen_pack_wave99()
        && honesty_buildable_residual_pack_wave99()
        && honesty_prerequisite_residual_pack_wave99()
        && honesty_command_button_residual_deepen_pack_wave99()
        && honesty_control_bar_residual_deepen_pack_wave99()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_residual_deepen_wave99_honesty() {
        assert!(honesty_production_residual_deepen_pack_wave99());
    }

    #[test]
    fn buildable_residual_wave99_honesty() {
        assert!(honesty_buildable_residual_pack_wave99());
    }

    #[test]
    fn prerequisite_residual_wave99_honesty() {
        assert!(honesty_prerequisite_residual_pack_wave99());
    }

    #[test]
    fn command_button_residual_deepen_wave99_honesty() {
        assert!(honesty_command_button_residual_deepen_pack_wave99());
    }

    #[test]
    fn control_bar_residual_deepen_wave99_honesty() {
        assert!(honesty_control_bar_residual_deepen_pack_wave99());
    }

    #[test]
    fn production_buildable_command_residual_pack_wave99_honesty() {
        assert!(honesty_production_buildable_command_residual_pack_wave99());
    }
}
