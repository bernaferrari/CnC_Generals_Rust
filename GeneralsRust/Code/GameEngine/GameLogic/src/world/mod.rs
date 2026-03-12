//! Lightweight world representation used by the modern game-logic core.

pub mod entities;

use self::entities::{EntityId, EntityStore, TemplateRef, Transform};
use std::collections::VecDeque;
use std::fmt;
use std::mem;

/// Identifier assigned to players.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlayerId(u8);

impl PlayerId {
    /// First playable slot.
    pub const FIRST: PlayerId = PlayerId(0);
    /// Neutral/observer slot.
    pub const NEUTRAL: PlayerId = PlayerId(0);

    /// Raw numeric value.
    pub fn get(self) -> u8 {
        self.0
    }
}

impl fmt::Display for PlayerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.get())
    }
}

/// Publicly visible player information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerInfo {
    /// Player identifier.
    pub id: PlayerId,
    /// Display name.
    pub name: String,
    /// Team slot (if any).
    pub team: Option<u8>,
    /// Whether the player is controlled by a human.
    pub is_human: bool,
}

/// Immutable snapshot of the world state for deterministic consumers.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldSnapshot {
    /// Frame index captured when the snapshot was produced.
    pub frame: u64,
    /// Present players in the simulation.
    pub players: Vec<PlayerInfo>,
    /// Active entities at the time of the snapshot.
    pub entities: Vec<EntitySummary>,
}

/// Summary information about an entity included in world snapshots.
#[derive(Debug, Clone, PartialEq)]
pub struct EntitySummary {
    /// Entity identifier.
    pub id: EntityId,
    /// Template name backing the entity.
    pub template: String,
    /// Owning player (if any).
    pub owner: Option<PlayerId>,
    /// World-space position.
    pub position: [f32; 3],
    /// Facing angle in radians.
    pub orientation: f32,
    /// Remaining hitpoints.
    pub health: f32,
}

/// Internal record stored for every allocated player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerData {
    /// Display name.
    pub name: String,
    /// Team slot (if any).
    pub team: Option<u8>,
    /// Indicates whether the player is human-controlled.
    pub is_human: bool,
}

impl PlayerData {
    /// Convert the stored record into a publicly shareable snapshot.
    pub fn to_info(&self, id: PlayerId) -> PlayerInfo {
        PlayerInfo {
            id,
            name: self.name.clone(),
            team: self.team,
            is_human: self.is_human,
        }
    }
}

#[derive(Debug, Clone)]
enum PlayerState {
    Vacant,
    Active(PlayerData),
}

impl Default for PlayerState {
    fn default() -> Self {
        PlayerState::Vacant
    }
}

#[derive(Debug, Clone)]
struct PlayerSlot {
    id: PlayerId,
    state: PlayerState,
}

impl PlayerSlot {
    fn is_active(&self) -> bool {
        matches!(self.state, PlayerState::Active(_))
    }

    fn summary(&self) -> Option<PlayerInfo> {
        match &self.state {
            PlayerState::Active(data) => Some(data.to_info(self.id)),
            PlayerState::Vacant => None,
        }
    }

    fn activate(&mut self, data: PlayerData) {
        self.state = PlayerState::Active(data);
    }

    fn deactivate(&mut self) -> Option<PlayerData> {
        match mem::replace(&mut self.state, PlayerState::Vacant) {
            PlayerState::Active(data) => Some(data),
            PlayerState::Vacant => None,
        }
    }

    fn data(&self) -> Option<&PlayerData> {
        match &self.state {
            PlayerState::Active(data) => Some(data),
            PlayerState::Vacant => None,
        }
    }

    fn data_mut(&mut self) -> Option<&mut PlayerData> {
        match &mut self.state {
            PlayerState::Active(data) => Some(data),
            PlayerState::Vacant => None,
        }
    }
}

/// Minimal world storage.
#[derive(Debug)]
pub struct World {
    frame: u64,
    slots: Vec<PlayerSlot>,
    available_ids: VecDeque<PlayerId>,
    entities: EntityStore,
}

