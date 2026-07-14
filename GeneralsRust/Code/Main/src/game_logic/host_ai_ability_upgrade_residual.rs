//! Wave 94 residual peels: AI state tables / special ability deepen /
//! upgrade full name table / CommandSet superweapon building residual.
//!
//! Host-testable residual for fuller ZH command/AI/ability/upgrade parity
//! without full exclusive module graph or ControlBar GPU cameos.
//!
//! Sources (retail ZH INI + C++):
//! - `AIStateMachine.h` `AIStateType` / `NUM_AI_STATES`
//! - `SpecialPower.ini` `SpecialAbility*` templates (Enum + ReloadTime)
//! - Object INI `SpecialAbilityUpdate` StartAbilityRange / Unpack / Pack / Prep
//! - `Upgrade.ini` full internal-name residual table
//! - `CommandSet.ini` superweapon building + command center slots
//! - Host `AIState` ↔ C++ `AI_*` residual bridge
//!
//! Fail-closed:
//! - Not full AIStateMachine exclusive state enter/exit / path residual
//! - Not full SpecialAbilityUpdate flee-after / MaxSpecialObjects matrix
//! - Not full UpgradeCenter NameKey purchase / multipleyer upgrade replication
//! - Not full ControlBar CommandSet slot UI matrix / WND cameo residual
//! - Shell `playable_claim` stays false; network deferred

use crate::game_logic::object::AIState;

// ---------------------------------------------------------------------------
// 1. AI state residual tables (AIStateMachine.h AIStateType)
// ---------------------------------------------------------------------------

/// C++ `NUM_AI_STATES` residual (count of AIStateType discriminants before sentinel).
pub const NUM_AI_STATES_RESIDUAL: usize = 44;

/// Ordered C++ `AIStateType` residual names (indices 0..43).
///
/// Source: `GeneralsMD/Code/GameEngine/Include/GameLogic/AIStateMachine.h`.
pub const AI_STATE_TYPE_NAME_TABLE: &[&str] = &[
    "AI_IDLE",                                      // 0
    "AI_MOVE_TO",                                   // 1
    "AI_FOLLOW_WAYPOINT_PATH_AS_TEAM",              // 2
    "AI_FOLLOW_WAYPOINT_PATH_AS_INDIVIDUALS",       // 3
    "AI_FOLLOW_WAYPOINT_PATH_AS_TEAM_EXACT",        // 4
    "AI_FOLLOW_WAYPOINT_PATH_AS_INDIVIDUALS_EXACT", // 5
    "AI_FOLLOW_PATH",                               // 6
    "AI_FOLLOW_EXITPRODUCTION_PATH",                // 7
    "AI_WAIT",                                      // 8
    "AI_ATTACK_POSITION",                           // 9
    "AI_ATTACK_OBJECT",                             // 10
    "AI_FORCE_ATTACK_OBJECT",                       // 11
    "AI_ATTACK_AND_FOLLOW_OBJECT",                  // 12
    "AI_DEAD",                                      // 13
    "AI_DOCK",                                      // 14
    "AI_ENTER",                                     // 15
    "AI_GUARD",                                     // 16
    "AI_HUNT",                                      // 17
    "AI_WANDER",                                    // 18
    "AI_PANIC",                                     // 19
    "AI_ATTACK_SQUAD",                              // 20
    "AI_GUARD_TUNNEL_NETWORK",                      // 21
    "AI_GET_REPAIRED",                              // 22
    "AI_MOVE_OUT_OF_THE_WAY",                       // 23
    "AI_MOVE_AND_TIGHTEN",                          // 24
    "AI_MOVE_AND_EVACUATE",                         // 25
    "AI_MOVE_AND_EVACUATE_AND_EXIT",                // 26
    "AI_MOVE_AND_DELETE",                           // 27
    "AI_ATTACK_AREA",                               // 28
    "AI_HACK_INTERNET",                             // 29
    "AI_ATTACK_MOVE_TO",                            // 30
    "AI_ATTACKFOLLOW_WAYPOINT_PATH_AS_INDIVIDUALS", // 31
    "AI_ATTACKFOLLOW_WAYPOINT_PATH_AS_TEAM",        // 32
    "AI_FACE_OBJECT",                               // 33
    "AI_FACE_POSITION",                             // 34
    "AI_RAPPEL_INTO",                               // 35
    "AI_COMBATDROP",                                // 36
    "AI_EXIT",                                      // 37
    "AI_PICK_UP_CRATE",                             // 38
    "AI_MOVE_AWAY_FROM_REPULSORS",                  // 39
    "AI_WANDER_IN_PLACE",                           // 40
    "AI_BUSY",                                      // 41
    "AI_EXIT_INSTANTLY",                            // 42
    "AI_GUARD_RETALIATE",                           // 43
];

/// Lookup C++ AI state residual name index.
pub fn ai_state_type_name_index(name: &str) -> Option<usize> {
    AI_STATE_TYPE_NAME_TABLE.iter().position(|&n| n == name)
}

/// Map host simplified `AIState` to primary C++ `AI_*` residual name.
///
/// Host AIState is a reduced command residual; several host states collapse
/// multiple C++ states (e.g. GuardingArea/GuardingObject → AI_GUARD).
pub fn host_ai_state_cpp_enum_name(state: &AIState) -> &'static str {
    match state {
        AIState::Idle => "AI_IDLE",
        AIState::Moving => "AI_MOVE_TO",
        AIState::Attacking => "AI_ATTACK_OBJECT",
        AIState::AttackMoving => "AI_ATTACK_MOVE_TO",
        AIState::AttackingGround => "AI_ATTACK_POSITION",
        AIState::Gathering => "AI_DOCK",
        AIState::ReturningResources => "AI_DOCK",
        AIState::Constructing => "AI_BUSY",
        AIState::Repairing => "AI_BUSY",
        AIState::GuardingArea | AIState::GuardingObject => "AI_GUARD",
        AIState::Patrolling => "AI_HUNT",
        AIState::Docked | AIState::Docking => "AI_DOCK",
        AIState::Garrisoned => "AI_ENTER",
        AIState::SpecialAbility => "AI_BUSY",
        AIState::SeekingRepair => "AI_GET_REPAIRED",
        AIState::SeekingHealing => "AI_GET_REPAIRED",
        AIState::Entering => "AI_ENTER",
        AIState::Capturing => "AI_BUSY",
    }
}

