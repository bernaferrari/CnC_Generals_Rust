//! Production module system for buildings
//!
//! This module provides the production and docking infrastructure for
//! buildings to produce units, manage build queues, handle unit entry/exit,
//! and coordinate with docking systems (repair, supply, etc.).

pub mod build_cost_calculator;
pub mod construction;
pub mod dock_update;
pub mod exit_strategies;
pub mod prerequisite_checker;
pub mod prison_dock;
pub mod production_update;
pub mod production_update_complete;
pub mod queue;
pub mod railed_transport_dock;
pub mod rally_point;
pub mod supply_warehouse_dock;
pub mod unit_exit;

#[cfg(test)]
mod tests;

/// Matches C++ AIFreeToExitType.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AIFreeToExitType {
    FreeToExit,
    WaitToExit,
}

// Re-export main types
pub use build_cost_calculator::{
    BuildCostCalculator, BuildFacilityContext, GlobalBuildModifiers, PlayerBuildModifiers,
};
pub use construction::{
    get_construction_manager, ConstructionInterruption, ConstructionManager, ConstructionProgress,
    ConstructionState, DozerConstructionTask, FoundationValidator,
};
pub use dock_update::{
    DockUpdate, DockUpdateData, RepairDockUpdate, RepairDockUpdateData, RepairDockUpdateModule,
    SupplyCenterDockUpdate, SupplyCenterDockUpdateData, SupplyCenterDockUpdateModule,
};
pub use exit_strategies::{
    DefaultProductionExit, ProductionExitStrategy, QueueProductionExit, SpawnPointProductionExit,
    SupplyCenterProductionExit,
};
pub use prerequisite_checker::{CanMakeType, PlayerBuildState, Prerequisite, PrerequisiteChecker};
pub use prison_dock::{PrisonDockUpdate, PrisonDockUpdateData, PrisonDockUpdateModule};
pub use production_update::{ProductionUpdate, ProductionUpdateData};
pub use production_update_complete::{
    ProductionUpdateComplete, ProductionUpdateModuleData, QuantityModifier,
};
pub use queue::{BuildPriority, BuildQueue, BuildQueueEntry, ProductionType};
pub use railed_transport_dock::{
    RailedTransportDockUpdate, RailedTransportDockUpdateData, RailedTransportDockUpdateModule,
};
pub use rally_point::{RallyPoint, RallyPointManager, RallyPointType};
pub use supply_warehouse_dock::{
    SupplyWarehouseDockUpdate, SupplyWarehouseDockUpdateData, SupplyWarehouseDockUpdateModule,
};
pub use unit_exit::{ExitDoor, ExitPath, StuckUnitHandler, UnitExitManager};
