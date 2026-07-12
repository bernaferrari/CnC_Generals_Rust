//! Host structure / vehicle repair residual.
//!
//! Residual slice (playability):
//! - Dozer / Worker `CommandType::Repair` → `AIState::Repairing` → approach structure
//!   → heal HP over time (C++ DozerAIUpdate DOZER_TASK_REPAIR residual).
//! - Damaged vehicles `CommandType::GetRepaired` → `AIState::SeekingRepair` → approach
//!   RepairPad **or WarFactory** (China RepairDockUpdate residual) → self-heal over time.
//! - Aircraft use Airfield for GetRepaired residual.
//!
//! Fail-closed honesty:
//! - Not full C++ RepairHealthPercentPerSecond INI matrix / sole-benefactor healing
//! - Not full RepairDockUpdate TimeForFullHeal dock bones / drone heal
//! - Not full bridge scaffolding / multi-dozer reject-on-reject path
//! - Not network repair replication (network deferred)

use crate::game_logic::buildings::BuildingType;

/// Host residual flat HP/sec for dozer structure repair and pad vehicle repair.
/// Fail-closed: not C++ `RepairHealthPercentPerSecond` / `TimeForFullHeal` per-template matrix.
pub const HOST_REPAIR_RATE_HP_PER_SEC: f32 = 35.0;

/// Host residual HP/sec for infantry heal-pad residual (paired with repair pad path).
pub const HOST_HEAL_RATE_HP_PER_SEC: f32 = 25.0;

/// Interact range residual for dozer/pad repair (world units).
pub const HOST_REPAIR_INTERACT_RANGE: f32 = 14.0;

/// Whether a building type can service vehicle GetRepaired residual.
///
/// Retail: USA Repair Bay (RepairPad); China War Factory docks vehicles
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
}
