//! Shell-smoke tests split by theme from the former monolithic tests.rs.

pub use super::*;
pub use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
pub use crate::presentation_frame::PresentationFrame;
pub use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
pub use crate::ui::GameHUD;
pub use glam::Vec3;

mod presentation_path;
mod victory;
mod aiplayer;
mod render_stubs;
#[path = "host_smoke/mod.rs"]
mod host_smoke;
mod playable_claim;
mod transform_health;
mod dual_tick;
mod presentation_shell;
