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
    PATRIOT_BINARY_DATA_STREAM, PATRIOT_LASER_INNER_COLOR, PATRIOT_LASER_TEXTURE,
    PatriotAssistLaserKind, ResidualPatriotAssistLaser, build_patriot_laser_line3d_segments,
};
use crate::game_logic::{
    CombatParticleKind, CombatParticleSystemEntry, DockKind, GameLogic, KindOf, ObjectId, Team,
};
use glam::Vec3;
use serde::{Deserialize, Serialize};

mod alive;
mod apply;
#[cfg(feature = "game_client")]
mod beacon_limit;
#[cfg(test)]
mod authored_model_owner_tests;
mod build;
pub(crate) use build::{LiveTerrainDecalSpec, live_terrain_decal_specs};
mod camera_terrain;
mod draw_conditions;
mod events;
mod floating_text;
mod follow_facts;
mod frame;
#[cfg(feature = "game_client")]
mod ghosts;
mod honesty;
mod kind_facts;
mod lasers;
mod live_draw_source;
pub(crate) use live_draw_source::{
    ObjectNativeDrawObservation, current_object_visual_template, object_native_draw_observation,
};
#[cfg(test)]
mod live_draw_source_tests;
#[cfg(test)]
mod money_audio_owner_tests;
mod native_factory_source;
mod overlay;
mod particles;
mod projectile;
mod queries;
mod restrict_a;
#[cfg(feature = "game_client")]
mod science_purchase;
#[cfg(feature = "game_client")]
mod shortcuts;
mod spectre;
mod terrain_motion;
mod types;
mod unit_render;
mod weapon_visual_dispatch;
mod world_env;
pub use terrain_motion::PresentationTerrainMotion;

#[cfg(test)]
mod tests;

pub use events::*;
pub use floating_text::*;
pub use frame::*;
pub use generals_presentation::{
    LogicFrame, PresentationHudFrame, PresentationPlayerInfo, PresentationPopupMessage,
    PresentationSuperweaponTimer,
};
pub use lasers::*;
pub use particles::*;
pub use projectile::*;
pub use restrict_a::PresentationRestrictA;
pub use spectre::*;
pub use types::*;
pub use unit_render::*;
pub use weapon_visual_dispatch::*;
pub use world_env::*;

/// Concatenated presentation_frame sources for residual `include_str` scans.
///
/// External crate tests previously read `presentation_frame.rs`. After the
/// directory split they should compare against this pack instead of a single file.
mod session_view;
pub(crate) use session_view::receiving_look_at_target;
pub(crate) use session_view::world_animations::WorldAnimations;
pub(crate) use session_view::{
    SessionView, SessionViewImpulse, SessionViewState, camera, follow, scripted, waypoint,
};

mod native_drawable_source;

mod camera_fade;