/// Wave 94 honesty: C++ AIStateType residual name/ordinal table.
///
/// Fail-closed: not full AIStateMachine exclusive enter/exit residual.
pub fn honesty_ai_state_residual_table_wave94() -> bool {
    if AI_STATE_TYPE_NAME_TABLE.len() != NUM_AI_STATES_RESIDUAL {
        return false;
    }
    // Unique + anchors.
    let mut seen = std::collections::HashSet::new();
    for n in AI_STATE_TYPE_NAME_TABLE {
        if !seen.insert(*n) || !n.starts_with("AI_") {
            return false;
        }
    }
    AI_STATE_TYPE_NAME_TABLE[0] == "AI_IDLE"
        && AI_STATE_TYPE_NAME_TABLE[1] == "AI_MOVE_TO"
        && AI_STATE_TYPE_NAME_TABLE[10] == "AI_ATTACK_OBJECT"
        && AI_STATE_TYPE_NAME_TABLE[14] == "AI_DOCK"
        && AI_STATE_TYPE_NAME_TABLE[15] == "AI_ENTER"
        && AI_STATE_TYPE_NAME_TABLE[16] == "AI_GUARD"
        && AI_STATE_TYPE_NAME_TABLE[22] == "AI_GET_REPAIRED"
        && AI_STATE_TYPE_NAME_TABLE[29] == "AI_HACK_INTERNET"
        && AI_STATE_TYPE_NAME_TABLE[30] == "AI_ATTACK_MOVE_TO"
        && AI_STATE_TYPE_NAME_TABLE[43] == "AI_GUARD_RETALIATE"
        && ai_state_type_name_index("AI_IDLE") == Some(0)
        && ai_state_type_name_index("AI_GUARD_RETALIATE") == Some(43)
        && ai_state_type_name_index("AI_DOES_NOT_EXIST").is_none()
        // Host bridge residual.
        && host_ai_state_cpp_enum_name(&AIState::Idle) == "AI_IDLE"
        && host_ai_state_cpp_enum_name(&AIState::Moving) == "AI_MOVE_TO"
        && host_ai_state_cpp_enum_name(&AIState::Attacking) == "AI_ATTACK_OBJECT"
        && host_ai_state_cpp_enum_name(&AIState::AttackMoving) == "AI_ATTACK_MOVE_TO"
        && host_ai_state_cpp_enum_name(&AIState::AttackingGround) == "AI_ATTACK_POSITION"
        && host_ai_state_cpp_enum_name(&AIState::Entering) == "AI_ENTER"
        && host_ai_state_cpp_enum_name(&AIState::GuardingArea) == "AI_GUARD"
        && host_ai_state_cpp_enum_name(&AIState::SeekingRepair) == "AI_GET_REPAIRED"
        && host_ai_state_cpp_enum_name(&AIState::SpecialAbility) == "AI_BUSY"
        && host_ai_state_cpp_enum_name(&AIState::Capturing) == "AI_BUSY"
        // Bridge names exist in the C++ residual table.
        && [
            AIState::Idle,
            AIState::Moving,
            AIState::Attacking,
            AIState::AttackMoving,
            AIState::AttackingGround,
            AIState::Gathering,
            AIState::Entering,
            AIState::GuardingObject,
            AIState::SeekingRepair,
            AIState::SpecialAbility,
            AIState::Capturing,
            AIState::Patrolling,
            AIState::Constructing,
        ]
        .iter()
        .all(|s| ai_state_type_name_index(host_ai_state_cpp_enum_name(s)).is_some())
}

// ---------------------------------------------------------------------------
// 2. Special ability residual deepen (SpecialPower.ini + SpecialAbilityUpdate)
// ---------------------------------------------------------------------------

/// Retail SpecialAbility* template residual count (core + general variants).
pub const SPECIAL_ABILITY_RESIDUAL_NAME_COUNT: usize = 27;

