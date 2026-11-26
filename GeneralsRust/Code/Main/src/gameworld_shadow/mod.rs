//! Legacy GameWorld mirror used by the remaining comparison tests.

use crate::game_logic::{GameLogic, ObjectId, Team};

use gamelogic::world::entities::{EntityId, EntityProductionItem, TemplateRef, Transform};
use gamelogic::world::{GameWorld, PlayerId, WorldMutation};
use std::collections::{HashMap, HashSet};

mod construct;
mod construction_observation;
mod projectile_observation;
mod weapon_observation;

/// Session holding GameWorld + stable host↔entity ID maps.
#[derive(Debug)]
pub struct GameWorldShadow {
    world: GameWorld,
    // Id-keyed lookup tables: iteration order never feeds sim decisions
    // (stale removal is order-independent), so use the Fx hasher.
    host_to_entity: game_engine::common::fast_hash::FxHashMap<u32, EntityId>,
    entity_to_host: game_engine::common::fast_hash::FxHashMap<u32, u32>,
    // Births are assigned in numeric host order. Reuse that order until the
    // receiving registration list changes, including between tick and present.
    registered_host_ids: Vec<ObjectId>,
    sorted_host_ids: Vec<ObjectId>,
    checked_entity_roster: Option<(u32, u64)>,
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

/// Host player factsfor C++ HordeUpdate AlliesOnly (`getRelationship == ALLIES`).
#[derive(Debug, Clone)]
pub(super) struct HordePlayerRel {
    pub alliance_team: i32,
    pub is_alive: bool,
    pub map_relations: HashMap<u32, gamelogic::common::Relationship>,
}

#[cfg(test)]
mod tests;
