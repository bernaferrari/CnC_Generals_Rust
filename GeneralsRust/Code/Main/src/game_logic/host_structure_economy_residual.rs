//! Wave 83 residual peels: production queue / supply warehouse / dozer build /
//! capture building / power plant energy / command center.
//!
//! Host-testable INI residual packs for core base-economy structures and the
//! production/capture loops that make skirmish playable. Orthogonal to Wave 82
//! enum tables.
//!
//! Sources (retail ZH INI):
//! - ProductionUpdate.cpp MaxQueueEntries **9**; GameData.ini RefundPercent **50%**,
//!   Min/MaxLowEnergyProductionSpeed **0.5/0.8**, LowEnergyPenaltyModifier **1.0**,
//!   MultipleFactory **1.0**, BuildSpeed **1.0**
//! - CivilianBuilding.ini SupplyWarehouse StartingBoxes **400** / SupplyPile **150**
//!   / SupplyPileSmall **50**; GameData.ini ValuePerSupplyBox **75**
//! - AmericaVehicle.ini AmericaDozer DozerAIUpdate repair/bored + BuildCost **1000**
//!   / BuildTime **5**s / MaxHealth **250**; GameData MinDistFromEdge **30**,
//!   SupplyBuildBorder **20**, AllowedHeightVariationForBuilding **10**
//! - SpecialPower.ini + AmericaInfantryRanger capture residual
//! - AmericaPowerPlant / ChinaPowerPlant EnergyProduction / EnergyBonus / RodsExtend
//! - America/China/GLA CommandCenter BuildCost **2000** / BuildTime **45**s /
//!   MaxHealth **5000**; GameData CommandCenterHealRange **500** /
//!   HealAmount **0.01** per logic frame
//!
//! Fail-closed:
//! - Not full ProductionUpdate door-anim / QuantityModifier / parking-place matrix
//! - Not full SupplyWarehouseDockUpdate approach-bone path / ResourceGatheringManager
//! - Not full DozerAIUpdate primary state machine / construct scaffolding
//! - Not full CaptureBuilding BinaryDataStream / ActionManager edge matrix
//! - Not full PowerPlantUpgrade model-condition rod draw / OverchargeBehavior
//! - Not full CommandCenter radar-extend / PreorderCreate / grant-upgrade radar
//! - Shell `playable_claim` stays false; network deferred

use crate::game_logic::buildings::DEFAULT_PRODUCTION_QUEUE_LIMIT;

/// Logic frames per second residual.
pub const STRUCTURE_ECONOMY_LOGIC_FPS: f32 = 30.0;

// ---------------------------------------------------------------------------
// 1. Production queue residual (ProductionUpdate + GameData energy/refund)
// ---------------------------------------------------------------------------

/// C++ ProductionUpdateModuleData default MaxQueueEntries residual.
pub const PRODUCTION_MAX_QUEUE_ENTRIES: usize = 9;

/// Retail GameData.ini RefundPercent residual (cancel production refund fraction).
pub const PRODUCTION_REFUND_PERCENT: f32 = 0.5;

/// Retail GameData.ini MinLowEnergyProductionSpeed residual.
pub const MIN_LOW_ENERGY_PRODUCTION_SPEED: f32 = 0.5;
/// Retail GameData.ini MaxLowEnergyProductionSpeed residual.
pub const MAX_LOW_ENERGY_PRODUCTION_SPEED: f32 = 0.8;
/// Retail GameData.ini LowEnergyPenaltyModifier residual.
pub const LOW_ENERGY_PENALTY_MODIFIER: f32 = 1.0;
/// Retail GameData.ini MultipleFactory residual.
pub const MULTIPLE_FACTORY: f32 = 1.0;
/// Retail GameData.ini BuildSpeed residual.
pub const BUILD_SPEED: f32 = 1.0;

/// AmericaCommandCenter ProductionUpdate door residual (msec).
pub const USA_CC_DOOR_OPENING_MS: u32 = 1500;
pub const USA_CC_DOOR_WAIT_OPEN_MS: u32 = 3000;
pub const USA_CC_DOOR_CLOSE_MS: u32 = 1500;
pub const USA_CC_CONSTRUCTION_COMPLETE_DURATION_MS: u32 = 1500;
/// AmericaCommandCenter door residual frames @ 30 FPS.
pub const USA_CC_DOOR_OPENING_FRAMES: u32 = 45;
pub const USA_CC_DOOR_WAIT_OPEN_FRAMES: u32 = 90;
pub const USA_CC_DOOR_CLOSE_FRAMES: u32 = 45;
pub const USA_CC_CONSTRUCTION_COMPLETE_DURATION_FRAMES: u32 = 45;

/// ChinaCommandCenter ProductionUpdate door residual (msec).
pub const CHINA_CC_DOOR_OPENING_MS: u32 = 3000;
pub const CHINA_CC_DOOR_WAIT_OPEN_MS: u32 = 3000;
pub const CHINA_CC_DOOR_CLOSE_MS: u32 = 3000;
pub const CHINA_CC_CONSTRUCTION_COMPLETE_DURATION_MS: u32 = 1500;
pub const CHINA_CC_DOOR_OPENING_FRAMES: u32 = 90;
pub const CHINA_CC_DOOR_WAIT_OPEN_FRAMES: u32 = 90;
pub const CHINA_CC_DOOR_CLOSE_FRAMES: u32 = 90;
pub const CHINA_CC_CONSTRUCTION_COMPLETE_DURATION_FRAMES: u32 = 45;