/// SpecialAbility residual rows: (template name, C++ Enum, ReloadTime ms).
///
/// Core 20 from SpecialPower.ini plus 7 general variants (Demo/Nuke/Lazr).
/// ReloadTime is 0 when omitted (CashBounty residual has no ReloadTime field).
pub const SPECIAL_ABILITY_RESIDUAL_TABLE: &[(&str, &str, u32)] = &[
    (
        "SpecialAbilityMissileDefenderLaserGuidedMissiles",
        "SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES",
        0,
    ),
    (
        "SpecialAbilityTankHunterTNTAttack",
        "SPECIAL_TANKHUNTER_TNT_ATTACK",
        7_500,
    ),
    ("SpecialAbilityBoobyTrap", "SPECIAL_BOOBY_TRAP", 7_500),
    (
        "SpecialAbilityColonelBurtonRemoteCharges",
        "SPECIAL_REMOTE_CHARGES",
        0,
    ),
    (
        "SpecialAbilityColonelBurtonTimedCharges",
        "SPECIAL_TIMED_CHARGES",
        0,
    ),
    (
        "SpecialAbilityHackerDisableBuilding",
        "SPECIAL_HACKER_DISABLE_BUILDING",
        500,
    ),
    (
        "SpecialAbilityMicrowaveDisableBuilding",
        "SPECIAL_HACKER_DISABLE_BUILDING",
        4_000,
    ),
    (
        "SpecialAbilityBlackLotusCaptureBuilding",
        "SPECIAL_BLACKLOTUS_CAPTURE_BUILDING",
        0,
    ),
    (
        "SpecialAbilityRangerCaptureBuilding",
        "SPECIAL_INFANTRY_CAPTURE_BUILDING",
        15_000,
    ),
    (
        "SpecialAbilityRedGuardCaptureBuilding",
        "SPECIAL_INFANTRY_CAPTURE_BUILDING",
        15_000,
    ),
    (
        "SpecialAbilityRebelCaptureBuilding",
        "SPECIAL_INFANTRY_CAPTURE_BUILDING",
        15_000,
    ),
    (
        "SpecialAbilityBlackLotusDisableVehicleHack",
        "SPECIAL_BLACKLOTUS_DISABLE_VEHICLE_HACK",
        0,
    ),
    (
        "SpecialAbilityBlackLotusStealCashHack",
        "SPECIAL_BLACKLOTUS_STEAL_CASH_HACK",
        2_000,
    ),
    (
        "SpecialAbilityDisguiseAsVehicle",
        "SPECIAL_DISGUISE_AS_VEHICLE",
        0,
    ),
    ("SpecialAbilityCashBounty1", "SPECIAL_CASH_BOUNTY", 0),
    ("SpecialAbilityCashBounty2", "SPECIAL_CASH_BOUNTY", 0),
    ("SpecialAbilityCashBounty3", "SPECIAL_CASH_BOUNTY", 0),
    (
        "SpecialAbilityChangeBattlePlans",
        "SPECIAL_CHANGE_BATTLE_PLANS",
        0,
    ),
    (
        "SpecialAbilityAmbulanceCleanupArea",
        "SPECIAL_CLEANUP_AREA",
        0,
    ),
    (
        "SpecialAbilityHelixNapalmBomb",
        "SPECIAL_HELIX_NAPALM_BOMB",
        10_000,
    ),
    // General variants residual
    (
        "Demo_SpecialAbilityDemoRebelTimedCharges",
        "SPECIAL_TIMED_CHARGES",
        30_000,
    ),
    (
        "Demo_SpecialAbilityKellRemoteCharges",
        "SPECIAL_REMOTE_CHARGES",
        0,
    ),
    (
        "Demo_SpecialAbilityDemoKellTimedCharges",
        "SPECIAL_TIMED_CHARGES",
        0,
    ),
    (
        "Demo_SpecialAbilityDemoKellStickyCharges",
        "SPECIAL_TIMED_CHARGES",
        0,
    ),
    (
        "Demo_SpecialAbilityBattleBusDemoTrapRollout",
        "SPECIAL_TIMED_CHARGES",
        7_500,
    ),
    (
        "Nuke_SpecialAbilityHelixNukeBomb",
        "SPECIAL_HELIX_NAPALM_BOMB",
        10_000,
    ),
    (
        "Lazr_SpecialAbilityLaserGuidedHowitzer",
        "SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES",
        0,
    ),
];

/// SpecialAbilityUpdate residual row for host deepen honesty.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpecialAbilityUpdateResidual {
    pub special_power: &'static str,
    pub start_ability_range: f32,
    pub unpack_ms: u32,
    pub pack_ms: u32,
    pub preparation_ms: u32,
    pub max_special_objects: u32,
}

/// Sample SpecialAbilityUpdate residual packs (Object INI).
///
/// Units: Black Lotus capture / disable / steal, Tank Hunter TNT,
/// Missile Defender laser, Ranger capture, Hacker disable, Helix napalm.
pub const SPECIAL_ABILITY_UPDATE_RESIDUAL_SAMPLES: &[SpecialAbilityUpdateResidual] = &[
    // ChinaInfantryBlackLotus capture residual (ChinaInfantry.ini).
    SpecialAbilityUpdateResidual {
        special_power: "SpecialAbilityBlackLotusCaptureBuilding",
        start_ability_range: 150.0,
        unpack_ms: 6_730,
        pack_ms: 2_800,
        preparation_ms: 6_000,
        max_special_objects: 0,
    },
    // Black Lotus disable vehicle residual.
    SpecialAbilityUpdateResidual {
        special_power: "SpecialAbilityBlackLotusDisableVehicleHack",
        start_ability_range: 150.0,
        unpack_ms: 2_000,
        pack_ms: 1_000,
        preparation_ms: 2_000,
        max_special_objects: 0,
    },
    // Black Lotus steal cash residual.
    SpecialAbilityUpdateResidual {
        special_power: "SpecialAbilityBlackLotusStealCashHack",
        start_ability_range: 150.0,
        unpack_ms: 6_730,
        pack_ms: 5_800,
        preparation_ms: 6_000,
        max_special_objects: 0,
    },
    // Tank Hunter TNT residual (ChinaInfantry.ini).
    SpecialAbilityUpdateResidual {
        special_power: "SpecialAbilityTankHunterTNTAttack",
        start_ability_range: 5.0,
        unpack_ms: 0,
        pack_ms: 0,
        preparation_ms: 0,
        max_special_objects: 8,
    },
    // Missile Defender laser residual (AmericaInfantry.ini).
    SpecialAbilityUpdateResidual {
        special_power: "SpecialAbilityMissileDefenderLaserGuidedMissiles",
        start_ability_range: 200.0,
        unpack_ms: 0,
        pack_ms: 0,
        preparation_ms: 1_000,
        max_special_objects: 0,
    },
    // Ranger capture residual (AmericaInfantry.ini).
    SpecialAbilityUpdateResidual {
        special_power: "SpecialAbilityRangerCaptureBuilding",
        start_ability_range: 5.0,
        unpack_ms: 3_000,
        pack_ms: 2_000,
        preparation_ms: 20_000,
        max_special_objects: 0,
    },
    // Hacker disable building residual (ChinaInfantry.ini).
    SpecialAbilityUpdateResidual {
        special_power: "SpecialAbilityHackerDisableBuilding",
        start_ability_range: 150.0,
        unpack_ms: 7_300,
        pack_ms: 5_133,
        preparation_ms: 3_000,
        max_special_objects: 0,
    },
    // Helix napalm residual (SpecialPower ReloadTime drive; update range 0 host residual).
    SpecialAbilityUpdateResidual {
        special_power: "SpecialAbilityHelixNapalmBomb",
        start_ability_range: 0.0,
        unpack_ms: 0,
        pack_ms: 0,
        preparation_ms: 0,
        max_special_objects: 0,
    },
];

