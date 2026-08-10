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
pub use identity::{Unit, UnitExt};
pub use registry::{register_unit, unregister_unit, UnitRegistry};
pub use types::{CombatMode, FormationType, MovementState, UnitOrder};

#[cfg(test)]
mod tests;