/// Convert residual milliseconds → logic frames @ 30 FPS (ceil, parseDuration style).
pub fn structure_economy_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * (STRUCTURE_ECONOMY_LOGIC_FPS / 1000.0)).ceil() as u32
}

/// C++ ThingTemplate::calcTimeToBuild / Player energy residual power factor.
///
/// Retail GameData.ini:
///   energy_short = (1 - ratio) * LowEnergyPenaltyModifier
///   rate = max(1 - energy_short, MinLowEnergyProductionSpeed)
///   if ratio < 1: rate = min(rate, MaxLowEnergyProductionSpeed)
pub fn production_power_factor_from_energy_ratio(energy_ratio: f32) -> f32 {
    let ratio = energy_ratio.clamp(0.0, 1.0);
    if ratio >= 1.0 {
        return 1.0;
    }
    let energy_short = (1.0 - ratio) * LOW_ENERGY_PENALTY_MODIFIER;
    let mut rate = (1.0 - energy_short).max(MIN_LOW_ENERGY_PRODUCTION_SPEED);
    rate = rate.min(MAX_LOW_ENERGY_PRODUCTION_SPEED);
    rate.max(0.01)
}

/// Production cancel residual refund (GameData RefundPercent).
pub fn production_cancel_refund(cost: u32) -> u32 {
    ((cost as f32) * PRODUCTION_REFUND_PERCENT).floor() as u32
}

/// Whether a production queue can accept another entry residual.
pub fn production_queue_can_enqueue(current_len: usize, max_entries: usize) -> bool {
    current_len < max_entries
}

/// Wave 83 honesty: production queue residual pack.
pub fn honesty_production_queue_residual_pack_wave83() -> bool {
    PRODUCTION_MAX_QUEUE_ENTRIES == 9
        && DEFAULT_PRODUCTION_QUEUE_LIMIT == PRODUCTION_MAX_QUEUE_ENTRIES
        && (PRODUCTION_REFUND_PERCENT - 0.5).abs() < 0.001
        && (MIN_LOW_ENERGY_PRODUCTION_SPEED - 0.5).abs() < 0.001
        && (MAX_LOW_ENERGY_PRODUCTION_SPEED - 0.8).abs() < 0.001
        && (LOW_ENERGY_PENALTY_MODIFIER - 1.0).abs() < 0.001
        && (MULTIPLE_FACTORY - 1.0).abs() < 0.001
        && (BUILD_SPEED - 1.0).abs() < 0.001
        && USA_CC_DOOR_OPENING_FRAMES
            == structure_economy_ms_to_frames(USA_CC_DOOR_OPENING_MS)
        && USA_CC_DOOR_WAIT_OPEN_FRAMES
            == structure_economy_ms_to_frames(USA_CC_DOOR_WAIT_OPEN_MS)
        && USA_CC_DOOR_CLOSE_FRAMES == structure_economy_ms_to_frames(USA_CC_DOOR_CLOSE_MS)
        && USA_CC_CONSTRUCTION_COMPLETE_DURATION_FRAMES
            == structure_economy_ms_to_frames(USA_CC_CONSTRUCTION_COMPLETE_DURATION_MS)
        && CHINA_CC_DOOR_OPENING_FRAMES
            == structure_economy_ms_to_frames(CHINA_CC_DOOR_OPENING_MS)
        && CHINA_CC_DOOR_WAIT_OPEN_FRAMES
            == structure_economy_ms_to_frames(CHINA_CC_DOOR_WAIT_OPEN_MS)
        && CHINA_CC_DOOR_CLOSE_FRAMES == structure_economy_ms_to_frames(CHINA_CC_DOOR_CLOSE_MS)
        && CHINA_CC_CONSTRUCTION_COMPLETE_DURATION_FRAMES
            == structure_economy_ms_to_frames(CHINA_CC_CONSTRUCTION_COMPLETE_DURATION_MS)
        // Full power → factor 1.0.
        && (production_power_factor_from_energy_ratio(1.0) - 1.0).abs() < 0.001
        // Zero energy residual clamps to MinLowEnergyProductionSpeed (0.5), then
        // MaxLowEnergyProductionSpeed still applies (min 0.8) → 0.5.
        && (production_power_factor_from_energy_ratio(0.0) - 0.5).abs() < 0.001
        // Half energy: short=0.5, rate=0.5, max-cap 0.8 → 0.5.
        && (production_power_factor_from_energy_ratio(0.5) - 0.5).abs() < 0.001
        // 90% energy: short=0.1, rate=0.9 → capped by Max to 0.8.
        && (production_power_factor_from_energy_ratio(0.9) - 0.8).abs() < 0.001
        && production_cancel_refund(1000) == 500
        && production_cancel_refund(101) == 50
        && production_queue_can_enqueue(0, PRODUCTION_MAX_QUEUE_ENTRIES)
        && production_queue_can_enqueue(8, PRODUCTION_MAX_QUEUE_ENTRIES)
        && !production_queue_can_enqueue(9, PRODUCTION_MAX_QUEUE_ENTRIES)
}

// ---------------------------------------------------------------------------
// 2. Supply warehouse residual (SupplyWarehouseDockUpdate + GameData box value)
// ---------------------------------------------------------------------------

/// Retail GameData.ini ValuePerSupplyBox residual (ZH override of C++ default 100).
pub const VALUE_PER_SUPPLY_BOX: i32 = 75;