/// Whether a special ability template name is in the residual table.
pub fn special_ability_residual_name_known(name: &str) -> bool {
    SPECIAL_ABILITY_RESIDUAL_TABLE
        .iter()
        .any(|(n, _, _)| *n == name)
}

/// ReloadTime residual (ms) for a special ability template, if known.
pub fn special_ability_reload_ms(name: &str) -> Option<u32> {
    SPECIAL_ABILITY_RESIDUAL_TABLE
        .iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, _, ms)| *ms)
}

/// C++ Enum residual for a special ability template.
pub fn special_ability_cpp_enum(name: &str) -> Option<&'static str> {
    SPECIAL_ABILITY_RESIDUAL_TABLE
        .iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, e, _)| *e)
}

/// Convert SpecialAbility ReloadTime ms → logic frames at 30 FPS residual.
pub fn special_ability_reload_frames(ms: u32) -> u32 {
    // C++ ConvertDurationFromMsecsToFrames residual (ms * 0.03 rounded).
    ((ms as f32) * 0.03).round() as u32
}

/// Wave 94 honesty: SpecialAbility residual deepen pack.
///
/// Fail-closed: not full SpecialAbilityUpdate exclusive flee / attach residual.
pub fn honesty_special_ability_residual_deepen_wave94() -> bool {
    if SPECIAL_ABILITY_RESIDUAL_TABLE.len() != SPECIAL_ABILITY_RESIDUAL_NAME_COUNT {
        return false;
    }
    // Unique template names.
    let mut seen = std::collections::HashSet::new();
    for (n, e, _) in SPECIAL_ABILITY_RESIDUAL_TABLE {
        if !seen.insert(*n) {
            return false;
        }
        if !n.contains("SpecialAbility") || !e.starts_with("SPECIAL_") {
            return false;
        }
    }
    // Reload anchors.
    special_ability_reload_ms("SpecialAbilityTankHunterTNTAttack") == Some(7_500)
        && special_ability_reload_ms("SpecialAbilityBoobyTrap") == Some(7_500)
        && special_ability_reload_ms("SpecialAbilityRangerCaptureBuilding") == Some(15_000)
        && special_ability_reload_ms("SpecialAbilityRedGuardCaptureBuilding") == Some(15_000)
        && special_ability_reload_ms("SpecialAbilityRebelCaptureBuilding") == Some(15_000)
        && special_ability_reload_ms("SpecialAbilityHackerDisableBuilding") == Some(500)
        && special_ability_reload_ms("SpecialAbilityMicrowaveDisableBuilding") == Some(4_000)
        && special_ability_reload_ms("SpecialAbilityBlackLotusStealCashHack") == Some(2_000)
        && special_ability_reload_ms("SpecialAbilityHelixNapalmBomb") == Some(10_000)
        && special_ability_reload_ms("SpecialAbilityMissileDefenderLaserGuidedMissiles")
            == Some(0)
        && special_ability_reload_ms("SpecialAbilityCashBounty1") == Some(0)
        // Enum anchors.
        && special_ability_cpp_enum("SpecialAbilityRangerCaptureBuilding")
            == Some("SPECIAL_INFANTRY_CAPTURE_BUILDING")
        && special_ability_cpp_enum("SpecialAbilityBlackLotusCaptureBuilding")
            == Some("SPECIAL_BLACKLOTUS_CAPTURE_BUILDING")
        && special_ability_cpp_enum("SpecialAbilityTankHunterTNTAttack")
            == Some("SPECIAL_TANKHUNTER_TNT_ATTACK")
        && special_ability_cpp_enum("SpecialAbilityChangeBattlePlans")
            == Some("SPECIAL_CHANGE_BATTLE_PLANS")
        // Frame conversion residual (7500ms → 225 frames @ 30 FPS).
        && special_ability_reload_frames(7_500) == 225
        && special_ability_reload_frames(15_000) == 450
        && special_ability_reload_frames(500) == 15
        && special_ability_reload_frames(0) == 0
        // SpecialAbilityUpdate samples residual.
        && SPECIAL_ABILITY_UPDATE_RESIDUAL_SAMPLES.len() == 8
        && SPECIAL_ABILITY_UPDATE_RESIDUAL_SAMPLES[0].start_ability_range == 150.0
        && SPECIAL_ABILITY_UPDATE_RESIDUAL_SAMPLES[0].unpack_ms == 6_730
        && SPECIAL_ABILITY_UPDATE_RESIDUAL_SAMPLES[0].preparation_ms == 6_000
        && SPECIAL_ABILITY_UPDATE_RESIDUAL_SAMPLES[1].start_ability_range == 150.0
        && SPECIAL_ABILITY_UPDATE_RESIDUAL_SAMPLES[1].preparation_ms == 2_000
        && SPECIAL_ABILITY_UPDATE_RESIDUAL_SAMPLES[3].max_special_objects == 8
        && SPECIAL_ABILITY_UPDATE_RESIDUAL_SAMPLES[3].start_ability_range == 5.0
        && SPECIAL_ABILITY_UPDATE_RESIDUAL_SAMPLES[4].start_ability_range == 200.0
        && SPECIAL_ABILITY_UPDATE_RESIDUAL_SAMPLES[4].preparation_ms == 1_000
        && SPECIAL_ABILITY_UPDATE_RESIDUAL_SAMPLES[5].preparation_ms == 20_000
        && SPECIAL_ABILITY_UPDATE_RESIDUAL_SAMPLES[6].start_ability_range == 150.0
        && SPECIAL_ABILITY_UPDATE_RESIDUAL_SAMPLES[6].unpack_ms == 7_300
        && SPECIAL_ABILITY_UPDATE_RESIDUAL_SAMPLES[6].preparation_ms == 3_000
        && SPECIAL_ABILITY_UPDATE_RESIDUAL_SAMPLES
            .iter()
            .all(|s| special_ability_residual_name_known(s.special_power))
        // General variant anchors.
        && special_ability_residual_name_known("Demo_SpecialAbilityDemoRebelTimedCharges")
        && special_ability_residual_name_known("Nuke_SpecialAbilityHelixNukeBomb")
        && special_ability_residual_name_known("Lazr_SpecialAbilityLaserGuidedHowitzer")
}

