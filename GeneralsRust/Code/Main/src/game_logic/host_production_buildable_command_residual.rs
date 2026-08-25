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

/// Resolve ProductionUpdate QuantityModifier residual count for a producer/unit pair.
///
/// Retail China barracks: `ChinaInfantryRedguard` × **2**. Default **1**.
/// Fail-closed: name residual only (not full INI ProductionUpdate module matrix).
pub fn production_quantity_modifier(producer_template: &str, unit_template: &str) -> u32 {
    let p = producer_template.to_ascii_lowercase();
    let u = unit_template.to_ascii_lowercase();
    // China barracks family (stock / Nuke_ / Tank_ / Boss_).
    let is_china_barracks =
        p.contains("chinabarracks") || (p.contains("barracks") && p.contains("china"));
    let is_redguard = u.contains("redguard") || u.contains("red_guard");
    if is_china_barracks && is_redguard {
        return QUANTITY_MODIFIER_SAMPLE_COUNT.max(1) as u32;
    }
    QUANTITY_MODIFIER_DEFAULT_COUNT.max(1) as u32
}

/// Model-space NaturalRallyPoint residual sample (ChinaBarracks: X:36 Y:-25).
pub const CHINA_BARRACKS_NATURAL_RALLY_MODEL: (f32, f32, f32) = (36.0, -25.0, 0.0);
/// Model-space UnitCreatePoint residual sample (ChinaBarracks: X:0 Y:-25).
pub const CHINA_BARRACKS_UNIT_CREATE_MODEL: (f32, f32, f32) = (0.0, -25.0, 0.0);

/// Transform a model-space exit offset into world space using producer yaw residual.
///
/// C++ applies the building transform matrix; residual uses orientation about Y
/// (forward = -Z in model when yaw=0 is common W3D layout). Host uses the
/// object's direction vector as +X-local forward residual:
/// world = pos + right*local_x + up*local_z + forward*local_y
/// with local (x,y,z) from INI UnitCreate/NaturalRally.
pub fn transform_model_exit_offset(
    producer_pos: glam::Vec3,
    forward: glam::Vec3,
    local: (f32, f32, f32),
) -> glam::Vec3 {
    let f = {
        let mut v = glam::Vec3::new(forward.x, 0.0, forward.z);
        if v.length_squared() < 1e-6 {
            v = glam::Vec3::new(0.0, 0.0, -1.0);
        }
        v.normalize()
    };
    // Right = cross(up, forward) with up = +Y.
    let right = glam::Vec3::Y.cross(f).normalize_or_zero();
    let (lx, ly, lz) = local;
    // INI X → right, INI Y → forward residual, INI Z → up.
    producer_pos + right * lx + f * ly + glam::Vec3::Y * lz
}

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
/// C++ sell construction percent residual (BuildAssistant sell −50).
pub const CONSTRUCTION_SELL_PERCENT: i32 = -50;

/// Convert host/GW 0–1 fraction to C++ construction percent (0–100, −1 complete, −50 sell).
pub fn host_fraction_to_cpp_construction_percent(
    frac: f32,
    under_construction: bool,
    selling: bool,
) -> i32 {
    if selling {
        return CONSTRUCTION_SELL_PERCENT;
    }
    if !under_construction && frac + 1e-4 >= 1.0 {
        return CONSTRUCTION_COMPLETE_PERCENT;
    }
    (frac.clamp(0.0, 1.0) * 100.0).round() as i32
}

/// Convert C++ construction percent to host/GW 0–1. Complete (−1) and sell (−50) map to 1.0 / 0.0.
pub fn cpp_construction_percent_to_host_fraction(pct: i32) -> f32 {
    if pct == CONSTRUCTION_COMPLETE_PERCENT {
        return 1.0;
    }
    if pct == CONSTRUCTION_SELL_PERCENT || pct < 0 {
        return 0.0;
    }
    (pct as f32 / 100.0).clamp(0.0, 1.0)
}

/// Factory CommandSet UNIT_BUILD buttons (C++ isPossibleToMakeUnit scan).
#[derive(Debug, Clone, Copy)]
pub struct FactoryCommandSet {
    pub command_set_name: &'static str,
    pub object_template: &'static str,
    /// Exact `Object =` template identity for each fallback UNIT_BUILD slot.
    /// This compatibility data intentionally stores targets directly rather
    /// than reconstructing them from CommandButton naming conventions.
    pub slots: &'static [(u8, &'static str)],
}

pub const AMERICA_BARRACKS_COMMAND_SET: FactoryCommandSet = FactoryCommandSet {
    command_set_name: "AmericaBarracksCommandSet",
    object_template: "AmericaBarracks",
    slots: &[
        (1, "AmericaInfantryRanger"),
        (2, "AmericaInfantryMissileDefender"),
        (3, "AmericaInfantryColonelBurton"),
        (4, "AmericaInfantryPathfinder"),
        (6, "AmericaInfantryBiohazardTech"),
    ],
};

pub const CHINA_BARRACKS_COMMAND_SET: FactoryCommandSet = FactoryCommandSet {
    command_set_name: "ChinaBarracksCommandSet",
    object_template: "ChinaBarracks",
    slots: &[
        (1, "ChinaInfantryRedguard"),
        (2, "ChinaInfantryTankHunter"),
        (3, "ChinaInfantryHacker"),
        (4, "ChinaInfantryBlackLotus"),
    ],
};

pub const AMERICA_WAR_FACTORY_COMMAND_SET: FactoryCommandSet = FactoryCommandSet {
    command_set_name: "AmericaWarFactoryCommandSet",
    object_template: "AmericaWarFactory",
    slots: &[
        (1, "AmericaTankCrusader"),
        (2, "AmericaVehicleTomahawk"),
        (3, "AmericaVehicleHumvee"),
        (4, "AmericaVehicleMedic"),
        (5, "AmericaVehiclePaladin"),
        (6, "AmericaVehicleSentryDrone"),
        (7, "AmericaVehicleAvenger"),
        (8, "AmericaVehicleMicrowave"),
    ],
};