/// Retail SupplyWarehouse StartingBoxes residual.
pub const SUPPLY_WAREHOUSE_STARTING_BOXES: i32 = 400;
/// Retail SupplyWarehouse NumberApproachPositions residual.
pub const SUPPLY_WAREHOUSE_APPROACH_POSITIONS: i32 = 9;
/// Retail SupplyWarehouse ImmortalBody MaxHealth residual.
pub const SUPPLY_WAREHOUSE_MAX_HEALTH: f32 = 1000.0;

/// Retail SupplyDock StartingBoxes residual (map dock variant).
pub const SUPPLY_DOCK_STARTING_BOXES: i32 = 400;
/// Retail SupplyDock NumberApproachPositions residual (-1 = infinite boneless).
pub const SUPPLY_DOCK_APPROACH_POSITIONS: i32 = -1;

/// Retail SupplyPile StartingBoxes residual.
pub const SUPPLY_PILE_STARTING_BOXES: i32 = 150;
/// Retail SupplyPile NumberApproachPositions residual.
pub const SUPPLY_PILE_APPROACH_POSITIONS: i32 = 5;
/// Retail SupplyPile DeleteWhenEmpty residual.
pub const SUPPLY_PILE_DELETE_WHEN_EMPTY: bool = true;

/// Retail SupplyPileSmall StartingBoxes residual.
pub const SUPPLY_PILE_SMALL_STARTING_BOXES: i32 = 50;
/// Retail SupplyPileSmall DeleteWhenEmpty residual.
pub const SUPPLY_PILE_SMALL_DELETE_WHEN_EMPTY: bool = true;

/// Retail SupplyWarehouseCripplingBehavior SelfHealSupression residual (msec).
pub const SUPPLY_WAREHOUSE_SELF_HEAL_SUPPRESSION_MS: u32 = 3000;
/// SelfHealSupression 3000ms → 90 frames.
pub const SUPPLY_WAREHOUSE_SELF_HEAL_SUPPRESSION_FRAMES: u32 = 90;
/// Retail SelfHealDelay residual (msec).
pub const SUPPLY_WAREHOUSE_SELF_HEAL_DELAY_MS: u32 = 500;
/// SelfHealDelay 500ms → 15 frames.
pub const SUPPLY_WAREHOUSE_SELF_HEAL_DELAY_FRAMES: u32 = 15;
/// Retail SelfHealAmount residual.
pub const SUPPLY_WAREHOUSE_SELF_HEAL_AMOUNT: f32 = 5.0;

/// Warehouse cash residual from boxes × ValuePerSupplyBox.
pub fn supply_warehouse_cash_value(boxes: i32) -> i32 {
    boxes.saturating_mul(VALUE_PER_SUPPLY_BOX)
}

/// Boxes residual from cash value (ceil, C++ setCashValue style).
pub fn supply_warehouse_boxes_from_cash(cash: i32) -> i32 {
    if cash <= 0 || VALUE_PER_SUPPLY_BOX <= 0 {
        return 0;
    }
    ((cash as f32) / (VALUE_PER_SUPPLY_BOX as f32)).ceil() as i32
}

/// Deplete one residual supply box; returns remaining boxes + whether empty-delete.
pub fn supply_warehouse_take_one_box(
    boxes_stored: i32,
    delete_when_empty: bool,
) -> (i32, bool /*destroy*/) {
    let remaining = (boxes_stored - 1).max(0);
    let destroy = delete_when_empty && remaining == 0;
    (remaining, destroy)
}

/// True when template is a residual supply source warehouse/pile/dock.
pub fn is_supply_warehouse_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("supplywarehouse")
        || n.contains("supply_warehouse")
        || n == "supplydock"
        || n.contains("supplydock")
        || n.contains("supplypile")
        || n.contains("supply_pile")
}

/// Wave 83 honesty: supply warehouse residual pack.
pub fn honesty_supply_warehouse_residual_pack_wave83() -> bool {
    VALUE_PER_SUPPLY_BOX == 75
        && SUPPLY_WAREHOUSE_STARTING_BOXES == 400
        && SUPPLY_WAREHOUSE_APPROACH_POSITIONS == 9
        && (SUPPLY_WAREHOUSE_MAX_HEALTH - 1000.0).abs() < 0.01
        && SUPPLY_DOCK_STARTING_BOXES == 400
        && SUPPLY_DOCK_APPROACH_POSITIONS == -1
        && SUPPLY_PILE_STARTING_BOXES == 150
        && SUPPLY_PILE_APPROACH_POSITIONS == 5
        && SUPPLY_PILE_DELETE_WHEN_EMPTY
        && SUPPLY_PILE_SMALL_STARTING_BOXES == 50
        && SUPPLY_PILE_SMALL_DELETE_WHEN_EMPTY
        && SUPPLY_WAREHOUSE_SELF_HEAL_SUPPRESSION_FRAMES
            == structure_economy_ms_to_frames(SUPPLY_WAREHOUSE_SELF_HEAL_SUPPRESSION_MS)
        && SUPPLY_WAREHOUSE_SELF_HEAL_DELAY_FRAMES
            == structure_economy_ms_to_frames(SUPPLY_WAREHOUSE_SELF_HEAL_DELAY_MS)
        && (SUPPLY_WAREHOUSE_SELF_HEAL_AMOUNT - 5.0).abs() < 0.01
        && supply_warehouse_cash_value(400) == 30_000
        && supply_warehouse_cash_value(150) == 11_250
        && supply_warehouse_cash_value(50) == 3_750
        && supply_warehouse_boxes_from_cash(75) == 1
        && supply_warehouse_boxes_from_cash(76) == 2
        && supply_warehouse_boxes_from_cash(0) == 0
        && {
            let (rem, destroy) = supply_warehouse_take_one_box(1, true);
            rem == 0 && destroy
        }
        && {
            let (rem, destroy) = supply_warehouse_take_one_box(2, true);
            rem == 1 && !destroy
        }
        && {
            let (rem, destroy) = supply_warehouse_take_one_box(1, false);
            rem == 0 && !destroy
        }
        && is_supply_warehouse_template("SupplyWarehouse")
        && is_supply_warehouse_template("SupplyPileSmall")
        && is_supply_warehouse_template("SupplyDock")
        && !is_supply_warehouse_template("AmericaSupplyCenter")
}