// ---------------------------------------------------------------------------
// 3. Upgrade residual full name table (Upgrade.ini)
// ---------------------------------------------------------------------------

/// Retail Upgrade.ini unique residual name count (duplicate SupW PD Drone collapsed).
pub const UPGRADE_RESIDUAL_NAME_COUNT: usize = 81;

/// Complete retail Upgrade.ini internal-name residual table (first-declaration order).
///
/// Source: `Data/INI/Upgrade.ini`. Duplicate `SupW_Upgrade_AmericaPointDefenseDrone`
/// second declaration is collapsed to one residual entry.
pub const UPGRADE_RESIDUAL_NAME_TABLE: &[&str] = &[
    "Upgrade_Nationalism",
    "Upgrade_Fanaticism",
    "Upgrade_AmericaRadar",
    "Upgrade_AmericaAdvancedControlRods",
    "SupW_Upgrade_AmericaAdvancedControlRods",
    "Upgrade_AmericaSupplyLines",
    "Upgrade_AmericaRangerFlashBangGrenade",
    "Upgrade_AmericaTOWMissile",
    "Upgrade_ComancheRocketPods",
    "Upgrade_AmericaLaserMissiles",
    "Upgrade_AmericaCountermeasures",
    "Upgrade_AmericaBunkerBusters",
    "Upgrade_AmericaAdvancedTraining",
    "Upgrade_AmericaDroneArmor",
    "Upgrade_AmericaCompositeArmor",
    "Upgrade_InfantryCaptureBuilding",
    "Upgrade_AmericaMOAB",
    "Upgrade_AmericaChemicalSuits",
    "Upgrade_AmericaSentryDroneGun",
    "Upgrade_AmericaScoutDrone",
    "Upgrade_AmericaBattleDrone",
    "Upgrade_AmericaHellfireDrone",
    "Upgrade_ChinaMines",
    "Upgrade_ChinaEMPMines",
    "Upgrade_ChinaRadar",
    "Upgrade_ChinaBlackNapalm",
    "Upgrade_ChinaChainGuns",
    "Upgrade_ChinaAircraftArmor",
    "Upgrade_ChinaTacticalNukeMig",
    "Upgrade_ChinaSubliminalMessaging",
    "Upgrade_ChinaSatelliteHackOne",
    "Upgrade_ChinaSatelliteHackTwo",
    "Upgrade_GLARadar",
    "Upgrade_ChinaUraniumShells",
    "Upgrade_ChinaNeutronShells",
    "Upgrade_ChinaNuclearTanks",
    "Upgrade_GLAWorkerFakeCommandSet",
    "Upgrade_GLAWorkerRealCommandSet",
    "Upgrade_BecomeRealGLACommandCenter",
    "Upgrade_BecomeRealGLABarracks",
    "Upgrade_BecomeRealGLASupplyStash",
    "Upgrade_BecomeRealGLAArmsDealer",
    "Upgrade_BecomeRealGLABlackMarket",
    "Upgrade_CashBounty",
    "Upgrade_GLAWorkerShoes",
    "Upgrade_GLAFortifiedStructure",
    "Upgrade_GLAInfantryRebelBoobyTrapAttack",
    "Upgrade_GLAScorpionRocket",
    "Upgrade_GLARadarVanScan",
    "Upgrade_GLAAnthraxBeta",
    "Upgrade_GLAToxinShells",
    "Upgrade_GLACamouflage",
    "Upgrade_GLAAPRockets",
    "Upgrade_GLAJunkRepair",
    "Upgrade_GLAAPBullets",
    "Upgrade_GLABuggyAmmo",
    "Upgrade_GLACamoNetting",
    "Upgrade_GLABombTruckHighExplosiveBomb",
    "Upgrade_GLABombTruckBioBomb",
    "Upgrade_GLAArmTheMob",
    "Upgrade_ChinaOverlordGattlingCannon",
    "Upgrade_ChinaOverlordPropagandaTower",
    "Upgrade_ChinaOverlordBattleBunker",
    "Upgrade_CostReduction",
    "Upgrade_HelixNapalmBomb",
    "Upgrade_ChinaHelixGattlingCannon",
    "Upgrade_ChinaHelixPropagandaTower",
    "Upgrade_ChinaHelixBattleBunker",
    "Upgrade_Infa_ChinaHelixBattleBunker",
    "Chem_Upgrade_GLAAnthraxGamma",
    "Tank_Upgrade_ChinaTankAutoLoader",
    "SupW_Upgrade_AmericaPointDefenseDrone",
    "GC_Slth_Upgrade_GLAQuadCannonSnipe",
    "Demo_Upgrade_GLADemoTrapHighExplosiveBomb",
    "Nuke_Upgrade_ChinaWGUraniumShells",
    "Upgrade_ChinaIsotopeStability",
    "Nuke_Upgrade_ChinaFusionReactors",
    "Nuke_Upgrade_HelixNukeBomb",
    "Demo_Upgrade_SuicideBomb",
    "AirF_Upgrade_StealthComanche",
    "RocketBuggyToxinUpgrade",
];

