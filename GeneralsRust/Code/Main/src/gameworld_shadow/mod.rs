//! Legacy GameWorld mirror used by the remaining comparison tests.

use crate::game_logic::{GameLogic, ObjectId, Team};

use gamelogic::world::entities::{EntityId, EntityProductionItem, TemplateRef, Transform};
use gamelogic::world::GameWorld;
use std::collections::{HashMap, HashSet};

mod construct;

/// Comparison world with a lookup from ObjectIDs to mirrored entities.
#[derive(Debug)]
pub struct GameWorldShadow {
    world: GameWorld,
    // Id-keyed lookup tables: iteration order never feeds sim decisions
    // (stale removal is order-independent), so use the Fx hasher.
    host_to_entity: game_engine::common::fast_hash::FxHashMap<u32, EntityId>,
}

#[cfg(test)]
mod tests;