// ---------------------------------------------------------------------------
// 3. Dozer build residual (DozerAIUpdate + AmericaDozer + GameData build pads)
// ---------------------------------------------------------------------------

/// Retail AmericaDozer BuildCost residual.
pub const DOZER_BUILD_COST: u32 = 1000;
/// Retail AmericaDozer BuildTime residual (seconds).
pub const DOZER_BUILD_TIME_SEC: f32 = 5.0;
/// Retail AmericaDozer BuildTime residual frames @ 30 FPS.
pub const DOZER_BUILD_TIME_FRAMES: u32 = 150;
/// Retail AmericaDozer ActiveBody MaxHealth residual.
pub const DOZER_MAX_HEALTH: f32 = 250.0;
/// Retail AmericaDozer VisionRange / ShroudClearingRange residual.
pub const DOZER_VISION_RANGE: f32 = 200.0;
/// Retail AmericaDozer TransportSlotCount residual.
pub const DOZER_TRANSPORT_SLOT_COUNT: u32 = 5;
/// Retail AmericaDozer CommandSet residual name.
pub const DOZER_COMMAND_SET: &str = "AmericaDozerCommandSet";
/// Retail AmericaDozer Locomotor residual name.
pub const DOZER_LOCOMOTOR: &str = "AmericaVehicleDozerLocomotor";
/// Retail DozerAIUpdate RepairHealthPercentPerSecond residual (= 2%).
pub const DOZER_REPAIR_HEALTH_PERCENT_PER_SEC: f32 = 0.02;
/// Retail BoredTime residual (msec).
pub const DOZER_BORED_TIME_MS: u32 = 5000;
/// BoredTime 5000ms → 150 frames.
pub const DOZER_BORED_TIME_FRAMES: u32 = 150;
/// Retail BoredRange residual.
pub const DOZER_BORED_RANGE: f32 = 150.0;

/// Retail GameData.ini MinDistFromEdgeOfMapForBuild residual.
pub const MIN_DIST_FROM_EDGE_OF_MAP_FOR_BUILD: f32 = 30.0;
/// Retail GameData.ini SupplyBuildBorder residual.
pub const SUPPLY_BUILD_BORDER: f32 = 20.0;
/// Retail GameData.ini AllowedHeightVariationForBuilding residual.
pub const ALLOWED_HEIGHT_VARIATION_FOR_BUILDING: f32 = 10.0;
/// Retail GameData.ini MaxLineBuildObjects residual.
pub const MAX_LINE_BUILD_OBJECTS: u32 = 50;

/// DozerTask residual enum ordinals (saved in game save — must not renumber).
pub const DOZER_TASK_BUILD: i32 = 0;
pub const DOZER_TASK_REPAIR: i32 = 1;
pub const DOZER_TASK_FORTIFY: i32 = 2;
pub const DOZER_NUM_TASKS: i32 = 3;

/// DozerBuildSubTask residual ordinals.
pub const DOZER_SELECT_BUILD_DOCK_LOCATION: i32 = 0;
pub const DOZER_MOVING_TO_BUILD_DOCK_LOCATION: i32 = 1;
pub const DOZER_DO_BUILD_AT_DOCK: i32 = 2;

/// Construction progress residual: base_rate * dozer_count * power_factor * BuildSpeed.
///
/// `build_time_sec` is ThingTemplate BuildTime; progress is 0..1 fraction complete.
pub fn dozer_construction_progress_delta(
    build_time_sec: f32,
    dozer_count: u32,
    power_factor: f32,
    dt_sec: f32,
) -> f32 {
    if build_time_sec <= 0.0 || dozer_count == 0 {
        return 0.0;
    }
    let base_rate = 1.0 / build_time_sec;
    base_rate * (dozer_count as f32) * power_factor.max(0.01) * BUILD_SPEED * dt_sec
}

/// Whether a map-edge residual build placement is legal.
pub fn is_legal_build_distance_from_map_edge(dist_from_edge: f32) -> bool {
    dist_from_edge >= MIN_DIST_FROM_EDGE_OF_MAP_FOR_BUILD
}

/// Whether residual supply-center placement respects SupplyBuildBorder.
pub fn is_legal_supply_center_distance(dist_from_supply_source: f32) -> bool {
    dist_from_supply_source >= SUPPLY_BUILD_BORDER
}

/// Whether residual footprint height variation is buildable.
pub fn is_legal_build_height_variation(height_delta: f32) -> bool {
    height_delta.abs() <= ALLOWED_HEIGHT_VARIATION_FOR_BUILDING
}

/// True when template is a residual dozer (not GLA worker dual-role).
pub fn is_dozer_build_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("dozer") && !n.contains("worker")
}

