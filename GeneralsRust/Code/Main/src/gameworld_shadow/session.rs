//! Read-only projection helpers for independently owned simulation sessions.

use super::*;
use crate::game_logic::{GameLogic, ObjectId, Team};
use gamelogic::world::entities::{EntityId, EntityProductionItem, TemplateRef, Transform};
use gamelogic::world::{GameWorld, PlayerId, WorldMutation, WorldSnapshot};
use std::collections::{HashMap, HashSet};

/// Rebuild convenience: one-shot mirror (stable map discarded with the session).
pub fn mirror_host_into_gameworld(logic: &GameLogic, max_entities: usize) -> GameWorld {
    let mut shadow = GameWorldShadow::new(max_entities);
    shadow.sync_from_host(logic);
    std::mem::replace(&mut shadow.world, GameWorld::new(8))
}

/// Incremental API with stable IDs: sync into an existing shadow session.
pub fn remirror_host_into_gameworld(world: &mut GameWorld, logic: &GameLogic, max_entities: usize) {
    // Legacy signature: no session — full replace (unstable IDs).
    *world = mirror_host_into_gameworld(logic, max_entities);
}

/// Session-based remirror (preferred).
pub fn sync_shadow_from_host(shadow: &mut GameWorldShadow, logic: &GameLogic) {
    shadow.sync_from_host(logic);
}

/// Build shadow session + probe.
pub fn probe_host_vs_gameworld(logic: &GameLogic) -> (GameWorldShadow, GameWorldShadowProbe) {
    const MAX_ENTITIES: usize = 4096;
    let mut shadow = GameWorldShadow::new(MAX_ENTITIES);
    shadow.sync_from_host(logic);
    let probe = shadow.probe(logic);
    (shadow, probe)
}