/// Whether an upgrade internal name is in the residual table.
pub fn upgrade_residual_name_known(name: &str) -> bool {
    UPGRADE_RESIDUAL_NAME_TABLE.iter().any(|n| *n == name)
}

/// Wave 94 honesty: Upgrade.ini full residual name table completeness.
///
/// Fail-closed: not full UpgradeCenter cost/time application matrix
/// (Wave 79 owns host cost/time residual for common kinds).
pub fn honesty_upgrade_name_table_residual_wave94() -> bool {
    if UPGRADE_RESIDUAL_NAME_TABLE.len() != UPGRADE_RESIDUAL_NAME_COUNT {
        return false;
    }
    let mut seen = std::collections::HashSet::new();
    for n in UPGRADE_RESIDUAL_NAME_TABLE {
        if !seen.insert(*n) {
            return false;
        }
    }
    // Anchors across factions + generals.
    let anchors = [
        "Upgrade_Nationalism",
        "Upgrade_Fanaticism",
        "Upgrade_AmericaSupplyLines",
        "Upgrade_AmericaRangerFlashBangGrenade",
        "Upgrade_AmericaTOWMissile",
        "Upgrade_ComancheRocketPods",
        "Upgrade_AmericaBunkerBusters",
        "Upgrade_AmericaCompositeArmor",
        "Upgrade_InfantryCaptureBuilding",
        "Upgrade_AmericaMOAB",
        "Upgrade_ChinaMines",
        "Upgrade_ChinaNeutronShells",
        "Upgrade_ChinaNuclearTanks",
        "Upgrade_ChinaSatelliteHackTwo",
        "Upgrade_GLAWorkerShoes",
        "Upgrade_GLACamouflage",
        "Upgrade_GLACamoNetting",
        "Upgrade_GLAAnthraxBeta",
        "Upgrade_GLAAPBullets",
        "Upgrade_GLAArmTheMob",
        "Upgrade_GLAInfantryRebelBoobyTrapAttack",
        "Upgrade_ChinaOverlordGattlingCannon",
        "Upgrade_HelixNapalmBomb",
        "Chem_Upgrade_GLAAnthraxGamma",
        "Demo_Upgrade_SuicideBomb",
        "Nuke_Upgrade_HelixNukeBomb",
        "AirF_Upgrade_StealthComanche",
        "SupW_Upgrade_AmericaPointDefenseDrone",
        "RocketBuggyToxinUpgrade",
    ];
    anchors.iter().all(|a| upgrade_residual_name_known(a))
        && UPGRADE_RESIDUAL_NAME_TABLE[0] == "Upgrade_Nationalism"
        && UPGRADE_RESIDUAL_NAME_TABLE[5] == "Upgrade_AmericaSupplyLines"
        && UPGRADE_RESIDUAL_NAME_TABLE[UPGRADE_RESIDUAL_NAME_COUNT - 1] == "RocketBuggyToxinUpgrade"
        && UPGRADE_RESIDUAL_NAME_COUNT == 81
}

// ---------------------------------------------------------------------------
// 4. CommandSet residual for superweapon buildings (+ command centers)
// ---------------------------------------------------------------------------

/// Max retail command-set button slots residual (CommandSet.ini 1..14).
pub const COMMAND_SET_SLOT_COUNT_RESIDUAL: usize = 14;

/// One CommandSet slot residual: (1-based slot, Command_* button name).
pub type CommandSetSlotResidual = (u8, &'static str);

/// CommandSet residual pack for a building.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandSetBuildingResidual {
    pub command_set_name: &'static str,
    pub object_template: &'static str,
    pub slots: &'static [CommandSetSlotResidual],
}

/// America Particle Uplink Cannon CommandSet residual.
pub const AMERICA_PARTICLE_UPLINK_COMMAND_SET: CommandSetBuildingResidual =
    CommandSetBuildingResidual {
        command_set_name: "AmericaParticleUplinkCannonCommandSet",
        object_template: "AmericaParticleCannonUplink",
        slots: &[
            (1, "Command_FireParticleUplinkCannon"),
            (14, "Command_Sell"),
        ],
    };

/// GLA Scud Storm CommandSet residual.
pub const GLA_SCUD_STORM_COMMAND_SET: CommandSetBuildingResidual = CommandSetBuildingResidual {
    command_set_name: "GLAScudStormCommandSet",
    object_template: "GLAScudStorm",
    slots: &[(1, "Command_ScudStorm"), (14, "Command_Sell")],
};

/// China Nuclear Missile Launcher CommandSet residual.
pub const CHINA_NUCLEAR_MISSILE_COMMAND_SET: CommandSetBuildingResidual =
    CommandSetBuildingResidual {
        command_set_name: "ChinaNuclearMissileCommandSet",
        object_template: "ChinaNuclearMissileLauncher",
        slots: &[
            (1, "Command_NeutronMissile"),
            (7, "Command_UpgradeChinaUraniumShells"),
            (8, "Command_UpgradeChinaNuclearTanks"),
            (10, "Command_UpgradeChinaNeutronShells"),
            (12, "Command_UpgradeChinaMines"),
            (14, "Command_Sell"),
        ],
    };