/// Wave 83 honesty: dozer build residual pack.
pub fn honesty_dozer_build_residual_pack_wave83() -> bool {
    DOZER_BUILD_COST == 1000
        && (DOZER_BUILD_TIME_SEC - 5.0).abs() < 0.01
        && DOZER_BUILD_TIME_FRAMES
            == structure_economy_ms_to_frames((DOZER_BUILD_TIME_SEC * 1000.0) as u32)
        && (DOZER_MAX_HEALTH - 250.0).abs() < 0.01
        && (DOZER_VISION_RANGE - 200.0).abs() < 0.01
        && DOZER_TRANSPORT_SLOT_COUNT == 5
        && DOZER_COMMAND_SET == "AmericaDozerCommandSet"
        && DOZER_LOCOMOTOR == "AmericaVehicleDozerLocomotor"
        && (DOZER_REPAIR_HEALTH_PERCENT_PER_SEC - 0.02).abs() < 0.0001
        && DOZER_BORED_TIME_MS == 5000
        && DOZER_BORED_TIME_FRAMES == structure_economy_ms_to_frames(DOZER_BORED_TIME_MS)
        && (DOZER_BORED_RANGE - 150.0).abs() < 0.01
        && (MIN_DIST_FROM_EDGE_OF_MAP_FOR_BUILD - 30.0).abs() < 0.01
        && (SUPPLY_BUILD_BORDER - 20.0).abs() < 0.01
        && (ALLOWED_HEIGHT_VARIATION_FOR_BUILDING - 10.0).abs() < 0.01
        && MAX_LINE_BUILD_OBJECTS == 50
        && DOZER_TASK_BUILD == 0
        && DOZER_TASK_REPAIR == 1
        && DOZER_TASK_FORTIFY == 2
        && DOZER_NUM_TASKS == 3
        && DOZER_SELECT_BUILD_DOCK_LOCATION == 0
        && DOZER_MOVING_TO_BUILD_DOCK_LOCATION == 1
        && DOZER_DO_BUILD_AT_DOCK == 2
        // 5s build, 1 dozer, full power, 1s dt → 0.2 progress.
        && (dozer_construction_progress_delta(5.0, 1, 1.0, 1.0) - 0.2).abs() < 0.001
        // Two dozers double residual rate.
        && (dozer_construction_progress_delta(5.0, 2, 1.0, 1.0) - 0.4).abs() < 0.001
        // Low-energy residual half-rate.
        && (dozer_construction_progress_delta(5.0, 1, 0.5, 1.0) - 0.1).abs() < 0.001
        && dozer_construction_progress_delta(5.0, 0, 1.0, 1.0) == 0.0
        && is_legal_build_distance_from_map_edge(30.0)
        && !is_legal_build_distance_from_map_edge(29.9)
        && is_legal_supply_center_distance(20.0)
        && !is_legal_supply_center_distance(19.0)
        && is_legal_build_height_variation(10.0)
        && !is_legal_build_height_variation(10.1)
        && is_dozer_build_template("USA_Dozer")
        && is_dozer_build_template("AmericaDozer")
        && !is_dozer_build_template("GLAWorker")
}

// ---------------------------------------------------------------------------
// 4. Capture building residual (Ranger / InfantryCapture pack)
// ---------------------------------------------------------------------------

/// Retail SpecialPower template residual name (Ranger).
pub const CAPTURE_SPECIAL_POWER_RANGER: &str = "SpecialAbilityRangerCaptureBuilding";
/// Retail SpecialPower enum residual.
pub const CAPTURE_SPECIAL_POWER_ENUM: &str = "SPECIAL_INFANTRY_CAPTURE_BUILDING";
/// Retail upgrade gate residual.
pub const UPGRADE_INFANTRY_CAPTURE_BUILDING: &str = "Upgrade_InfantryCaptureBuilding";
/// Retail CommandButton residual name.
pub const CAPTURE_COMMAND_BUTTON: &str = "Command_AmericaRangerCaptureBuilding";
/// Retail CursorName residual.
pub const CAPTURE_CURSOR_NAME: &str = "CaptureBuilding";
/// Retail TextLabel residual.
pub const CAPTURE_TEXT_LABEL: &str = "CONTROLBAR:CaptureBuilding";
/// Retail ButtonImage residual.
pub const CAPTURE_BUTTON_IMAGE: &str = "SSCaptureBuilding";

/// Retail SpecialPower ReloadTime residual (msec).
pub const CAPTURE_RELOAD_MS: u32 = 15_000;
/// ReloadTime 15000ms → 450 frames.
pub const CAPTURE_RELOAD_FRAMES: u32 = 450;
/// Retail SpecialAbilityUpdate StartAbilityRange residual.
pub const CAPTURE_START_ABILITY_RANGE: f32 = 5.0;
/// Retail UnpackTime residual (msec).
pub const CAPTURE_UNPACK_MS: u32 = 3_000;
/// Unpack 3000ms → 90 frames.
pub const CAPTURE_UNPACK_FRAMES: u32 = 90;
/// Retail PreparationTime residual (msec).
pub const CAPTURE_PREP_MS: u32 = 20_000;
/// Prep 20000ms → 600 frames.
pub const CAPTURE_PREP_FRAMES: u32 = 600;
/// Retail PackTime residual (msec).
pub const CAPTURE_PACK_MS: u32 = 2_000;
/// Pack 2000ms → 60 frames.
pub const CAPTURE_PACK_FRAMES: u32 = 60;
/// Retail AwardXPForTriggering residual (Ranger).
pub const CAPTURE_AWARD_XP: u32 = 15;
/// Retail DoCaptureFX residual.
pub const CAPTURE_DO_FX: bool = true;
/// Retail InitiateSound residual.
pub const CAPTURE_INITIATE_SOUND: &str = "RangerVoiceCapture";
/// Total capture channel residual frames (unpack + prep + pack).
pub const CAPTURE_TOTAL_CHANNEL_FRAMES: u32 =
    CAPTURE_UNPACK_FRAMES + CAPTURE_PREP_FRAMES + CAPTURE_PACK_FRAMES;

