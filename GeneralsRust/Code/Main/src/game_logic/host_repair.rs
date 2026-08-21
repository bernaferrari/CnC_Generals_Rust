//! Host structure / vehicle repair residual.
//!
//! Residual slice (playability):
//! - Dozer / Worker `CommandType::Repair` → `AIState::Repairing` → approach structure
//!   → heal HP over time (C++ DozerAIUpdate DOZER_TASK_REPAIR residual).
//! - Damaged vehicles `CommandType::GetRepaired` → `AIState::SeekingRepair` → approach
//!   RepairPad **or WarFactory** (China RepairDockUpdate residual) → self-heal over time.
//! - Aircraft use Airfield for GetRepaired residual.
//!
//! Wave 52 residual pack (retail AmericaVehicleDozer / RepairDockUpdate INI):
//! - DozerAIUpdate RepairHealthPercentPerSecond = **2%** of max health / sec
//! - RepairDockUpdate TimeForFullHeal = **5000** ms (WarFactory / TechRepairPad)
//! - NumberApproachPositions = **5** residual (dock approach bones)
//! - Host flat HP/sec fallback retained for path that has no max-health context
//!
//! Fail-closed honesty:
//! - Not full C++ sole-benefactor multi-dozer reject-on-reject path
//! - RepairDock drone heal + DockWaiting bones are on the live host path
//! - Not full bridge scaffolding path
//! - Not network repair replication (network deferred)

use crate::game_logic::buildings::BuildingType;
use glam::Vec3;

/// Logic frames per second residual.
pub const REPAIR_LOGIC_FPS: f32 = 30.0;

/// Host residual flat HP/sec fallback for dozer structure repair and pad vehicle repair
/// when max-health context is unavailable.
///
/// Prefer [`dozer_repair_hp_per_sec`] / [`repair_dock_hp_per_sec`] for retail percent /
/// TimeForFullHeal residual math.
pub const HOST_REPAIR_RATE_HP_PER_SEC: f32 = 35.0;

/// Host residual HP/sec for infantry heal-pad residual (paired with repair pad path).
pub const HOST_HEAL_RATE_HP_PER_SEC: f32 = 25.0;

/// Interact range residual for **pad / vehicle** repair (world units).
/// Dozer *structure* repair uses [`repair_action_range`] (C++ MIN_ACTION_TOLERANCE).
pub const HOST_REPAIR_INTERACT_RANGE: f32 = 14.0;

/// C++ `MIN_ACTION_TOLERANCE` (DozerAIUpdate.cpp:321-322).
pub const DOZER_MIN_ACTION_TOLERANCE: f32 = 70.0;
/// C++ dock arrival slop added to the dozer bounding sphere.
pub const DOZER_ACTION_SLOP: f32 = 15.0;
/// C++ `PATHFIND_CELL_SIZE_F`.
pub const PATHFIND_CELL_SIZE_F: f32 = 10.0;
/// C++ `DozerAIUpdate::newTask` END dock is 5 pathfind cells past the action dock.
pub const DOZER_END_DOCK_CELLS: f32 = 5.0;
/// C++ `TAiData::m_aiDozerBoredRadiusModifier` default (AI.cpp:920).
pub const AI_DOZER_BORED_RADIUS_MODIFIER: f32 = 2.0;

/// C++ `DozerAIUpdate::findGoodBuildOrRepairPosition` seed for a support
/// order.  A dozer does not path to a structure's centre: it starts from the
/// closest side of the target, biased by half the target's major radius, then
/// asks the pathfinder for a viable nearby point.  The host has no authored
/// dock-bone matrix yet, so `selection_radius` is its geometry proxy.
///
/// This is intentionally a *goal seed*, not an authorization range. The
/// compact host support state has a centre-distance interaction limit, so the
/// seed may not exceed that limit: otherwise a completed route could never
/// start the support action. Callers must still use
/// [`HOST_REPAIR_INTERACT_RANGE`] for the actual support tick.
#[inline]
pub fn support_approach_position(
    source_position: Vec3,
    target_position: Vec3,
    target_selection_radius: f32,
) -> Vec3 {
    let offset = source_position - target_position;
    let direction = offset.normalize_or_zero();
    if direction.length_squared() <= f32::EPSILON {
        return target_position;
    }
    let side_offset = (target_selection_radius.max(0.0) * 0.5).min(HOST_REPAIR_INTERACT_RANGE);
    target_position + direction * side_offset
}

