//! Unit class - Moveable game entities
//!
//! Units are mobile objects that can move around the map, engage in combat,
//! and perform various actions. This includes infantry, vehicles, aircraft, etc.
//!
//! Port reference: GameLogic/Object/Unit.cpp, GameLogic/Object/Update/AIUpdate.cpp.

mod imports;

mod ai_commands;
mod ai_core;
mod ai_drop;
mod ai_helpers;
mod ai_interface;
mod ai_interface_update;
mod ai_loco;
mod ai_path;
mod ai_specialized;
mod combat;
mod identity;
mod movement;
mod orders;
mod registry;
mod types;

pub use ai_core::UnitAIUpdate;
pub use ai_path::{
    leftover_compute_quick_path_coords, leftover_is_in_region_no_z,
    leftover_should_force_direct_path_for_off_map_start,
    leftover_should_use_direct_path_for_line_passable_non_final_goal,
};
pub use identity::{Unit, UnitExt};
pub use registry::{UnitRegistry, register_unit, unregister_unit};
pub use types::{CombatMode, FormationType, MovementState, UnitOrder};

#[cfg(test)]
mod tests;