/// Capture residual legality matrix (host-testable subset).
pub fn is_legal_capture_building_target(
    is_structure: bool,
    is_alive: bool,
    is_enemy_or_neutral: bool,
    is_capturable: bool,
    under_construction: bool,
    has_capture_upgrade: bool,
    hero_bypass_upgrade: bool,
) -> bool {
    if !is_structure || !is_alive || !is_enemy_or_neutral || !is_capturable || under_construction {
        return false;
    }
    has_capture_upgrade || hero_bypass_upgrade
}

/// Capture residual progress fraction after elapsed frames in prep phase.
pub fn capture_prep_progress(elapsed_prep_frames: u32) -> f32 {
    if CAPTURE_PREP_FRAMES == 0 {
        return 1.0;
    }
    (elapsed_prep_frames as f32 / CAPTURE_PREP_FRAMES as f32).clamp(0.0, 1.0)
}

/// Wave 83 honesty: capture building residual pack.
pub fn honesty_capture_building_residual_pack_wave83() -> bool {
    CAPTURE_SPECIAL_POWER_RANGER == "SpecialAbilityRangerCaptureBuilding"
        && CAPTURE_SPECIAL_POWER_ENUM == "SPECIAL_INFANTRY_CAPTURE_BUILDING"
        && UPGRADE_INFANTRY_CAPTURE_BUILDING == "Upgrade_InfantryCaptureBuilding"
        && CAPTURE_COMMAND_BUTTON == "Command_AmericaRangerCaptureBuilding"
        && CAPTURE_CURSOR_NAME == "CaptureBuilding"
        && CAPTURE_TEXT_LABEL == "CONTROLBAR:CaptureBuilding"
        && CAPTURE_BUTTON_IMAGE == "SSCaptureBuilding"
        && CAPTURE_RELOAD_MS == 15_000
        && CAPTURE_RELOAD_FRAMES == structure_economy_ms_to_frames(CAPTURE_RELOAD_MS)
        && (CAPTURE_START_ABILITY_RANGE - 5.0).abs() < 0.01
        && CAPTURE_UNPACK_FRAMES == structure_economy_ms_to_frames(CAPTURE_UNPACK_MS)
        && CAPTURE_PREP_FRAMES == structure_economy_ms_to_frames(CAPTURE_PREP_MS)
        && CAPTURE_PACK_FRAMES == structure_economy_ms_to_frames(CAPTURE_PACK_MS)
        && CAPTURE_AWARD_XP == 15
        && CAPTURE_DO_FX
        && CAPTURE_INITIATE_SOUND == "RangerVoiceCapture"
        && CAPTURE_TOTAL_CHANNEL_FRAMES == 90 + 600 + 60
        && (capture_prep_progress(0) - 0.0).abs() < 0.001
        && (capture_prep_progress(300) - 0.5).abs() < 0.001
        && (capture_prep_progress(600) - 1.0).abs() < 0.001
        && is_legal_capture_building_target(true, true, true, true, false, true, false)
        && is_legal_capture_building_target(true, true, true, true, false, false, true)
        && !is_legal_capture_building_target(true, true, true, true, false, false, false)
        && !is_legal_capture_building_target(true, true, true, true, true, true, false)
        && !is_legal_capture_building_target(false, true, true, true, false, true, false)
}

// ---------------------------------------------------------------------------
// 5. Power plant residual energy residual
// ---------------------------------------------------------------------------

/// Retail AmericaPowerPlant template residual name (Cold Fusion).
pub const AMERICA_POWER_PLANT_TEMPLATE: &str = "AmericaPowerPlant";
/// Retail ChinaPowerPlant template residual name (Nuclear Reactor).
pub const CHINA_POWER_PLANT_TEMPLATE: &str = "ChinaPowerPlant";
/// Retail Advanced Control Rods upgrade residual name.
pub const UPGRADE_AMERICA_ADVANCED_CONTROL_RODS: &str = "Upgrade_AmericaAdvancedControlRods";

/// America Cold Fusion EnergyProduction residual.
pub const AMERICA_POWER_ENERGY_PRODUCTION: i32 = 5;
/// America Cold Fusion EnergyBonus residual (control rods).
pub const AMERICA_POWER_ENERGY_BONUS: i32 = 5;
/// America BuildCost residual.
pub const AMERICA_POWER_BUILD_COST: u32 = 800;
/// America BuildTime residual (seconds).
pub const AMERICA_POWER_BUILD_TIME_SEC: f32 = 10.0;
/// America MaxHealth residual.
pub const AMERICA_POWER_MAX_HEALTH: f32 = 800.0;
/// America PowerPlantUpdate RodsExtendTime residual (msec → parseDuration).
pub const AMERICA_POWER_RODS_EXTEND_MS: u32 = 600;
/// RodsExtendTime 600ms → 18 frames.
pub const AMERICA_POWER_RODS_EXTEND_FRAMES: u32 = 18;