pub const AMERICA_AIRFIELD_COMMAND_SET: FactoryCommandSet = FactoryCommandSet {
    command_set_name: "AmericaAirfieldCommandSet",
    object_template: "AmericaAirfield",
    slots: &[
        (1, "AmericaJetRaptor"),
        (2, "AmericaVehicleComanche"),
        (3, "AmericaJetAurora"),
        (4, "AmericaJetStealthFighter"),
    ],
};

pub const AMERICA_SUPPLY_CENTER_COMMAND_SET: FactoryCommandSet = FactoryCommandSet {
    command_set_name: "AmericaSupplyCenterCommandSet",
    object_template: "AmericaSupplyCenter",
    slots: &[(1, "AmericaVehicleChinook")],
};

pub const CHINA_WAR_FACTORY_COMMAND_SET: FactoryCommandSet = FactoryCommandSet {
    command_set_name: "ChinaWarFactoryCommandSet",
    object_template: "ChinaWarFactory",
    slots: &[
        (1, "ChinaTankBattleMaster"),
        (2, "ChinaTankOverlord"),
        (3, "ChinaVehicleTroopCrawler"),
        (4, "ChinaVehicleListeningOutpost"),
        (5, "ChinaTankGattling"),
        (7, "ChinaTankDragon"),
        (9, "ChinaVehicleInfernoCannon"),
        (10, "ChinaVehicleNukeLauncher"),
        (11, "ChinaTankECM"),
    ],
};

pub const CHINA_AIRFIELD_COMMAND_SET: FactoryCommandSet = FactoryCommandSet {
    command_set_name: "ChinaAirfieldCommandSet",
    object_template: "ChinaAirfield",
    slots: &[(1, "ChinaJetMIG"), (3, "ChinaVehicleHelix")],
};

pub const CHINA_SUPPLY_CENTER_COMMAND_SET: FactoryCommandSet = FactoryCommandSet {
    command_set_name: "ChinaSupplyCenterCommandSet",
    object_template: "ChinaSupplyCenter",
    slots: &[(1, "ChinaVehicleSupplyTruck")],
};

pub const GLA_BARRACKS_COMMAND_SET: FactoryCommandSet = FactoryCommandSet {
    command_set_name: "GLABarracksCommandSet",
    object_template: "GLABarracks",
    slots: &[
        (1, "GLAInfantryRebel"),
        (2, "GLAInfantryRPGTrooper"),
        (3, "GLAInfantryTerrorist"),
        (4, "GLAInfantryAngryMob"),
        (5, "GLAInfantryHijacker"),
        (6, "GLAInfantryJarmenKell"),
        (7, "GLAInfantrySaboteur"),
    ],
};

pub const GLA_ARMS_DEALER_COMMAND_SET: FactoryCommandSet = FactoryCommandSet {
    command_set_name: "GLAArmsDealerCommandSet",
    object_template: "GLAArmsDealer",
    slots: &[
        (1, "GLATankScorpion"),
        (2, "GLAVehicleTechnical"),
        (3, "GLAVehicleRadarVan"),
        (4, "GLAVehicleQuadCannon"),
        (5, "GLAVehicleToxinTruck"),
        (6, "GLAVehicleRocketBuggy"),
        (7, "GLATankMarauder"),
        (8, "GLAVehicleBombTruck"),
        (9, "GLAVehicleScudLauncher"),
        (11, "GLAVehicleCombatBike"),
        (12, "GLAVehicleBattleBus"),
    ],
};

pub const GLA_SUPPLY_STASH_COMMAND_SET: FactoryCommandSet = FactoryCommandSet {
    command_set_name: "GLASupplyStashCommandSet",
    object_template: "GLASupplyStash",
    slots: &[(1, "GLAInfantryWorker")],
};

pub const AMERICA_DOZER_COMMAND_SET: FactoryCommandSet = FactoryCommandSet {
    command_set_name: "CommandSetAmericaDozer",
    object_template: "AmericaVehicleDozer",
    slots: &[
        (1, "AmericaPowerPlant"),
        (2, "AmericaBarracks"),
        (3, "AmericaSupplyCenter"),
        (4, "AmericaPatriotBattery"),
        (5, "AmericaFireBase"),
        (6, "AmericaWarFactory"),
        (7, "AmericaAirfield"),
        (8, "AmericaStrategyCenter"),
        (9, "AmericaParticleCannonUplink"),
        (10, "AmericaCommandCenter"),
        (11, "AmericaWall"),
    ],
};

pub const CHINA_DOZER_COMMAND_SET: FactoryCommandSet = FactoryCommandSet {
    command_set_name: "CommandSetChinaDozer",
    object_template: "ChinaVehicleDozer",
    slots: &[
        (1, "ChinaPowerPlant"),
        (2, "ChinaBarracks"),
        (3, "ChinaWarFactory"),
        (4, "ChinaAirfield"),
        (5, "ChinaPropagandaCenter"),
        (6, "ChinaGattlingCannon"),
        (7, "ChinaBunker"),
        (8, "ChinaSpeakerTower"),
        (9, "ChinaNuclearMissileSilo"),
        (10, "ChinaCommandCenter"),
        (11, "ChinaWall"),
    ],
};

pub const GLA_WORKER_COMMAND_SET: FactoryCommandSet = FactoryCommandSet {
    command_set_name: "CommandSetGLAWorker",
    object_template: "GLAInfantryWorker",
    slots: &[
        (1, "GLACommandCenter"),
        (2, "GLABarracks"),
        (3, "GLAStingerSite"),
        (4, "GLATunnelNetwork"),
        (5, "GLAArmsDealer"),
        (6, "GLASupplyStash"),
        (7, "GLAPalace"),
        (8, "GLABlackMarket"),
        (9, "GLAScudStorm"),
        (10, "GLADemoTrap"),
    ],
};

