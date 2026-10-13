//! Immutable GameWorld presentation views.

use super::*;
use crate::game_logic::{GameLogic, ObjectId, Team};
use gamelogic::world::entities::{EntityId, EntityProductionItem, TemplateRef, Transform};
use gamelogic::world::{GameWorld, PlayerId, WorldMutation, WorldSnapshot};
use std::collections::{HashMap, HashSet};

/// Observe-path presentation from GameWorld (no Main GameLogic borrow).
#[derive(Debug, Clone)]
pub struct GameWorldPresentationView {
    pub frame: u64,
    pub local_supplies: u32,
    pub entities: Vec<GameWorldEntityView>,
}

#[derive(Debug, Clone)]
pub struct GameWorldEntityView {
    pub id: u32,
    pub template: String,
    pub owner: Option<u8>,
    pub position: [f32; 3],
    pub orientation: f32,
    pub health: f32,
    /// Host max HP residual (presentation health bar).
    pub max_health: f32,
    /// Team ordinal: 0 USA, 1 China, 2 GLA, 255 Neutral.
    pub team_ordinal: u8,
    pub selected: bool,
    pub destroyed: bool,
    pub velocity: [f32; 3],
    pub move_max_speed: f32,
    pub move_target: Option<[f32; 3]>,
    pub body_damage_state: u8,
    pub team_color: [f32; 4],
}

pub fn presentation_view_from_gameworld(
    world: &GameWorld,
    local_player_index: u8,
) -> GameWorldPresentationView {
    // Prefer live Entity store over thin EntitySummary so observe-path carries
    // motion/selection/body identity (Wave 191).
    let local_supplies = world
        .player(gamelogic::world::PlayerId::from_index(local_player_index))
        .map(|p| p.supplies)
        .unwrap_or(0);
    let entities = world
        .world()
        .entities()
        .map(|e| GameWorldEntityView {
            id: e.id.get(),
            template: e.template.name.clone(),
            owner: e.owner.map(|o| o.get()),
            position: [
                e.transform.position.x,
                e.transform.position.y,
                e.transform.position.z,
            ],
            orientation: e.transform.orientation,
            health: e.health,
            max_health: e.max_health,
            team_ordinal: e.team_ordinal,
            selected: e.selected,
            destroyed: e.destroyed || e.health <= 0.0,
            velocity: e.velocity,
            move_max_speed: e.move_max_speed,
            move_target: e.move_target,
            body_damage_state: e.body_damage_state,
            team_color: e.team_color,
        })
        .collect();
    GameWorldPresentationView {
        frame: world.frame(),
        local_supplies,
        entities,
    }
}

pub fn presentation_view_from_shadow(
    shadow: &GameWorldShadow,
    local_player_index: u8,
) -> GameWorldPresentationView {
    presentation_view_from_gameworld(shadow.world(), local_player_index)
}

/// Entity count only (observe-path stamp): the same value as
/// `presentation_view_from_shadow(shadow, _).entities.len()` without
/// building per-entity views (each one clones the template name String).
pub fn presentation_entity_count_from_shadow(shadow: &GameWorldShadow) -> usize {
    presentation_entity_count_from_gameworld(shadow.world())
}

/// Entity count only for a bare GameWorld (see
/// [`presentation_entity_count_from_shadow`]).
pub fn presentation_entity_count_from_gameworld(world: &GameWorld) -> usize {
    // `EntityStore::len()` and `EntityStore::iter()` read the same map, so
    // this is exactly the view's `entities.len()`.
    world.world().entity_count()
}