/// China Nuclear Reactor EnergyProduction residual.
pub const CHINA_POWER_ENERGY_PRODUCTION: i32 = 10;
/// China EnergyBonus residual (overcharge bonus field).
pub const CHINA_POWER_ENERGY_BONUS: i32 = 5;
/// China BuildCost residual.
pub const CHINA_POWER_BUILD_COST: u32 = 1000;
/// China BuildTime residual (seconds).
pub const CHINA_POWER_BUILD_TIME_SEC: f32 = 10.0;
/// China MaxHealth residual.
pub const CHINA_POWER_MAX_HEALTH: f32 = 1500.0;
/// China RodsExtendTime residual (msec).
pub const CHINA_POWER_RODS_EXTEND_MS: u32 = 1;
/// China RodsExtendTime 1ms → 1 frame (ceil).
pub const CHINA_POWER_RODS_EXTEND_FRAMES: u32 = 1;
/// China OverchargeBehavior HealthPercentToDrainPerSecond residual (= 3%).
pub const CHINA_OVERCHARGE_DRAIN_PERCENT_PER_SEC: f32 = 0.03;

/// Effective residual energy production (base + bonus when upgraded/overcharged).
pub fn power_plant_effective_energy(base: i32, bonus: i32, upgraded: bool) -> i32 {
    if upgraded {
        base.saturating_add(bonus)
    } else {
        base
    }
}

/// True when template is a residual power plant.
pub fn is_power_plant_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("powerplant")
        || n.contains("power_plant")
        || n.contains("coldfusion")
        || n.contains("nuclearreactor")
}

/// Wave 83 honesty: power plant residual energy pack.
pub fn honesty_power_plant_residual_pack_wave83() -> bool {
    AMERICA_POWER_PLANT_TEMPLATE == "AmericaPowerPlant"
        && CHINA_POWER_PLANT_TEMPLATE == "ChinaPowerPlant"
        && UPGRADE_AMERICA_ADVANCED_CONTROL_RODS == "Upgrade_AmericaAdvancedControlRods"
        && AMERICA_POWER_ENERGY_PRODUCTION == 5
        && AMERICA_POWER_ENERGY_BONUS == 5
        && AMERICA_POWER_BUILD_COST == 800
        && (AMERICA_POWER_BUILD_TIME_SEC - 10.0).abs() < 0.01
        && (AMERICA_POWER_MAX_HEALTH - 800.0).abs() < 0.01
        && AMERICA_POWER_RODS_EXTEND_FRAMES
            == structure_economy_ms_to_frames(AMERICA_POWER_RODS_EXTEND_MS)
        && CHINA_POWER_ENERGY_PRODUCTION == 10
        && CHINA_POWER_ENERGY_BONUS == 5
        && CHINA_POWER_BUILD_COST == 1000
        && (CHINA_POWER_BUILD_TIME_SEC - 10.0).abs() < 0.01
        && (CHINA_POWER_MAX_HEALTH - 1500.0).abs() < 0.01
        && CHINA_POWER_RODS_EXTEND_FRAMES
            == structure_economy_ms_to_frames(CHINA_POWER_RODS_EXTEND_MS)
        && (CHINA_OVERCHARGE_DRAIN_PERCENT_PER_SEC - 0.03).abs() < 0.0001
        && power_plant_effective_energy(5, 5, false) == 5
        && power_plant_effective_energy(5, 5, true) == 10
        && power_plant_effective_energy(10, 5, true) == 15
        && is_power_plant_template("AmericaPowerPlant")
        && is_power_plant_template("ChinaPowerPlant")
        && is_power_plant_template("USA_PowerPlant")
        && !is_power_plant_template("AmericaCommandCenter")
}

// ---------------------------------------------------------------------------
// 6. Command center residual peels
// ---------------------------------------------------------------------------

/// Retail America/China/GLA CommandCenter BuildCost residual.
pub const COMMAND_CENTER_BUILD_COST: u32 = 2000;
/// Retail CommandCenter BuildTime residual (seconds).
pub const COMMAND_CENTER_BUILD_TIME_SEC: f32 = 45.0;
/// Retail CommandCenter BuildTime residual frames.
pub const COMMAND_CENTER_BUILD_TIME_FRAMES: u32 = 1350;
/// Retail CommandCenter EnergyProduction residual (free).
pub const COMMAND_CENTER_ENERGY_PRODUCTION: i32 = 0;
/// Retail CommandCenter MaxHealth residual.
pub const COMMAND_CENTER_MAX_HEALTH: f32 = 5000.0;
/// Retail CommandCenter VisionRange / ShroudClearingRange residual.
pub const COMMAND_CENTER_VISION_RANGE: f32 = 300.0;
/// Retail ExperienceValue residual (all ranks).
pub const COMMAND_CENTER_EXPERIENCE_VALUE: u32 = 200;

/// Retail template residual names.
pub const AMERICA_COMMAND_CENTER_TEMPLATE: &str = "AmericaCommandCenter";
pub const CHINA_COMMAND_CENTER_TEMPLATE: &str = "ChinaCommandCenter";
pub const GLA_COMMAND_CENTER_TEMPLATE: &str = "GLACommandCenter";
/// Retail USA CommandSet residual.
pub const AMERICA_COMMAND_CENTER_COMMAND_SET: &str = "AmericaCommandCenterCommandSet";
/// Retail radar grant upgrade residual (USA CC).
pub const UPGRADE_AMERICA_RADAR: &str = "Upgrade_AmericaRadar";

/// Retail GameData.ini CommandCenterHealRange residual.
pub const COMMAND_CENTER_HEAL_RANGE: f32 = 500.0;
/// Retail GameData.ini CommandCenterHealAmount residual (HP per logic frame).
pub const COMMAND_CENTER_HEAL_AMOUNT_PER_FRAME: f32 = 0.01;