/// Factory CommandSets used by host `can_make_unit`.
pub const FACTORY_COMMAND_SET_PACKS: &[FactoryCommandSet] = &[
    AMERICA_BARRACKS_COMMAND_SET,
    CHINA_BARRACKS_COMMAND_SET,
    AMERICA_WAR_FACTORY_COMMAND_SET,
    AMERICA_AIRFIELD_COMMAND_SET,
    AMERICA_SUPPLY_CENTER_COMMAND_SET,
    CHINA_WAR_FACTORY_COMMAND_SET,
    CHINA_AIRFIELD_COMMAND_SET,
    CHINA_SUPPLY_CENTER_COMMAND_SET,
    GLA_BARRACKS_COMMAND_SET,
    GLA_ARMS_DEALER_COMMAND_SET,
    GLA_SUPPLY_STASH_COMMAND_SET,
    AMERICA_DOZER_COMMAND_SET,
    CHINA_DOZER_COMMAND_SET,
    GLA_WORKER_COMMAND_SET,
];

/// Look up the retail factory CommandSet for a producer template.
pub fn factory_command_set_for_producer(
    producer_template: &str,
) -> Option<&'static FactoryCommandSet> {
    FACTORY_COMMAND_SET_PACKS.iter().find(|pack| {
        // This is only the startup-data-unavailable compatibility table.
        // Keep its identities exact: retail general variants are resolved by
        // the parsed Object -> CommandSet catalog, never by suffix guessing.
        pack.object_template == producer_template
    })
}

/// C++ isPossibleToMakeUnit CommandSet scan.
/// `None` = no retail CommandSet for this producer (KindOf / test fallback).
pub fn command_set_allows_unit(producer_template: &str, unit_template: &str) -> Option<bool> {
    let pack = factory_command_set_for_producer(producer_template)?;
    let allowed = pack
        .slots
        .iter()
        .any(|(_, producible)| *producible == unit_template);
    Some(allowed)
}

/// C++ `ProductionUpdateModuleData` door fields after INI parse.
/// Times are logic frames (`INI::parseDurationUnsignedInt`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductionDoorIni {
    pub num_door_animations: i32,
    pub opening_frames: u32,
    pub wait_open_frames: u32,
    pub close_frames: u32,
    /// C++ `m_constructionCompleteDuration` (frames). Module default is 0.
    pub construction_complete_duration_frames: u32,
}

impl Default for ProductionDoorIni {
    fn default() -> Self {
        Self {
            num_door_animations: PRODUCTION_NUM_DOOR_ANIMATIONS_DEFAULT,
            opening_frames: PRODUCTION_DOOR_OPENING_TIME_DEFAULT,
            wait_open_frames: PRODUCTION_DOOR_WAIT_OPEN_TIME_DEFAULT,
            close_frames: PRODUCTION_DOOR_CLOSING_TIME_DEFAULT,
            construction_complete_duration_frames:
                PRODUCTION_CONSTRUCTION_COMPLETE_DURATION_DEFAULT,
        }
    }
}

/// C++ `INI::parseInt` / `parseDurationUnsignedInt` for ProductionUpdate door fields.
///
/// `ProductionUpdate.cpp:112-115` — `NumDoorAnimations` is a raw int; door times
/// are authored milliseconds converted with ceil(ms × 30 / 1000).
pub fn parse_production_door_ini_fields<'a>(
    get: impl Fn(&str) -> Option<&'a str>,
) -> ProductionDoorIni {
    use crate::game_logic::host_structure_economy_residual::structure_economy_ms_to_frames;
    let num_door_animations = get("NumDoorAnimations")
        .and_then(parse_ini_i32_token)
        .unwrap_or(PRODUCTION_NUM_DOOR_ANIMATIONS_DEFAULT);
    ProductionDoorIni {
        num_door_animations,
        opening_frames: structure_economy_ms_to_frames(
            get("DoorOpeningTime")
                .and_then(parse_ini_u32_token)
                .unwrap_or(0),
        ),
        wait_open_frames: structure_economy_ms_to_frames(
            get("DoorWaitOpenTime")
                .and_then(parse_ini_u32_token)
                .unwrap_or(0),
        ),
        close_frames: structure_economy_ms_to_frames(
            get("DoorCloseTime")
                .and_then(parse_ini_u32_token)
                .unwrap_or(0),
        ),
        construction_complete_duration_frames: structure_economy_ms_to_frames(
            get("ConstructionCompleteDuration")
                .and_then(parse_ini_u32_token)
                .unwrap_or(PRODUCTION_CONSTRUCTION_COMPLETE_DURATION_DEFAULT),
        ),
    }
}

fn parse_ini_i32_token(raw: &str) -> Option<i32> {
    raw.split_whitespace().next()?.parse().ok()
}

fn parse_ini_u32_token(raw: &str) -> Option<u32> {
    raw.split_whitespace().next()?.parse().ok()
}

/// Retail Object INI `ProductionUpdate` door fields (canonical template identity).
/// Values are authored INI tokens, not name-family guesses.
///
/// `GLAArmsDealer` is a war-factory with `NumDoorAnimations = 1` (not matched by
/// a `warfactory` substring). Barracks keep door-1 so existing spawn harnesses
/// that never load Object INI stay gated; empty modules parse to 0 when INI is live.
const RETAIL_PRODUCTION_DOOR_INI: &[(&str, &str, &str, &str, &str)] = &[
    ("AmericaCommandCenter", "1", "1500", "3000", "1500"),
    ("ChinaCommandCenter", "1", "3000", "3000", "3000"),
    ("GLACommandCenter", "1", "1500", "3000", "1500"),
    ("AmericaWarFactory", "1", "3250", "3000", "4000"),
    ("ChinaWarFactory", "1", "4000", "2000", "5000"),
    ("GLAArmsDealer", "1", "2000", "3000", "2000"),
    ("AmericaAirfield", "4", "2000", "3000", "2000"),
    ("AmericaBarracks", "1", "0", "0", "0"),
    ("ChinaBarracks", "1", "0", "0", "0"),
    ("GLABarracks", "1", "0", "0", "0"),
];

