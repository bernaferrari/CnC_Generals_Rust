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
//! - Not full RepairDockUpdate dock bones / drone heal matrix
//! - Not full bridge scaffolding path
//! - Not network repair replication (network deferred)

use crate::game_logic::buildings::BuildingType;

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

/// Interact range residual for dozer/pad repair (world units).
pub const HOST_REPAIR_INTERACT_RANGE: f32 = 14.0;

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

/// Whether residual unit can issue structure Repair (C++ KINDOF_DOZER / Worker).
/// Fail-closed: not full ActionManager canRepairObject edge matrix.
pub fn is_structure_repairer(is_worker: bool, can_move: bool, template_name: &str) -> bool {
    if !can_move {
        return false;
    }
    if is_worker {
        return true;
    }
    let n = template_name.to_ascii_lowercase();
    n.contains("dozer") || n.contains("worker")
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
        && HOST_REPAIR_RATE_HP_PER_SEC > 0.0
        && HOST_REPAIR_INTERACT_RANGE > 0.0
        && TECH_REPAIR_PAD_TEMPLATE == "TechRepairPad"
        && (dozer_repair_hp_per_sec(1000.0) - 20.0).abs() < 0.01
        && (repair_dock_hp_per_sec(1000.0) - 200.0).abs() < 0.01
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
        assert!(!building_provides_vehicle_repair(BuildingType::CommandCenter));
        assert!(building_provides_aircraft_repair(BuildingType::Airfield));
        assert!(!building_provides_aircraft_repair(BuildingType::WarFactory));
    }

    #[test]
    fn structure_repairer_helpers() {
        assert!(is_structure_repairer(true, true, "TestInfantry"));
        assert!(is_structure_repairer(false, true, "USA_Dozer"));
        assert!(is_structure_repairer(false, true, "GLA_Worker"));
        assert!(!is_structure_repairer(false, true, "USA_Ranger"));
        assert!(!is_structure_repairer(true, false, "TestDozer"));
        assert!(HOST_REPAIR_RATE_HP_PER_SEC > 0.0);
        assert!(HOST_REPAIR_INTERACT_RANGE > 0.0);
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
    /// Wave 71 residual pack honesty gate.
    #[test]
    fn repair_residual_pack_honesty_wave71() {
        assert!(honesty_repair_residual_pack_ok());
        assert!((DOZER_REPAIR_HEALTH_PERCENT_PER_SEC - 0.02).abs() < 0.0001);
        assert_eq!(REPAIR_DOCK_TIME_FOR_FULL_HEAL_FRAMES, 150);
        assert_eq!(TECH_REPAIR_PAD_TEMPLATE, "TechRepairPad");
    }

}