/// KindOf residual token honesty for command centers.
pub const COMMAND_CENTER_KINDOF_TOKENS: &[&str] = &[
    "STRUCTURE",
    "SELECTABLE",
    "IMMOBILE",
    "COMMANDCENTER",
    "CAPTURABLE",
    "FS_FACTORY",
    "AUTO_RALLYPOINT",
    "MP_COUNT_FOR_VICTORY",
];

/// Residual heal amount over N frames (GameData heal amount × frames).
pub fn command_center_heal_over_frames(frames: u32) -> f32 {
    COMMAND_CENTER_HEAL_AMOUNT_PER_FRAME * frames as f32
}

/// Whether a unit is in residual CC heal range.
pub fn is_in_command_center_heal_range(dist: f32) -> bool {
    dist <= COMMAND_CENTER_HEAL_RANGE
}

/// True when template is a residual command center.
pub fn is_command_center_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("commandcenter")
        || n.contains("command_center")
        || (n.contains("command") && n.contains("center"))
        || n.ends_with("_command")
        || n == "gla_command"
}

/// Wave 83 honesty: command center residual pack.
pub fn honesty_command_center_residual_pack_wave83() -> bool {
    COMMAND_CENTER_BUILD_COST == 2000
        && (COMMAND_CENTER_BUILD_TIME_SEC - 45.0).abs() < 0.01
        && COMMAND_CENTER_BUILD_TIME_FRAMES
            == structure_economy_ms_to_frames((COMMAND_CENTER_BUILD_TIME_SEC * 1000.0) as u32)
        && COMMAND_CENTER_ENERGY_PRODUCTION == 0
        && (COMMAND_CENTER_MAX_HEALTH - 5000.0).abs() < 0.01
        && (COMMAND_CENTER_VISION_RANGE - 300.0).abs() < 0.01
        && COMMAND_CENTER_EXPERIENCE_VALUE == 200
        && AMERICA_COMMAND_CENTER_TEMPLATE == "AmericaCommandCenter"
        && CHINA_COMMAND_CENTER_TEMPLATE == "ChinaCommandCenter"
        && GLA_COMMAND_CENTER_TEMPLATE == "GLACommandCenter"
        && AMERICA_COMMAND_CENTER_COMMAND_SET == "AmericaCommandCenterCommandSet"
        && UPGRADE_AMERICA_RADAR == "Upgrade_AmericaRadar"
        && (COMMAND_CENTER_HEAL_RANGE - 500.0).abs() < 0.01
        && (COMMAND_CENTER_HEAL_AMOUNT_PER_FRAME - 0.01).abs() < 0.0001
        && (command_center_heal_over_frames(100) - 1.0).abs() < 0.001
        && (command_center_heal_over_frames(30) - 0.3).abs() < 0.001
        && is_in_command_center_heal_range(500.0)
        && !is_in_command_center_heal_range(500.1)
        && COMMAND_CENTER_KINDOF_TOKENS.contains(&"COMMANDCENTER")
        && COMMAND_CENTER_KINDOF_TOKENS.contains(&"FS_FACTORY")
        && COMMAND_CENTER_KINDOF_TOKENS.contains(&"AUTO_RALLYPOINT")
        && COMMAND_CENTER_KINDOF_TOKENS.len() == 8
        && is_command_center_template("AmericaCommandCenter")
        && is_command_center_template("USA_CommandCenter")
        && is_command_center_template("GLA_Command")
        && !is_command_center_template("AmericaPowerPlant")
        // Production door residual already covered in production pack; re-check USA door.
        && USA_CC_DOOR_OPENING_MS == 1500
        && CHINA_CC_DOOR_OPENING_MS == 3000
}

/// Combined Wave 83 residual honesty pack (all six peels).
pub fn honesty_structure_economy_residual_pack_wave83() -> bool {
    honesty_production_queue_residual_pack_wave83()
        && honesty_supply_warehouse_residual_pack_wave83()
        && honesty_dozer_build_residual_pack_wave83()
        && honesty_capture_building_residual_pack_wave83()
        && honesty_power_plant_residual_pack_wave83()
        && honesty_command_center_residual_pack_wave83()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_queue_residual_pack_wave83_honesty() {
        assert!(honesty_production_queue_residual_pack_wave83());
        assert_eq!(production_cancel_refund(2000), 1000);
        assert!((production_power_factor_from_energy_ratio(0.9) - 0.8).abs() < 0.001);
    }

    #[test]
    fn supply_warehouse_residual_pack_wave83_honesty() {
        assert!(honesty_supply_warehouse_residual_pack_wave83());
        assert_eq!(supply_warehouse_cash_value(1), 75);
    }

    #[test]
    fn dozer_build_residual_pack_wave83_honesty() {
        assert!(honesty_dozer_build_residual_pack_wave83());
        assert!((dozer_construction_progress_delta(10.0, 1, 1.0, 1.0) - 0.1).abs() < 0.001);
    }

    #[test]
    fn capture_building_residual_pack_wave83_honesty() {
        assert!(honesty_capture_building_residual_pack_wave83());
        assert_eq!(CAPTURE_TOTAL_CHANNEL_FRAMES, 750);
    }

    #[test]
    fn power_plant_residual_pack_wave83_honesty() {
        assert!(honesty_power_plant_residual_pack_wave83());
        assert_eq!(power_plant_effective_energy(5, 5, true), 10);
    }

    #[test]
    fn command_center_residual_pack_wave83_honesty() {
        assert!(honesty_command_center_residual_pack_wave83());
        assert!((command_center_heal_over_frames(1000) - 10.0).abs() < 0.001);
    }

    #[test]
    fn structure_economy_residual_pack_wave83_combined() {
        assert!(honesty_structure_economy_residual_pack_wave83());
    }
}