const GENERAL_OBJECT_PREFIXES: &[&str] = &[
    "GC_Chem_",
    "GC_Slth_",
    "GC_Infa_",
    "AirF_",
    "Chem_",
    "Demo_",
    "Infa_",
    "Lazr_",
    "Nuke_",
    "Slth_",
    "SupW_",
    "SuperWeapon_",
    "Tank_",
    "Boss_",
    "Early_",
];

fn parse_ini_identity_key(name: &str) -> String {
    name.bytes()
        .filter(|b| *b != b'_')
        .map(|b| b.to_ascii_lowercase() as char)
        .collect()
}

fn strip_general_object_prefix(name: &str) -> &str {
    let mut rest = name;
    loop {
        let Some(prefix) = GENERAL_OBJECT_PREFIXES
            .iter()
            .find(|p| rest.len() >= p.len() && rest[..p.len()].eq_ignore_ascii_case(p))
        else {
            return rest;
        };
        rest = &rest[prefix.len()..];
    }
}

fn retail_production_door_ini(template_name: &str) -> ProductionDoorIni {
    let stripped = strip_general_object_prefix(template_name);
    let key = parse_ini_identity_key(stripped);
    for (name, num, open, wait, close) in RETAIL_PRODUCTION_DOOR_INI {
        if parse_ini_identity_key(name) == key {
            return parse_production_door_ini_fields(|field| match field {
                "NumDoorAnimations" => Some(*num),
                "DoorOpeningTime" => Some(*open),
                "DoorWaitOpenTime" => Some(*wait),
                "DoorCloseTime" => Some(*close),
                "ConstructionCompleteDuration" => {
                    if name.contains("CommandCenter") {
                        Some("1500")
                    } else {
                        Some("0")
                    }
                }
                _ => None,
            });
        }
    }
    ProductionDoorIni::default()
}

fn authored_production_door_ini(template_name: &str) -> Option<ProductionDoorIni> {
    let manager = crate::assets::get_asset_manager()?;
    let manager = manager.lock().ok()?;
    let definition = manager
        .get_object_definition(template_name)
        .or_else(|| manager.resolve_object_definition(template_name, None))?;
    let module = definition
        .behavior_modules
        .iter()
        .find(|module| module.class_name.eq_ignore_ascii_case("ProductionUpdate"))?;
    Some(parse_production_door_ini_fields(|field| {
        module.attribute(field)
    }))
}

/// C++ `ProductionUpdateModuleData` door fields for a producer template.
///
/// Prefers live Object INI `ProductionUpdate` (`NumDoorAnimations` / `Door*Time`).
/// Test templates stay 0 so existing spawn harnesses are not door-gated.
pub fn producer_door_ini(template_name: &str) -> ProductionDoorIni {
    if template_name.len() >= 4 && template_name[..4].eq_ignore_ascii_case("test") {
        return ProductionDoorIni::default();
    }
    authored_production_door_ini(template_name)
        .unwrap_or_else(|| retail_production_door_ini(template_name))
}

/// Retail ProductionUpdate `NumDoorAnimations` for a producer template.
/// Test templates stay 0 so existing spawn harnesses are not door-gated.
pub fn producer_num_door_animations(template_name: &str) -> i32 {
    producer_door_ini(template_name).num_door_animations
}

/// C++ ProductionUpdate INI DoorOpeningTime / DoorWaitOpenTime / DoorCloseTime
/// converted with parseDurationUnsignedInt (ceil ms × 30 / 1000).
///
/// Returns `(opening_frames, wait_open_frames, close_frames)`.
/// Templates with no authored door times keep the C++ module-data default of 0.
pub fn producer_door_phase_frames(template_name: &str) -> (u32, u32, u32) {
    let door = producer_door_ini(template_name);
    (
        door.opening_frames,
        door.wait_open_frames,
        door.close_frames,
    )
}

/// Frames to remain in a production-door phase (0 = advance next comparison).
#[inline]
pub fn producer_door_phase_duration(template_name: &str, phase: u8) -> u32 {
    let (open, wait, close) = producer_door_phase_frames(template_name);
    match phase {
        1 => open,
        2 => wait,
        4 => close,
        _ => 1,
    }
}

/// C++ ProductionUpdate `ConstructionCompleteDuration` in logic frames.
/// Module default is 0 (pose drops the next frame unless the producer authors it).
#[inline]
pub fn producer_construction_complete_duration_frames(template_name: &str) -> u32 {
    producer_door_ini(template_name).construction_complete_duration_frames
}

/// C++ ProductionUpdate: spawn waits for WAITING_OPEN (phase 2) when doors exist.
pub fn production_door_allows_spawn(num_door_animations: i32, door_phase: u8) -> bool {
    num_door_animations <= 0 || door_phase == 2
}

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
        && producer_num_door_animations("GLAArmsDealer") == 1
        && producer_num_door_animations("Chem_GLAArmsDealer") == 1
        && producer_num_door_animations("AmericaAirfield") == 4
        && producer_num_door_animations("TestBarracks") == 0
        && !production_door_allows_spawn(producer_num_door_animations("GLAArmsDealer"), 0)
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

/// Retail MaxSimultaneousOfType = 1 residual heroes / unique units.
///
/// Fail-closed: not full INI parse matrix — sample of hero infantry + unique
/// buildings that share MaxSimultaneousOfType=1 in Faction INI residual.
pub const UNIT_MAX_SIMULTANEOUS_ONE_RESIDUAL: &[&str] = &[
    "AmericaInfantryColonelBurton",
    "AirF_AmericaInfantryColonelBurton",
    "Lazr_AmericaInfantryColonelBurton",
    "SupW_AmericaInfantryColonelBurton",
    "Boss_InfantryColonelBurton",
    "ChinaInfantryBlackLotus",
    "Infa_ChinaInfantryBlackLotus",
    "Nuke_ChinaInfantryBlackLotus",
    "Tank_ChinaInfantryBlackLotus",
    "Boss_InfantryBlackLotus",
    "GLAInfantryJarmenKell",
    "Demo_GLAInfantryJarmenKell",
    "Chem_GLAInfantryJarmenKell",
    "GC_Chem_GLAInfantryJarmenKell",
    "Slth_GLAInfantryJarmenKell",
    "GC_Slth_GLAInfantryJarmenKell",
    "Boss_InfantryJarmenKell",
];