impl World {
    /// Create a new world supporting the requested number of players.
    pub fn new(max_players: usize) -> Self {
        let mut slots = Vec::with_capacity(max_players);
        let mut available_ids = VecDeque::with_capacity(max_players);
        for idx in 0..max_players.min(255) {
            let id = PlayerId(idx as u8);
            slots.push(PlayerSlot {
                id,
                state: PlayerState::Vacant,
            });
            available_ids.push_back(id);
        }

        Self {
            frame: 0,
            slots,
            available_ids,
            entities: EntityStore::new(),
        }
    }

    /// Adjust the number of supported player slots.
    pub fn resize(&mut self, max_players: usize) {
        if max_players == self.slots.len() {
            return;
        }

        let mut new_self = World::new(max_players);
        for slot in self.slots.iter().filter(|slot| slot.is_active()) {
            if let Some(id) = new_self.allocate_player_internal() {
                if id == slot.id {
                    if let Some(entry) = new_self.slots.iter_mut().find(|s| s.id == id) {
                        if let Some(data) = slot.data() {
                            entry.activate(data.clone());
                        }
                    }
                }
            }
        }
        new_self.frame = self.frame;
        new_self.entities = self.entities.clone();
        *self = new_self;
    }

    /// Mark the simulation as having advanced to the next frame.
    pub(crate) fn advance(&mut self, frame: u32) {
        self.frame = frame as u64;
    }

    /// Allocate a player slot if available.
    pub fn allocate_player(&mut self) -> Option<PlayerId> {
        self.allocate_player_with_name(None, None, true)
    }

    /// Allocate a player slot with custom metadata.
    pub fn allocate_player_with_name(
        &mut self,
        name: Option<String>,
        team: Option<u8>,
        is_human: bool,
    ) -> Option<PlayerId> {
        let id = self.allocate_player_internal()?;
        if let Some(slot) = self.slots.iter_mut().find(|slot| slot.id == id) {
            let player_name = name.unwrap_or_else(|| format!("Player {}", id.get() + 1));
            slot.activate(PlayerData {
                name: player_name,
                team,
                is_human,
            });
        }
        Some(id)
    }

    fn allocate_player_internal(&mut self) -> Option<PlayerId> {
        self.available_ids.pop_front()
    }

    /// Remove a player. Returns a snapshot of the removed player if the slot
    /// was occupied.
    pub fn remove_player(&mut self, id: PlayerId) -> Option<PlayerInfo> {
        if let Some(slot) = self.slots.iter_mut().find(|slot| slot.id == id) {
            if let Some(data) = slot.deactivate() {
                let info = data.to_info(id);
                self.available_ids.push_back(id);
                return Some(info);
            }
        }
        None
    }

    /// Fetch immutable player data.
    pub fn player(&self, id: PlayerId) -> Option<&PlayerData> {
        self.slots
            .iter()
            .find(|slot| slot.id == id)
            .and_then(PlayerSlot::data)
    }

    /// Fetch mutable player data.
    pub fn player_mut(&mut self, id: PlayerId) -> Option<&mut PlayerData> {
        self.slots
            .iter_mut()
            .find(|slot| slot.id == id)
            .and_then(PlayerSlot::data_mut)
    }

    /// Iterate over active players alongside their data.
    pub fn active_players(&self) -> impl Iterator<Item = (PlayerId, &PlayerData)> {
        self.slots
            .iter()
            .filter_map(|slot| slot.data().map(|data| (slot.id, data)))
    }

    /// Number of active players currently occupying slots.
    pub fn active_player_count(&self) -> usize {
        self.slots.iter().filter(|slot| slot.is_active()).count()
    }

    /// Number of entities present in the world.
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    /// Produce an immutable view of the world.
    pub fn snapshot(&self) -> WorldSnapshot {
        let players = self.slots.iter().filter_map(PlayerSlot::summary).collect();
        let entities = self.entities.iter().map(World::entity_summary).collect();
        WorldSnapshot {
            frame: self.frame,
            players,
            entities,
        }
    }

