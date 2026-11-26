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
#[cfg(test)]
pub(crate) use host_observation_boundary::run_owned_host_boundary;
pub use presentation::*;
pub use session::*;

/// Session holding GameWorld + stable host↔entity ID maps.
#[derive(Debug)]
pub struct GameWorldShadow {
    /// Tint owner for hostless presentation through the shared shadow API.
    pub(crate) drawable_tint_envelopes: std::cell::RefCell<crate::game_logic::DrawableTintEnvelopes>,
    world: GameWorld,
    // Id-keyed lookup tables: iteration order never feeds sim decisions
    // (stale removal / metrics are order-independent), so use the Fx hasher.
    host_to_entity: game_engine::common::fast_hash::FxHashMap<u32, EntityId>,
    entity_to_host: game_engine::common::fast_hash::FxHashMap<u32, u32>,
    // Births are assigned in numeric host order. Reuse that order until the
    // receiving registration list changes, including between tick and present.
    registered_host_ids: Vec<ObjectId>,
    sorted_host_ids: Vec<ObjectId>,
    checked_entity_roster: Option<(u32, u64)>,
    observed_object_revision: Option<ObjectObservationRevision>,
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

    carpet_pending_drops: Vec<crate::game_logic::host_carpet_bomb_flight::PendingCarpetBombDrop>,
}

#[derive(Debug)]
struct ObjectObservationRevision {
    host: std::sync::Weak<()>,
    host_revision: u64,
    entity_roster: (u32, u64),
}

#[cfg(test)]
mod tests;
#[cfg(test)]
pub(crate) use tests::harness::isolated_shadow_case;