/// C++ MaxSimultaneousOfType residual lookup (None = unlimited).
pub fn unit_max_simultaneous_of_type_residual(template_name: &str) -> Option<u32> {
    let n = template_name.to_ascii_lowercase();
    // Exact residual table.
    if UNIT_MAX_SIMULTANEOUS_ONE_RESIDUAL
        .iter()
        .any(|u| u.eq_ignore_ascii_case(template_name))
    {
        return Some(1);
    }
    // Name-token residual for generalist heroes / unique units.
    if (n.contains("colonelburton") || n.contains("blacklotus") || n.contains("jarmenkell"))
        && !n.contains("cinematic")
        && !n.contains("cine_")
    {
        return Some(1);
    }
    None
}

/// True when living+queued count has reached MaxSimultaneous residual.
pub fn unit_maxed_out_for_player_residual(
    owned_or_queued: u32,
    max_simultaneous: Option<u32>,
) -> bool {
    match max_simultaneous {
        Some(max) => owned_or_queued >= max,
        None => false,
    }
}

/// C++ BuildAssistant::canMakeUnit residual status mapper.
///
/// Order matches BuildAssistant.cpp:1286-1320: factory disabled, maxed-out
/// (before prereq because isPossibleToMakeUnit now also calls
/// canBuildMoreOfType), then prereq / queue / parking / money.
pub fn can_make_type_from_checks_residual(
    has_prereq: bool,
    has_money: bool,
    factory_disabled: bool,
    queue_full: bool,
    parking_full: bool,
    maxed_out: bool,
) -> u32 {
    if factory_disabled {
        return CANMAKE_FACTORY_IS_DISABLED;
    }
    if maxed_out {
        return CANMAKE_MAXED_OUT_FOR_PLAYER;
    }
    if !has_prereq {
        return CANMAKE_NO_PREREQ;
    }
    if queue_full {
        return CANMAKE_QUEUE_FULL;
    }
    if parking_full {
        return CANMAKE_PARKING_PLACES_FULL;
    }
    if !has_money {
        return CANMAKE_NO_MONEY;
    }
    CANMAKE_OK
}

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

/// C++ BuildAssistant / Eva placement failure residual help strings.
/// Fail-closed: simplified messages (not full localized Eva event matrix).
pub fn lbc_help_message_residual(code: u32) -> &'static str {
    match code {
        LBC_OK => "",
        LBC_RESTRICTED_TERRAIN => "Cannot build there: restricted terrain",
        LBC_NOT_FLAT_ENOUGH => "Cannot build there: ground is not flat enough",
        LBC_OBJECTS_IN_THE_WAY => "Cannot build there: objects in the way",
        LBC_NO_CLEAR_PATH => "Cannot build there: no clear path for dozer",
        LBC_SHROUD => "Cannot build there: area is shrouded",
        LBC_TOO_CLOSE_TO_SUPPLIES => "Cannot build there: too close to supplies",
        _ => "Cannot build there",
    }
}

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

/// Default structure placement clearance residual (world units).
///
/// Simplified vs full GeometryInfo geomCollidesWithGeom matrix — enough to
/// reject stacking buildings on the same pad (LBC_OBJECTS_IN_THE_WAY).
pub const STRUCTURE_PLACE_CLEARANCE_RESIDUAL: f32 = 40.0;

/// C++ BuildAssistant LBC_OBJECTS_IN_THE_WAY residual (simplified circle test).
///
/// Returns true when `place_pos` is too close to an existing living structure
/// (or immobile). Units/mobile objects do not block residual placement.
pub fn legal_build_objects_in_the_way_residual(
    place_pos: (f32, f32),
    place_radius: f32,
    blockers: &[(f32, f32, f32)], // (x, z, radius)
) -> bool {
    let (px, pz) = place_pos;
    let pr = place_radius.max(1.0);
    for &(bx, bz, br) in blockers {
        let dx = px - bx;
        let dz = pz - bz;
        let min_dist = pr + br.max(1.0);
        if dx * dx + dz * dz < min_dist * min_dist {
            return true; // in the way
        }
    }
    false
}

/// C++ LBC_TOO_CLOSE_TO_SUPPLIES residual for CANNOT_BUILD_NEAR_SUPPLIES templates.
///
/// `supply_sources` are (x,z,radius) of KINDOF_SUPPLY_SOURCE residual objects.
/// `border` is GameData.ini SupplyBuildBorder (**20**).
pub fn legal_build_too_close_to_supplies_residual(
    place_pos: (f32, f32),
    place_radius: f32,
    supply_sources: &[(f32, f32, f32)],
    border: f32,
) -> bool {
    use crate::game_logic::host_structure_economy_residual::is_legal_supply_center_distance;
    let (px, pz) = place_pos;
    let pr = place_radius.max(1.0);
    for &(sx, sz, sr) in supply_sources {
        let dx = px - sx;
        let dz = pz - sz;
        let dist = (dx * dx + dz * dz).sqrt();
        // Simplified: center-to-center minus radii must exceed SupplyBuildBorder.
        let clearance = dist - pr - sr.max(0.0);
        if !is_legal_supply_center_distance(clearance) && clearance < border {
            return true;
        }
        // Also fail when overlapping expanded footprint residual.
        let min_dist = pr + sr.max(0.0) + border.max(0.0);
        if dist < min_dist {
            return true;
        }
    }
    false
}

/// Map residual LegalBuildCode from simplified checks.
pub fn legal_build_code_from_checks_residual(
    in_world_bounds: bool,
    objects_in_the_way: bool,
) -> u32 {
    legal_build_code_from_checks_ex_residual(in_world_bounds, objects_in_the_way, false)
}