/// C++ `findGoodBuildOrRepairPosition` dock seed: half major radius, **not**
/// clamped to the 14-unit pad interact range. Dozer repair/build uses this.
#[inline]
pub fn dozer_repair_approach_position(
    source_position: Vec3,
    target_position: Vec3,
    target_selection_radius: f32,
) -> Vec3 {
    let offset = source_position - target_position;
    let direction = offset.normalize_or_zero();
    if direction.length_squared() <= f32::EPSILON {
        return target_position;
    }
    let side_offset = target_selection_radius.max(0.0) * 0.5;
    target_position + direction * side_offset
}

/// C++ arrival window: `max(MIN_ACTION_TOLERANCE, boundingSphere + 15)`.
#[inline]
pub fn repair_action_range(target_selection_radius: f32) -> f32 {
    DOZER_MIN_ACTION_TOLERANCE.max(target_selection_radius.max(0.0) + DOZER_ACTION_SLOP)
}

/// C++ `DozerAIUpdate::getBoredRange`: computer dozers scan `modifier * BoredRange`.
#[inline]
pub fn dozer_bored_range(is_computer_dozer: bool) -> f32 {
    if is_computer_dozer {
        DOZER_BORED_RANGE * AI_DOZER_BORED_RADIUS_MODIFIER
    } else {
        DOZER_BORED_RANGE
    }
}

/// C++ `DozerAIUpdate::newTask` END dock: `dock + normalize(dock - target) * 5 * cell`.
#[inline]
pub fn dozer_end_dock_position(dock_position: Vec3, building_position: Vec3) -> Vec3 {
    let mut offset = dock_position - building_position;
    offset.y = 0.0;
    let direction = offset.normalize_or_zero();
    let push = DOZER_END_DOCK_CELLS * PATHFIND_CELL_SIZE_F;
    if direction.length_squared() <= f32::EPSILON {
        return dock_position + Vec3::new(push, 0.0, 0.0);
    }
    dock_position + direction * push
}

/// Retail DozerAIUpdate / WorkerAIUpdate RepairHealthPercentPerSecond residual (= 2%).
pub const DOZER_REPAIR_HEALTH_PERCENT_PER_SEC: f32 = 0.02;

/// Retail DozerAIUpdate BoredTime residual (msec).
pub const DOZER_BORED_TIME_MS: u32 = 5000;
/// BoredTime 5000ms → 150 frames.
pub const DOZER_BORED_TIME_FRAMES: u32 = 150;
/// Retail DozerAIUpdate BoredRange residual.
pub const DOZER_BORED_RANGE: f32 = 150.0;

/// Retail RepairDockUpdate TimeForFullHeal residual (msec) —
/// AmericaWarFactory / ChinaWarFactory / TechRepairPad.
pub const REPAIR_DOCK_TIME_FOR_FULL_HEAL_MS: u32 = 5000;
/// TimeForFullHeal 5000ms → 150 frames @ 30 FPS.
pub const REPAIR_DOCK_TIME_FOR_FULL_HEAL_FRAMES: u32 = 150;
/// Retail RepairDockUpdate NumberApproachPositions residual.
pub const REPAIR_DOCK_NUMBER_APPROACH_POSITIONS: u32 = 5;

/// Retail TechRepairPad template residual name.
pub const TECH_REPAIR_PAD_TEMPLATE: &str = "TechRepairPad";

/// Convert msec residual → logic frames @ 30 FPS (C++ parseDurationUnsignedInt ceil).
pub fn repair_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * (REPAIR_LOGIC_FPS / 1000.0)).ceil() as u32
}

