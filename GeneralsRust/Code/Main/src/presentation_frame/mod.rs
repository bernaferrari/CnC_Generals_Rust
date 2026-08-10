//! Immutable presentation snapshot built from the authoritative Main GameLogic.
//!
//! Policy: GameClient / renderer / HUD should consume `PresentationFrame` only.
//! They must not lock or mutate the sim while a WGPU pass is active.
//!
//! Ownership: borrow-first on the authority during `build_*`; then the snapshot
//! is owned values with no live borrows into the world.
//!
//! Wave 956: host_object/host_objects when building presentation from host.
//! Wave 958: host_object dual-read seal (tests + residual).

use crate::fow_rendering::{FOWRenderingBridge, ObjectVisibility, PresentationFowGrid};
use crate::game_logic::host_base_defense::{
    build_patriot_laser_line3d_segments, PatriotAssistLaserKind, ResidualPatriotAssistLaser,
    PATRIOT_BINARY_DATA_STREAM, PATRIOT_LASER_INNER_COLOR, PATRIOT_LASER_TEXTURE,
};
use crate::game_logic::{
    CombatParticleKind, CombatParticleSystemEntry, GameLogic, KindOf, ObjectId, Team,
};
use glam::Vec3;
use serde::{Deserialize, Serialize};

mod types;
mod unit_render;
mod projectile;
mod events;
mod particles;
mod lasers;
mod floating_text;
mod spectre;
mod world_env;
mod frame;
mod build;
mod queries;
mod alive;
mod overlay;
mod honesty;
mod apply;

#[cfg(test)]
mod tests;

pub use types::*;
pub use unit_render::*;
pub use projectile::*;
pub use events::*;
pub use particles::*;
pub use lasers::*;
pub use floating_text::*;
pub use spectre::*;
pub use world_env::*;
pub use frame::*;

/// Concatenated presentation_frame sources for residual `include_str` scans.
///
/// External crate tests previously read `presentation_frame.rs`. After the
/// directory split they should compare against this pack instead of a single file.
#[cfg(test)]
pub const PRESENTATION_FRAME_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("types.rs"),
    include_str!("unit_render.rs"),
    include_str!("projectile.rs"),
    include_str!("events.rs"),
    include_str!("particles.rs"),
    include_str!("lasers.rs"),
    include_str!("floating_text.rs"),
    include_str!("spectre.rs"),
    include_str!("world_env.rs"),
    include_str!("frame.rs"),
    include_str!("build.rs"),
    include_str!("queries.rs"),
    include_str!("alive.rs"),
    include_str!("overlay.rs"),
    include_str!("honesty.rs"),
    include_str!("apply.rs"),
);
