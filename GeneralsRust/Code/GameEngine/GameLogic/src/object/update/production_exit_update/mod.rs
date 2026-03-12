//! Compatibility shims for ProductionExitUpdate modules.

pub mod default_production_exit_update;
pub mod queue_production_exit_update;
pub mod spawn_point_production_exit_update;
pub mod supply_center_production_exit_update;

pub use default_production_exit_update::*;
pub use queue_production_exit_update::*;
pub use spawn_point_production_exit_update::*;
pub use supply_center_production_exit_update::*;