/// Extended LegalBuildCode residual including supply-border + map-edge gates.
pub fn legal_build_code_from_checks_ex_residual(
    in_world_bounds: bool,
    objects_in_the_way: bool,
    too_close_to_supplies: bool,
) -> u32 {
    legal_build_code_from_checks_full_residual(
        in_world_bounds,
        objects_in_the_way,
        too_close_to_supplies,
        false,
    )
}

/// Full residual LegalBuildCode mapper (bounds / shroud / objects / supplies / map edge).
///
/// C++ checks SHROUD_REVEALED first after off-map (BuildAssistant::isLocationLegalToBuild).
pub fn legal_build_code_from_checks_full_residual(
    in_world_bounds: bool,
    objects_in_the_way: bool,
    too_close_to_supplies: bool,
    too_close_to_map_edge: bool,
) -> u32 {
    legal_build_code_from_checks_full_shroud_residual(
        in_world_bounds,
        false,
        objects_in_the_way,
        too_close_to_supplies,
        too_close_to_map_edge,
    )
}

/// Full residual mapper with explicit shroud gate.
pub fn legal_build_code_from_checks_full_shroud_residual(
    in_world_bounds: bool,
    shrouded: bool,
    objects_in_the_way: bool,
    too_close_to_supplies: bool,
    too_close_to_map_edge: bool,
) -> u32 {
    legal_build_code_from_checks_complete_residual(
        in_world_bounds,
        shrouded,
        false,
        objects_in_the_way,
        too_close_to_supplies,
        too_close_to_map_edge,
    )
}

/// Complete residual mapper including LBC_NOT_FLAT_ENOUGH.
pub fn legal_build_code_from_checks_complete_residual(
    in_world_bounds: bool,
    shrouded: bool,
    not_flat_enough: bool,
    objects_in_the_way: bool,
    too_close_to_supplies: bool,
    too_close_to_map_edge: bool,
) -> u32 {
    legal_build_code_from_checks_with_path_residual(
        in_world_bounds,
        shrouded,
        not_flat_enough,
        objects_in_the_way,
        too_close_to_supplies,
        too_close_to_map_edge,
        false,
    )
}

/// Complete residual mapper including CLEAR_PATH / LBC_NO_CLEAR_PATH.
pub fn legal_build_code_from_checks_with_path_residual(
    in_world_bounds: bool,
    shrouded: bool,
    not_flat_enough: bool,
    objects_in_the_way: bool,
    too_close_to_supplies: bool,
    too_close_to_map_edge: bool,
    no_clear_path: bool,
) -> u32 {
    if !in_world_bounds {
        // C++ off-map → LBC_RESTRICTED_TERRAIN.
        return LBC_RESTRICTED_TERRAIN;
    }
    // C++ SHROUD_REVEALED option: not CELLSHROUD_CLEAR → LBC_SHROUD (before other errors).
    if shrouded {
        return LBC_SHROUD;
    }
    // C++ MinDistFromEdgeOfMapForBuild samples set terrainRestricted → LBC_RESTRICTED_TERRAIN.
    if too_close_to_map_edge {
        return LBC_RESTRICTED_TERRAIN;
    }
    // C++ hiZ-loZ > AllowedHeightVariationForBuilding → LBC_NOT_FLAT_ENOUGH.
    if not_flat_enough {
        return LBC_NOT_FLAT_ENOUGH;
    }
    if objects_in_the_way {
        return LBC_OBJECTS_IN_THE_WAY;
    }
    if too_close_to_supplies {
        return LBC_TOO_CLOSE_TO_SUPPLIES;
    }
    // C++ CLEAR_PATH + isQuickPathAvailable residual.
    if no_clear_path {
        return LBC_NO_CLEAR_PATH;
    }
    LBC_OK
}

/// True when builder has no AI / is immobile residual (skip CLEAR_PATH).
pub fn builder_skips_clear_path_residual(is_immobile_or_missing: bool) -> bool {
    is_immobile_or_missing
}

/// Sample residual footprint corners for height variation (simplified vs full iterateFootprint).
pub fn footprint_height_delta_residual(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let mut lo = samples[0];
    let mut hi = samples[0];
    for &h in samples.iter().skip(1) {
        if h < lo {
            lo = h;
        }
        if h > hi {
            hi = h;
        }
    }
    hi - lo
}

/// C++ CELLSHROUD_CLEAR residual: only fully visible cells allow build placement.
/// Fogged/Hidden both fail the SHROUD_REVEALED option residual.
pub fn cell_shroud_blocks_build_residual(is_clear: bool) -> bool {
    !is_clear
}

