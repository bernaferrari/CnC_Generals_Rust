//! Convenient re-export hub for downstream code.
//!
//! The legacy code base relied on massive `use crate::common::*` imports.
//! Those wildcards hid dependencies and routinely caused name collisions.
//! The new prelude keeps the ergonomics while remaining explicit about what
//! becomes part of the public surface.

// Re-export common types that update modules frequently need
pub use crate::common::*;
pub use crate::modules::*;

pub use crate::modules::PhysicsBehaviorExt;
pub use crate::runtime::{
    FrameResult, GameLogic as ExperimentalGameLogic, GameLogicConfig, SimulationStats,
};
pub use crate::system::game_logic::GameLogic;
pub use crate::world::{PlayerId, WorldSnapshot};
pub use game_engine::common::time::SimulationClock;
