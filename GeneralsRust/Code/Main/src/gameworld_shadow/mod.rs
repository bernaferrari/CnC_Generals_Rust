//! Immutable Main simulation projection for presentation and diagnostics.
//! Main owns objects, AI, timers and effects. The observer copies completed
//! values and never advances gameplay or writes them back into the session.

use crate::game_logic::{GameLogic, ObjectId, Team};

use gamelogic::world::entities::{EntityId, EntityProductionItem, TemplateRef, Transform};
use gamelogic::world::{GameWorld, PlayerId, WorldMutation, WorldSnapshot};
use std::collections::{HashMap, HashSet};

mod tick;
mod types;
pub use tick::*;
use types::HordePlayerRel;
pub use types::*;

mod construct;
mod construction_observation;
mod counts;
mod host_observation_boundary;
mod observation_metrics;
mod presentation;
mod projectile_observation;
mod session;
mod weapon_observation;
pub(crate) use host_observation_boundary::run_owned_host_boundary;
pub use presentation::*;
pub use session::*;

/// Session holding GameWorld + stable host↔entity ID maps.
#[derive(Debug)]
pub struct GameWorldShadow {
    /// Tint owner when a presentation has no authoritative Main world.
    pub(crate) drawable_tint_envelopes: crate::game_logic::DrawableTintEnvelopes,
    world: GameWorld,
    host_to_entity: HashMap<u32, EntityId>,
    entity_to_host: HashMap<u32, u32>,
    max_entities: usize,
    /// Host player id → dense GameWorld PlayerId
    host_player_to_gw: HashMap<u32, PlayerId>,
    /// Host player relationship snapshot for HordeUpdate AlliesOnly
    /// (C++ `PartitionFilterHordeMember` `getRelationship == ALLIES`).
    horde_player_rel: HashMap<u32, HordePlayerRel>,

    /// Last host energy shortfall residual per producer host id (sole-tick).
    production_power_factor_by_host: HashMap<u32, f32>,
    special_power_frozen_by_host: HashMap<u32, bool>,
    /// Host world XZ extent for DeliverPayload HeadOffMap / isOffMap.
    map_min_x: f32,
    map_min_z: f32,
    map_max_x: f32,
    map_max_z: f32,

    /// Pending A10 missile drops (mirrored from host registry under dual-tick).
    a10_pending_drops: Vec<crate::game_logic::host_a10_strike_flight::PendingA10MissileDrop>,
    artillery_pending_drops:
        Vec<crate::game_logic::host_artillery_barrage_flight::PendingArtilleryShellDrop>,
    carpet_pending_drops: Vec<crate::game_logic::host_carpet_bomb_flight::PendingCarpetBombDrop>,
}

#[cfg(test)]
mod tests;

/// Concatenated live gameworld_shadow sources for residual `include_str` scans.
pub const GAMEWORLD_SHADOW_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("construct.rs"),
    include_str!("object_observation.rs"),
    include_str!("construction_observation.rs"),
    include_str!("counts.rs"),
    include_str!("observation_metrics.rs"),
    include_str!("weapon_observation.rs"),
    include_str!("projectile_observation.rs"),
    include_str!("presentation.rs"),
    include_str!("session.rs"),
    include_str!("types.rs"),
    include_str!("tick/env.rs"),
    include_str!("tick/mod.rs"),
);