/// America Command Center CommandSet residual (superweapon science buttons).
pub const AMERICA_COMMAND_CENTER_COMMAND_SET: CommandSetBuildingResidual =
    CommandSetBuildingResidual {
        command_set_name: "AmericaCommandCenterCommandSet",
        object_template: "AmericaCommandCenter",
        slots: &[
            (1, "Command_ConstructAmericaDozer"),
            (2, "Command_SpectreGunship"),
            (4, "Command_LeafletDrop"),
            (5, "Command_A10ThunderboltMissileStrike"),
            (6, "Command_Paradrop"),
            (7, "Command_SpyDrone"),
            (8, "Command_EmergencyRepair"),
            (9, "Command_DaisyCutter"),
            (10, "Command_SpySatelliteScan"),
            (13, "Command_SetRallyPoint"),
            (14, "Command_Sell"),
        ],
    };

/// China Command Center CommandSet residual.
pub const CHINA_COMMAND_CENTER_COMMAND_SET: CommandSetBuildingResidual =
    CommandSetBuildingResidual {
        command_set_name: "ChinaCommandCenterCommandSet",
        object_template: "ChinaCommandCenter",
        slots: &[
            (1, "Command_ConstructChinaDozer"),
            (2, "Early_Command_ChinaCarpetBomb"),
            (3, "Command_NapalmStrike"),
            (4, "Command_ClusterMines"),
            (5, "Command_CashHack"),
            (6, "Command_ArtilleryBarrage"),
            (7, "Command_EmergencyRepair"),
            (8, "Command_EMPPulse"),
            (9, "Command_UpgradeChinaRadar"),
            (10, "Command_Frenzy"),
            (12, "Command_UpgradeChinaMines"),
            (13, "Command_SetRallyPoint"),
            (14, "Command_Sell"),
        ],
    };

/// GLA Command Center CommandSet residual.
pub const GLA_COMMAND_CENTER_COMMAND_SET: CommandSetBuildingResidual = CommandSetBuildingResidual {
    command_set_name: "GLACommandCenterCommandSet",
    object_template: "GLACommandCenter",
    slots: &[
        (1, "Command_ConstructGLAWorker"),
        (4, "Command_GPSScrambler"),
        (5, "Command_Ambush"),
        (6, "Command_EmergencyRepair"),
        (7, "Command_AnthraxBomb"),
        (8, "Command_SneakAttack"),
        (13, "Command_SetRallyPoint"),
        (14, "Command_Sell"),
    ],
};

/// America Strategy Center CommandSet residual (battle plans + science upgrades).
pub const AMERICA_STRATEGY_CENTER_COMMAND_SET: CommandSetBuildingResidual =
    CommandSetBuildingResidual {
        command_set_name: "AmericaStrategyCenterCommandSet",
        object_template: "AmericaStrategyCenter",
        slots: &[
            (1, "Command_InitiateBattlePlanBombardment"),
            (2, "Command_CIAIntelligence"),
            (3, "Command_InitiateBattlePlanHoldTheLine"),
            (5, "Command_InitiateBattlePlanSearchAndDestroy"),
            (6, "Command_UpgradeAmericaChemicalSuits"),
            (7, "Command_UpgradeAmericaMOAB"),
            (8, "Command_UpgradeAmericaCompositeArmor"),
            (9, "Command_UpgradeAmericaAdvancedTraining"),
            (10, "Command_UpgradeAmericaDroneArmor"),
            (11, "Command_StrategyCenter_Stop"),
            (13, "Command_UpgradeAmericaSupplyLines"),
            (14, "Command_Sell"),
        ],
    };

/// All Wave 94 CommandSet residual packs for iteration.
pub const COMMAND_SET_BUILDING_RESIDUAL_PACKS: &[CommandSetBuildingResidual] = &[
    AMERICA_PARTICLE_UPLINK_COMMAND_SET,
    GLA_SCUD_STORM_COMMAND_SET,
    CHINA_NUCLEAR_MISSILE_COMMAND_SET,
    AMERICA_COMMAND_CENTER_COMMAND_SET,
    CHINA_COMMAND_CENTER_COMMAND_SET,
    GLA_COMMAND_CENTER_COMMAND_SET,
    AMERICA_STRATEGY_CENTER_COMMAND_SET,
];

/// Lookup residual command button name at a 1-based slot.
pub fn command_set_slot_button(
    pack: &CommandSetBuildingResidual,
    slot: u8,
) -> Option<&'static str> {
    pack.slots.iter().find(|(s, _)| *s == slot).map(|(_, b)| *b)
}

/// Whether residual CommandSet has a button containing the given command fragment.
pub fn command_set_has_button(pack: &CommandSetBuildingResidual, button: &str) -> bool {
    pack.slots.iter().any(|(_, b)| *b == button)
}

