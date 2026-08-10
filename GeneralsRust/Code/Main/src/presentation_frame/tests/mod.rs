//! Honesty / residual tests for the presentation snapshot.
//!
//! Split from the former monolithic `presentation_frame.rs` `mod tests`.

pub use super::*;
pub use crate::game_logic::{GameLogic, GameMode, KindOf, ObjectId, Player, Team, ThingTemplate};
pub use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
pub use glam::Vec3;

mod render_overlay;
mod freeze_queries;
mod dual_tick_registry;
mod apply_honesty;
mod runtime_heightmap;
mod fow_own_team;