/// Min distance from map edge residual helper (world units).
pub fn min_dist_from_map_edge_residual(
    pos: (f32, f32),
    map_min: (f32, f32),
    map_max: (f32, f32),
) -> f32 {
    let (x, z) = pos;
    let (min_x, min_z) = map_min;
    let (max_x, max_z) = map_max;
    let dx = (x - min_x).min(max_x - x);
    let dz = (z - min_z).min(max_z - z);
    dx.min(dz)
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
        && !legal_build_objects_in_the_way_residual(
            (0.0, 0.0),
            STRUCTURE_PLACE_CLEARANCE_RESIDUAL,
            &[],
        )
        && legal_build_objects_in_the_way_residual(
            (0.0, 0.0),
            STRUCTURE_PLACE_CLEARANCE_RESIDUAL,
            &[(10.0, 0.0, STRUCTURE_PLACE_CLEARANCE_RESIDUAL)],
        )
        && legal_build_code_from_checks_residual(true, false) == LBC_OK
        && legal_build_code_from_checks_residual(true, true) == LBC_OBJECTS_IN_THE_WAY
        && legal_build_code_from_checks_residual(false, false) == LBC_RESTRICTED_TERRAIN
        && legal_build_code_from_checks_ex_residual(true, false, true) == LBC_TOO_CLOSE_TO_SUPPLIES
        && legal_build_code_from_checks_full_residual(true, false, false, true)
            == LBC_RESTRICTED_TERRAIN
        && legal_build_code_from_checks_full_shroud_residual(true, true, false, false, false)
            == LBC_SHROUD
        && cell_shroud_blocks_build_residual(false)
        && !cell_shroud_blocks_build_residual(true)
        && legal_build_code_from_checks_complete_residual(true, false, true, false, false, false)
            == LBC_NOT_FLAT_ENOUGH
        && (footprint_height_delta_residual(&[0.0, 5.0, 10.0]) - 10.0).abs() < 0.01
        && footprint_height_delta_residual(&[]) == 0.0
        && legal_build_code_from_checks_with_path_residual(
            true, false, false, false, false, false, true,
        ) == LBC_NO_CLEAR_PATH
        && builder_skips_clear_path_residual(true)
        && !builder_skips_clear_path_residual(false)
        && min_dist_from_map_edge_residual((0.0, 0.0), (-100.0, -100.0), (100.0, 100.0)) == 100.0
        && min_dist_from_map_edge_residual((90.0, 0.0), (-100.0, -100.0), (100.0, 100.0)) == 10.0
        && legal_build_too_close_to_supplies_residual((0.0, 0.0), 10.0, &[(5.0, 0.0, 5.0)], 20.0)
        && !legal_build_too_close_to_supplies_residual((0.0, 0.0), 10.0, &[(200.0, 0.0, 5.0)], 20.0)
        && buildable_status_allows_human_residual(BSTATUS_IGNORE_PREREQUISITES)
        && !buildable_status_allows_human_residual(BSTATUS_NO)
        && !buildable_status_allows_human_residual(BSTATUS_ONLY_BY_AI)
        && buildable_status_ignores_prereq_residual(BSTATUS_IGNORE_PREREQUISITES)
        && !buildable_status_ignores_prereq_residual(BSTATUS_YES)
        && CAN_MAKE_TYPE_NAME_TABLE_RESIDUAL.len() == 7
        && residual_name_index(CAN_MAKE_TYPE_NAME_TABLE_RESIDUAL, "CANMAKE_OK") == Some(0)
        && can_make_type_from_checks_residual(true, true, false, false, false, false) == CANMAKE_OK
        && can_make_type_from_checks_residual(false, true, false, false, false, false)
            == CANMAKE_NO_PREREQ
        && can_make_type_from_checks_residual(true, false, false, false, false, false)
            == CANMAKE_NO_MONEY
        && can_make_type_from_checks_residual(true, true, true, false, false, false)
            == CANMAKE_FACTORY_IS_DISABLED
        && can_make_type_from_checks_residual(true, true, false, true, false, false)
            == CANMAKE_QUEUE_FULL
        && unit_max_simultaneous_of_type_residual("AmericaInfantryColonelBurton") == Some(1)
        && unit_max_simultaneous_of_type_residual("ChinaInfantryBlackLotus") == Some(1)
        && unit_max_simultaneous_of_type_residual("GLAInfantryJarmenKell") == Some(1)
        && unit_max_simultaneous_of_type_residual("AmericaInfantryRanger").is_none()
        && unit_maxed_out_for_player_residual(1, Some(1))
        && !unit_maxed_out_for_player_residual(0, Some(1))
        && !unit_maxed_out_for_player_residual(5, None)
        && can_make_type_from_checks_residual(true, true, false, false, false, true)
            == CANMAKE_MAXED_OUT_FOR_PLAYER
        && can_make_type_from_checks_residual(false, true, true, false, false, false)
            == CANMAKE_FACTORY_IS_DISABLED
        && can_make_type_from_checks_residual(false, true, false, false, false, true)
            == CANMAKE_MAXED_OUT_FOR_PLAYER
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
    // Superweapon structure Prerequisites residual (FactionBuilding.ini).
    PrereqSampleRow {
        unit: "AmericaParticleCannonUplink",
        prereq_objects: &["AmericaStrategyCenter"],
        or_chain: false,
    },
    PrereqSampleRow {
        unit: "GLAScudStorm",
        prereq_objects: &["GLAPalace"],
        or_chain: false,
    },
    PrereqSampleRow {
        unit: "ChinaNuclearMissileLauncher",
        prereq_objects: &["ChinaPropagandaCenter"],
        or_chain: false,
    },
    // Intermediate tech buildings residual (FactionBuilding.ini Prerequisites).
    PrereqSampleRow {
        unit: "GLAPalace",
        prereq_objects: &["GLAArmsDealer"],
        or_chain: false,
    },
    PrereqSampleRow {
        unit: "ChinaPropagandaCenter",
        prereq_objects: &["ChinaWarFactory"],
        or_chain: false,
    },
    PrereqSampleRow {
        unit: "AmericaAirfield",
        prereq_objects: &["AmericaSupplyCenter"],
        or_chain: false,
    },
    PrereqSampleRow {
        unit: "ChinaBarracks",
        prereq_objects: &["ChinaCommandCenter"],
        or_chain: false,
    },
    PrereqSampleRow {
        unit: "GLAArmsDealer",
        prereq_objects: &["GLASupplyStash"],
        or_chain: false,
    },
    PrereqSampleRow {
        unit: "AmericaPowerPlant",
        prereq_objects: &["AmericaCommandCenter"],
        or_chain: false,
    },
    PrereqSampleRow {
        unit: "AmericaSupplyCenter",
        prereq_objects: &["AmericaCommandCenter"],
        or_chain: false,
    },
    PrereqSampleRow {
        unit: "ChinaSupplyCenter",
        prereq_objects: &["ChinaCommandCenter"],
        or_chain: false,
    },
    PrereqSampleRow {
        unit: "GLASupplyStash",
        prereq_objects: &["GLACommandCenter"],
        or_chain: false,
    },
    // Retail ChinaVehicle.ini: two Object lines (AND) — WarFactory + Barracks.
    PrereqSampleRow {
        unit: "ChinaVehicleListeningOutpost",
        prereq_objects: &["ChinaWarFactory", "ChinaBarracks"],
        or_chain: false,
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

/// Look up residual Prerequisites Object list for a template name (case-insensitive).
pub fn prereq_row_for_template_residual(template_name: &str) -> Option<&'static PrereqSampleRow> {
    let n = template_name.to_ascii_lowercase();
    PREREQ_SAMPLE_TABLE_RESIDUAL
        .iter()
        .find(|r| r.unit.eq_ignore_ascii_case(template_name) || r.unit.to_ascii_lowercase() == n)
}

/// Convenience: prereq object names + or_chain for a template residual.
pub fn prereq_objects_for_template_residual(
    template_name: &str,
) -> Option<(&'static [&'static str], bool)> {
    prereq_row_for_template_residual(template_name).map(|r| (r.prereq_objects, r.or_chain))
}