/// Dozer structure-repair residual HP/sec from RepairHealthPercentPerSecond.
///
/// Retail: 2% of max health per second.
pub fn dozer_repair_hp_per_sec(max_health: f32) -> f32 {
    if max_health <= 0.0 {
        return 0.0;
    }
    max_health * DOZER_REPAIR_HEALTH_PERCENT_PER_SEC
}

/// Repair-dock residual HP/sec from TimeForFullHeal (full health restored in N ms).
///
/// Retail TimeForFullHeal = 5000 ms → 100% max health / 5 sec → 20% max / sec.
pub fn repair_dock_hp_per_sec(max_health: f32) -> f32 {
    if max_health <= 0.0 {
        return 0.0;
    }
    let seconds = (REPAIR_DOCK_TIME_FOR_FULL_HEAL_MS as f32) / 1000.0;
    if seconds <= 0.0 {
        return 0.0;
    }
    max_health / seconds
}

/// C++ `RepairDockUpdate::action` first-docker rate:
/// `(maxHealth - health) / TimeForFullHeal` seconds.
///
/// Retail 5000 ms → a Humvee and an Overlord both finish in 5 s from their
/// arrival damage, instead of sharing a flat 35 HP/s.
pub fn repair_dock_hp_per_sec_from_missing(max_health: f32, current_health: f32) -> f32 {
    if max_health <= 0.0 {
        return 0.0;
    }
    let seconds = (REPAIR_DOCK_TIME_FOR_FULL_HEAL_MS as f32) / 1000.0;
    if seconds <= 0.0 {
        return 0.0;
    }
    (max_health - current_health).max(0.0) / seconds
}


/// Whether a building type can service vehicle GetRepaired residual.
///
/// Retail: USA Repair Bay (RepairPad / TechRepairPad); China War Factory docks vehicles
/// (`RepairDockUpdate` on WarFactory). Fail-closed: not per-template module matrix.
pub fn building_provides_vehicle_repair(building_type: BuildingType) -> bool {
    matches!(
        building_type,
        BuildingType::RepairPad | BuildingType::WarFactory
    )
}

/// Whether a building type can service aircraft GetRepaired residual.
pub fn building_provides_aircraft_repair(building_type: BuildingType) -> bool {
    building_type == BuildingType::Airfield
}

/// Whether target is a legal structure-repair destination residual.
pub fn is_legal_structure_repair_target(
    is_structure: bool,
    is_alive: bool,
    is_damaged: bool,
    under_construction: bool,
    same_or_neutral_team: bool,
) -> bool {
    is_structure && is_alive && is_damaged && !under_construction && same_or_neutral_team
}