/// Wave 94 honesty: CommandSet residual for superweapon buildings + CCs.
///
/// Fail-closed: not full ControlBar CommandSet slot UI matrix.
pub fn honesty_command_set_superweapon_residual_wave94() -> bool {
    COMMAND_SET_SLOT_COUNT_RESIDUAL == 14
        && COMMAND_SET_BUILDING_RESIDUAL_PACKS.len() == 7
        // Pure superweapon buildings: fire button slot 1 + Sell slot 14.
        && command_set_slot_button(&AMERICA_PARTICLE_UPLINK_COMMAND_SET, 1)
            == Some("Command_FireParticleUplinkCannon")
        && command_set_slot_button(&AMERICA_PARTICLE_UPLINK_COMMAND_SET, 14)
            == Some("Command_Sell")
        && AMERICA_PARTICLE_UPLINK_COMMAND_SET.slots.len() == 2
        && command_set_slot_button(&GLA_SCUD_STORM_COMMAND_SET, 1) == Some("Command_ScudStorm")
        && command_set_slot_button(&GLA_SCUD_STORM_COMMAND_SET, 14) == Some("Command_Sell")
        && command_set_slot_button(&CHINA_NUCLEAR_MISSILE_COMMAND_SET, 1)
            == Some("Command_NeutronMissile")
        && command_set_has_button(
            &CHINA_NUCLEAR_MISSILE_COMMAND_SET,
            "Command_UpgradeChinaNeutronShells",
        )
        && command_set_has_button(
            &CHINA_NUCLEAR_MISSILE_COMMAND_SET,
            "Command_UpgradeChinaNuclearTanks",
        )
        && CHINA_NUCLEAR_MISSILE_COMMAND_SET.slots.len() == 6
        // Command centers: superweapon science residual buttons.
        && command_set_has_button(&AMERICA_COMMAND_CENTER_COMMAND_SET, "Command_DaisyCutter")
        && command_set_has_button(
            &AMERICA_COMMAND_CENTER_COMMAND_SET,
            "Command_A10ThunderboltMissileStrike",
        )
        && command_set_has_button(&AMERICA_COMMAND_CENTER_COMMAND_SET, "Command_SpectreGunship")
        && command_set_has_button(&AMERICA_COMMAND_CENTER_COMMAND_SET, "Command_Paradrop")
        && command_set_slot_button(&AMERICA_COMMAND_CENTER_COMMAND_SET, 9)
            == Some("Command_DaisyCutter")
        && command_set_has_button(&CHINA_COMMAND_CENTER_COMMAND_SET, "Command_ArtilleryBarrage")
        && command_set_has_button(&CHINA_COMMAND_CENTER_COMMAND_SET, "Command_EMPPulse")
        && command_set_has_button(&CHINA_COMMAND_CENTER_COMMAND_SET, "Command_ClusterMines")
        && command_set_has_button(&CHINA_COMMAND_CENTER_COMMAND_SET, "Command_Frenzy")
        && command_set_has_button(&GLA_COMMAND_CENTER_COMMAND_SET, "Command_AnthraxBomb")
        && command_set_has_button(&GLA_COMMAND_CENTER_COMMAND_SET, "Command_SneakAttack")
        && command_set_has_button(&GLA_COMMAND_CENTER_COMMAND_SET, "Command_Ambush")
        && command_set_has_button(&GLA_COMMAND_CENTER_COMMAND_SET, "Command_GPSScrambler")
        // Strategy center residual battle plans.
        && command_set_has_button(
            &AMERICA_STRATEGY_CENTER_COMMAND_SET,
            "Command_InitiateBattlePlanBombardment",
        )
        && command_set_has_button(
            &AMERICA_STRATEGY_CENTER_COMMAND_SET,
            "Command_CIAIntelligence",
        )
        && command_set_has_button(
            &AMERICA_STRATEGY_CENTER_COMMAND_SET,
            "Command_UpgradeAmericaMOAB",
        )
        // Cross-link Wave 80 superweapon kindof CommandSet name residual.
        && AMERICA_PARTICLE_UPLINK_COMMAND_SET.command_set_name
            == crate::game_logic::host_superweapon_kindof::PARTICLE_CANNON_COMMAND_SET
        && GLA_SCUD_STORM_COMMAND_SET.command_set_name
            == crate::game_logic::host_superweapon_kindof::SCUD_STORM_COMMAND_SET
        && CHINA_NUCLEAR_MISSILE_COMMAND_SET.command_set_name
            == crate::game_logic::host_superweapon_kindof::NUCLEAR_MISSILE_COMMAND_SET
        // Slot numbers in range 1..14, unique per pack, Command_* / Early_Command_* names.
        && COMMAND_SET_BUILDING_RESIDUAL_PACKS.iter().all(|p| {
            let mut slots = std::collections::HashSet::new();
            p.slots.iter().all(|(s, b)| {
                *s >= 1
                    && (*s as usize) <= COMMAND_SET_SLOT_COUNT_RESIDUAL
                    && (b.starts_with("Command_") || b.starts_with("Early_Command_"))
                    && slots.insert(*s)
            })
        })
}

// ---------------------------------------------------------------------------
// Combined Wave 94 pack
// ---------------------------------------------------------------------------

/// Combined Wave 94 honesty pack (AI / special ability / upgrade / CommandSet).
pub fn honesty_ai_ability_upgrade_residual_pack_wave94() -> bool {
    honesty_ai_state_residual_table_wave94()
        && honesty_special_ability_residual_deepen_wave94()
        && honesty_upgrade_name_table_residual_wave94()
        && honesty_command_set_superweapon_residual_wave94()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_state_residual_table_wave94_honesty() {
        assert!(honesty_ai_state_residual_table_wave94());
        assert_eq!(NUM_AI_STATES_RESIDUAL, 44);
        assert_eq!(AI_STATE_TYPE_NAME_TABLE[29], "AI_HACK_INTERNET");
    }

    #[test]
    fn special_ability_residual_deepen_wave94_honesty() {
        assert!(honesty_special_ability_residual_deepen_wave94());
        assert_eq!(
            special_ability_reload_ms("SpecialAbilityTankHunterTNTAttack"),
            Some(7_500)
        );
        assert_eq!(special_ability_reload_frames(7_500), 225);
    }

    #[test]
    fn upgrade_name_table_wave94_honesty() {
        assert!(honesty_upgrade_name_table_residual_wave94());
        assert!(upgrade_residual_name_known("Demo_Upgrade_SuicideBomb"));
        assert_eq!(UPGRADE_RESIDUAL_NAME_COUNT, 81);
    }

    #[test]
    fn command_set_superweapon_wave94_honesty() {
        assert!(honesty_command_set_superweapon_residual_wave94());
        assert_eq!(
            command_set_slot_button(&GLA_SCUD_STORM_COMMAND_SET, 1),
            Some("Command_ScudStorm")
        );
    }

    #[test]
    fn ai_ability_upgrade_residual_pack_wave94_honesty() {
        assert!(honesty_ai_ability_upgrade_residual_pack_wave94());
    }
}