    /// Spawn an entity in the world and return its identifier.
    pub fn spawn_entity(
        &mut self,
        template: TemplateRef,
        owner: Option<PlayerId>,
        transform: Transform,
        health: f32,
    ) -> EntityId {
        self.entities.spawn(template, owner, transform, health)
    }

    /// Remove an entity; returns `true` if one was removed.
    pub fn remove_entity(&mut self, id: EntityId) -> bool {
        self.entities.remove(id).is_some()
    }

    /// Immutable access to an entity.
    pub fn entity(&self, id: EntityId) -> Option<&entities::Entity> {
        self.entities.get(id)
    }

    /// Iterate over all active entities.
    pub fn entities(&self) -> impl Iterator<Item = &entities::Entity> {
        self.entities.iter()
    }

    /// Snapshot information for a specific entity.
    pub fn entity_summary_by_id(&self, id: EntityId) -> Option<EntitySummary> {
        self.entities.get(id).map(World::entity_summary)
    }

    fn entity_summary(entity: &entities::Entity) -> EntitySummary {
        EntitySummary {
            id: entity.id,
            template: entity.template.name.clone(),
            owner: entity.owner,
            position: [
                entity.transform.position.x,
                entity.transform.position.y,
                entity.transform.position.z,
            ],
            orientation: entity.transform.orientation,
            health: entity.health,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::entities::{TemplateRef, Transform};
    use super::*;

    #[test]
    fn allocates_players_until_capacity() {
        let mut world = World::new(2);
        let first = world.allocate_player().unwrap();
        assert_eq!(first, PlayerId::FIRST);
        let second = world.allocate_player().unwrap();
        assert_eq!(second.get(), 1);
        assert!(world.allocate_player().is_none());

        let removed = world.remove_player(first).expect("player removed");
        assert_eq!(removed.id, first);
        let recycled = world.allocate_player().unwrap();
        assert_eq!(recycled, first);
    }

    #[test]
    fn snapshot_reflects_active_players() {
        let mut world = World::new(3);
        world.allocate_player();
        world.advance(5);

        let snapshot = world.snapshot();
        assert_eq!(snapshot.frame, 5);
        assert_eq!(snapshot.players.len(), 1);
        assert!(snapshot.entities.is_empty());
    }

    #[test]
    fn resize_preserves_active_players() {
        let mut world = World::new(4);
        let p1 = world.allocate_player_with_name(Some("Alice".into()), Some(1), true);
        let p2 = world.allocate_player_with_name(Some("Bot".into()), Some(2), false);
        assert!(p1.is_some() && p2.is_some());

        world.resize(2);
        let snapshot = world.snapshot();
        assert_eq!(snapshot.players.len(), 2);
        assert!(snapshot.players.iter().any(|p| p.name == "Alice"));
        assert!(snapshot.players.iter().any(|p| p.name == "Bot"));
        assert!(snapshot.players.iter().all(|p| p.team.is_some()));
    }

    #[test]
    fn removing_unknown_player_is_noop() {
        let mut world = World::new(1);
        assert!(world.remove_player(PlayerId::FIRST).is_none());
    }

    #[test]
    fn entity_lifecycle_affects_snapshot() {
        let mut world = World::new(2);
        let entity_id = world.spawn_entity(
            TemplateRef::new("GLAInfantryRebel"),
            Some(PlayerId::FIRST),
            Transform::new([0.0, 0.0, 0.0], 0.0),
            100.0,
        );

        let snapshot = world.snapshot();
        assert_eq!(snapshot.entities.len(), 1);
        let entity = &snapshot.entities[0];
        assert_eq!(entity.id, entity_id);
        assert_eq!(entity.template, "GLAInfantryRebel");
        assert_eq!(entity.owner, Some(PlayerId::FIRST));

        assert!(world.remove_entity(entity_id));
        let snapshot = world.snapshot();
        assert!(snapshot.entities.is_empty());
    }
}