/// Wave 52 residual honesty: dozer percent rate + pad TimeForFullHeal residual.
pub fn honesty_repair_residual_ok() -> bool {
    (DOZER_REPAIR_HEALTH_PERCENT_PER_SEC - 0.02).abs() < 0.0001
        && REPAIR_DOCK_TIME_FOR_FULL_HEAL_MS == 5000
        && REPAIR_DOCK_TIME_FOR_FULL_HEAL_FRAMES
            == repair_ms_to_frames(REPAIR_DOCK_TIME_FOR_FULL_HEAL_MS)
        && REPAIR_DOCK_NUMBER_APPROACH_POSITIONS == 5
        && DOZER_BORED_TIME_MS == 5000
        && DOZER_BORED_TIME_FRAMES == repair_ms_to_frames(DOZER_BORED_TIME_MS)
        && (DOZER_BORED_RANGE - 150.0).abs() < 0.01
        && (AI_DOZER_BORED_RADIUS_MODIFIER - 2.0).abs() < 0.01
        && (dozer_bored_range(true) - 300.0).abs() < 0.01
        && (dozer_bored_range(false) - 150.0).abs() < 0.01
        && (DOZER_MIN_ACTION_TOLERANCE - 70.0).abs() < 0.01
        && HOST_REPAIR_RATE_HP_PER_SEC > 0.0
        && HOST_REPAIR_INTERACT_RANGE > 0.0
        && TECH_REPAIR_PAD_TEMPLATE == "TechRepairPad"
        && (dozer_repair_hp_per_sec(1000.0) - 20.0).abs() < 0.01
        && (repair_dock_hp_per_sec(1000.0) - 200.0).abs() < 0.01
        && (dozer_repair_hp_per_sec(200.0) - 4.0).abs() < 0.01
}
/// Combined residual honesty pack (Wave 71).
pub fn honesty_repair_residual_pack_ok() -> bool {
    honesty_repair_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vehicle_repair_destinations_include_war_factory() {
        assert!(building_provides_vehicle_repair(BuildingType::RepairPad));
        assert!(building_provides_vehicle_repair(BuildingType::WarFactory));
        assert!(!building_provides_vehicle_repair(BuildingType::Barracks));
        assert!(!building_provides_vehicle_repair(
            BuildingType::CommandCenter
        ));
        assert!(building_provides_aircraft_repair(BuildingType::Airfield));
        assert!(!building_provides_aircraft_repair(BuildingType::WarFactory));
    }

    #[test]
    fn support_approach_is_biased_toward_the_source_side_of_the_target() {
        let target = Vec3::ZERO;
        let source = Vec3::new(60.0, 0.0, 0.0);
        assert_eq!(
            support_approach_position(source, target, 24.0),
            Vec3::new(12.0, 0.0, 0.0)
        );
        assert_eq!(
            support_approach_position(target, target, 24.0),
            target,
            "zero-length direction must not manufacture a side"
        );
        assert_eq!(
            support_approach_position(source, target, 60.0),
            Vec3::new(HOST_REPAIR_INTERACT_RANGE, 0.0, 0.0),
            "the approach seed must remain within the live support range"
        );
    }

    #[test]
    fn dozer_repair_docks_at_half_radius_not_fourteen() {
        // C++ DozerAIUpdate.cpp:1855-1866 / 316-322.
        let target = Vec3::ZERO;
        let source = Vec3::new(200.0, 0.0, 0.0);
        assert_eq!(
            dozer_repair_approach_position(source, target, 80.0),
            Vec3::new(40.0, 0.0, 0.0)
        );
        assert!((repair_action_range(80.0) - 95.0).abs() < 0.01);
        assert!((repair_action_range(10.0) - 70.0).abs() < 0.01);
        assert!((dozer_repair_hp_per_sec(200.0) - 4.0).abs() < 0.01);
    }

    #[test]
    fn dozer_end_dock_is_five_cells_away_from_building() {
        // C++ DozerAIUpdate.cpp:1992-1998.
        let building = Vec3::ZERO;
        let dock = Vec3::new(20.0, 0.0, 0.0);
        let end = dozer_end_dock_position(dock, building);
        assert!((end.x - 70.0).abs() < 0.01);
        assert!((end.z).abs() < 0.01);
    }

    #[test]
    fn computer_dozer_bored_range_is_double() {
        // C++ DozerAIUpdate.cpp:2305-2312; AI.cpp:920.
        assert!((dozer_bored_range(false) - 150.0).abs() < 0.01);
        assert!((dozer_bored_range(true) - 300.0).abs() < 0.01);
    }

    #[test]
    fn legal_structure_repair_target_matrix() {
        assert!(is_legal_structure_repair_target(
            true, true, true, false, true
        ));
        assert!(!is_legal_structure_repair_target(
            false, true, true, false, true
        ));
        assert!(!is_legal_structure_repair_target(
            true, false, true, false, true
        ));
        assert!(!is_legal_structure_repair_target(
            true, true, false, false, true
        ));
        assert!(!is_legal_structure_repair_target(
            true, true, true, true, true
        ));
        assert!(!is_legal_structure_repair_target(
            true, true, true, false, false
        ));
    }

    #[test]
    fn repair_residual_pack_honesty() {
        assert!(honesty_repair_residual_ok());
        // Dozer RepairHealthPercentPerSecond = 2%.
        assert!((DOZER_REPAIR_HEALTH_PERCENT_PER_SEC - 0.02).abs() < 0.0001);
        // 2% of 500 max → 10 HP/sec residual.
        assert!((dozer_repair_hp_per_sec(500.0) - 10.0).abs() < 0.01);
        assert_eq!(dozer_repair_hp_per_sec(0.0), 0.0);
        // Pad TimeForFullHeal = 5000ms → full heal in 5s → 20% max/sec.
        assert_eq!(REPAIR_DOCK_TIME_FOR_FULL_HEAL_MS, 5000);
        assert_eq!(REPAIR_DOCK_TIME_FOR_FULL_HEAL_FRAMES, 150);
        assert_eq!(repair_ms_to_frames(5000), 150);
        assert!((repair_dock_hp_per_sec(500.0) - 100.0).abs() < 0.01);
        assert_eq!(REPAIR_DOCK_NUMBER_APPROACH_POSITIONS, 5);
        assert_eq!(TECH_REPAIR_PAD_TEMPLATE, "TechRepairPad");
        assert_eq!(DOZER_BORED_TIME_MS, 5000);
        assert_eq!(DOZER_BORED_RANGE, 150.0);
    }

    #[test]
    fn repair_dock_time_for_full_heal_scales_with_missing_hp() {
        // Humvee 240 max / 40 current vs Overlord 2000 max / 400 current:
        // both finish in 5 s, so rates differ (not flat 35 HP/s).
        let humvee = repair_dock_hp_per_sec_from_missing(240.0, 40.0);
        let overlord = repair_dock_hp_per_sec_from_missing(2000.0, 400.0);
        assert!((humvee - 40.0).abs() < 0.01);
        assert!((overlord - 320.0).abs() < 0.01);
        assert!(humvee < overlord);
        assert_ne!(humvee, HOST_REPAIR_RATE_HP_PER_SEC);
        assert_ne!(overlord, HOST_REPAIR_RATE_HP_PER_SEC);
        assert_eq!(repair_dock_hp_per_sec_from_missing(100.0, 100.0), 0.0);
        assert_eq!(repair_dock_hp_per_sec_from_missing(0.0, 0.0), 0.0);
    }

    /// Wave 71 residual pack honesty gate.
    #[test]
    fn repair_residual_pack_honesty_wave71() {
        assert!(honesty_repair_residual_pack_ok());
        assert!((DOZER_REPAIR_HEALTH_PERCENT_PER_SEC - 0.02).abs() < 0.0001);
        assert_eq!(REPAIR_DOCK_TIME_FOR_FULL_HEAL_FRAMES, 150);
        assert_eq!(TECH_REPAIR_PAD_TEMPLATE, "TechRepairPad");
    }

    #[test]
    fn get_repaired_does_not_time_for_full_heal_airborne_aircraft() {
        use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
        crate::game_logic::host_supply_gather::reset_live_dock_queues();
        let mut logic = GameLogic::new();
        let mut air = ThingTemplate::new("TestAirfield");
        air.add_kind_of(KindOf::FSAirfield).set_health(2000.0);
        let mut heli = ThingTemplate::new("TestComanche");
        heli.add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Aircraft)
            .set_health(240.0);
        logic.templates.insert(air.name.clone(), air);
        logic.templates.insert(heli.name.clone(), heli);
        let af = logic
            .create_object("TestAirfield", Team::USA, glam::Vec3::ZERO)
            .unwrap();
        let id = logic
            .create_object("TestComanche", Team::USA, glam::Vec3::new(5.0, 20.0, 0.0))
            .unwrap();
        {
            let o = logic.host_object_mut(id).unwrap();
            o.health.current = 40.0;
            o.status.airborne_target = true;
            o.set_target(Some(af));
            o.set_ai_state(AIState::SeekingRepair);
        }
        logic.update_support_states_for_test(&[id], 1.0);
        let hp = logic.host_object(id).unwrap().health.current;
        assert!(
            (hp - 40.0).abs() < 0.01,
            "airborne aircraft must not TimeForFullHeal, hp={hp}"
        );
        crate::game_logic::host_supply_gather::reset_live_dock_queues();
    }

    #[test]
    fn repair_dock_heals_slave_drone_to_max() {
        use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
        crate::game_logic::host_supply_gather::reset_live_dock_queues();
        let mut logic = GameLogic::new();
        let mut pad = ThingTemplate::new("TestRepairPad");
        pad.add_kind_of(KindOf::RepairPad).set_health(2000.0);
        let mut humvee = ThingTemplate::new("TestHumvee");
        humvee.add_kind_of(KindOf::Vehicle).set_health(240.0);
        let mut drone = ThingTemplate::new("TestBattleDrone");
        drone
            .add_kind_of(KindOf::Drone)
            .add_kind_of(KindOf::Vehicle)
            .set_health(80.0);
        logic.templates.insert(pad.name.clone(), pad);
        logic.templates.insert(humvee.name.clone(), humvee);
        logic.templates.insert(drone.name.clone(), drone);
        let pad_id = logic
            .create_object("TestRepairPad", Team::USA, glam::Vec3::ZERO)
            .unwrap();
        let master = logic
            .create_object("TestHumvee", Team::USA, glam::Vec3::new(2.0, 0.0, 0.0))
            .unwrap();
        let drone_id = logic
            .create_object("TestBattleDrone", Team::USA, glam::Vec3::new(3.0, 0.0, 0.0))
            .unwrap();
        {
            let o = logic.host_object_mut(master).unwrap();
            o.health.current = 40.0;
            o.set_target(Some(pad_id));
            o.set_ai_state(AIState::SeekingRepair);
        }
        {
            let d = logic.host_object_mut(drone_id).unwrap();
            d.health.current = 10.0;
            d.producer_id = Some(master);
        }
        logic.update_support_states_for_test(&[master], 1.0 / 30.0);
        let drone_hp = logic.host_object(drone_id).unwrap().health.current;
        assert!(
            (drone_hp - 80.0).abs() < 0.01,
            "slave drone must snap to max while master docks, hp={drone_hp}"
        );
        crate::game_logic::host_supply_gather::reset_live_dock_queues();
    }

    #[test]
    fn worker_clears_moving_while_docking_at_supply_source() {
        use crate::game_logic::host_enum_table_residual::{
            docking_beginning_model_bit, moving_model_bit,
        };
        use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
        crate::game_logic::host_supply_gather::reset_live_dock_queues();
        let mut logic = GameLogic::new();
        let mut wh = ThingTemplate::new("TestWarehouse");
        wh.add_kind_of(KindOf::SupplySource)
            .add_kind_of(KindOf::Structure)
            .set_health(500.0);
        wh.dock_kind = crate::game_logic::DockKind::SupplyWarehouse;
        let mut worker = ThingTemplate::new("GLAInfantryWorker");
        worker
            .add_kind_of(KindOf::Dozer)
            .add_kind_of(KindOf::Harvester)
            .add_kind_of(KindOf::Infantry)
            .set_health(100.0);
        logic.templates.insert(wh.name.clone(), wh);
        logic.templates.insert(worker.name.clone(), worker);
        let dock = logic
            .create_object("TestWarehouse", Team::GLA, glam::Vec3::ZERO)
            .unwrap();
        let wid = logic
            .create_object("GLAInfantryWorker", Team::GLA, glam::Vec3::ZERO)
            .unwrap();
        {
            let o = logic.host_object_mut(wid).unwrap();
            o.model_condition_bits |= 1u128 << moving_model_bit();
            o.set_target(Some(dock));
            o.set_ai_state(AIState::Gathering);
        }
        assert!(logic.try_claim_dock_for_test(dock, wid));
        let o = logic.host_object(wid).unwrap();
        assert_ne!(o.model_condition_bits & (1u128 << docking_beginning_model_bit()), 0);
        assert_eq!(o.model_condition_bits & (1u128 << moving_model_bit()), 0);
        crate::game_logic::host_supply_gather::reset_live_dock_queues();
    }
}