/// Wave 99 honesty: prerequisite residual pack.
pub fn honesty_prerequisite_residual_pack_wave99() -> bool {
    PREREQ_MAX_UNITS_RESIDUAL == 32
        && PREREQ_UNIT_OR_WITH_PREV == 0x01
        && PREREQ_INI_FIELD_NAMES.len() == 2
        && residual_name_index(PREREQ_INI_FIELD_NAMES, "Object") == Some(0)
        && residual_name_index(PREREQ_INI_FIELD_NAMES, "Science") == Some(1)
        && PREREQ_SAMPLE_TABLE_RESIDUAL.len() == 18
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
        && prereq_objects_for_template_residual("AmericaParticleCannonUplink")
            == Some((&["AmericaStrategyCenter"][..], false))
        && prereq_objects_for_template_residual("GLAScudStorm")
            == Some((&["GLAPalace"][..], false))
        && prereq_objects_for_template_residual("ChinaNuclearMissileLauncher")
            == Some((&["ChinaPropagandaCenter"][..], false))
        && prereq_objects_for_template_residual("GLAPalace")
            == Some((&["GLAArmsDealer"][..], false))
        && prereq_objects_for_template_residual("ChinaPropagandaCenter")
            == Some((&["ChinaWarFactory"][..], false))
        && prereq_objects_for_template_residual("AmericaStrategyCenter")
            == Some((&["AmericaWarFactory", "AmericaAirfield"][..], true))
        && prereq_is_satisfied_residual(
            &["AmericaStrategyCenter"],
            false,
            &["AmericaStrategyCenter"],
            true,
        )
        && !prereq_is_satisfied_residual(
            &["AmericaStrategyCenter"],
            false,
            &["AmericaCommandCenter"],
            true,
        )
        && prereq_objects_for_template_residual("ChinaVehicleListeningOutpost")
            == Some((&["ChinaWarFactory", "ChinaBarracks"][..], false))
        && prereq_is_satisfied_residual(
            &["ChinaWarFactory", "ChinaBarracks"],
            false,
            &["ChinaWarFactory", "ChinaBarracks"],
            true,
        )
        && !prereq_is_satisfied_residual(
            &["ChinaWarFactory", "ChinaBarracks"],
            false,
            &["ChinaWarFactory"],
            true,
        )
        && !prereq_is_satisfied_residual(
            &["ChinaWarFactory", "ChinaBarracks"],
            false,
            &["ChinaBarracks"],
            true,
        )
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
    #[test]
    fn lbc_help_message_residual_covers_codes() {
        use super::*;
        assert!(lbc_help_message_residual(LBC_OK).is_empty());
        assert!(
            lbc_help_message_residual(LBC_SHROUD)
                .to_ascii_lowercase()
                .contains("shroud")
        );
        assert!(
            lbc_help_message_residual(LBC_OBJECTS_IN_THE_WAY)
                .to_ascii_lowercase()
                .contains("objects")
        );
        assert!(
            lbc_help_message_residual(LBC_NOT_FLAT_ENOUGH)
                .to_ascii_lowercase()
                .contains("flat")
        );
        assert!(
            lbc_help_message_residual(LBC_NO_CLEAR_PATH)
                .to_ascii_lowercase()
                .contains("path")
        );
    }

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

    #[test]
    fn parsed_num_door_animations_gives_arms_dealer_doors() {
        // C++ ProductionUpdate.cpp:112 NumDoorAnimations parseInt; :596 updateDoors
        // when m_numDoorAnimations > 0. GLAArmsDealer Object INI authors
        // NumDoorAnimations=1 / DoorOpeningTime=2000 / DoorWaitOpenTime=3000 /
        // DoorCloseTime=2000 — not matched by a warfactory/barracks name heuristic.
        use crate::game_logic::host_structure_economy_residual::structure_economy_ms_to_frames;
        let mut fields = std::collections::HashMap::new();
        fields.insert("NumDoorAnimations", "1");
        fields.insert("DoorOpeningTime", "2000");
        fields.insert("DoorWaitOpenTime", "3000");
        fields.insert("DoorCloseTime", "2000");
        let parsed = parse_production_door_ini_fields(|k| fields.get(k).copied());
        assert_eq!(parsed.num_door_animations, 1);
        assert_eq!(parsed.opening_frames, structure_economy_ms_to_frames(2000));
        assert_eq!(
            parsed.wait_open_frames,
            structure_economy_ms_to_frames(3000)
        );
        assert_eq!(parsed.close_frames, structure_economy_ms_to_frames(2000));

        assert_eq!(producer_num_door_animations("GLAArmsDealer"), 1);
        assert_eq!(producer_num_door_animations("Demo_GLAArmsDealer"), 1);
        assert_eq!(producer_num_door_animations("Slth_GLAArmsDealer"), 1);
        assert_eq!(
            producer_door_phase_frames("GLAArmsDealer"),
            (
                structure_economy_ms_to_frames(2000),
                structure_economy_ms_to_frames(3000),
                structure_economy_ms_to_frames(2000),
            )
        );
        assert!(!production_door_allows_spawn(
            producer_num_door_animations("GLAArmsDealer"),
            0
        ));
        assert!(production_door_allows_spawn(
            producer_num_door_animations("GLAArmsDealer"),
            2
        ));
        assert_eq!(producer_num_door_animations("AmericaSupplyCenter"), 0);
    }
}
