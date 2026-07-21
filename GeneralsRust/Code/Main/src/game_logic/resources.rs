use super::*;
use serde::{Deserialize, Serialize};

/// Resource types in the game
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceType {
    Supplies,
    Power,
}

/// Resource gathering state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceGatherer {
    pub gathering: bool,
    pub target_source: Option<ObjectId>,
    pub carried_amount: u32,
    pub capacity: u32,
    pub gather_rate: f32, // Resources per second
}

impl Default for ResourceGatherer {
    fn default() -> Self {
        Self {
            gathering: false,
            target_source: None,
            carried_amount: 0,
            capacity: 1000,
            gather_rate: 100.0, // 100 supplies per second
        }
    }
}

/// Resource source (supply stashes, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSource {
    pub resource_type: ResourceType,
    pub amount_remaining: u32,
    pub max_amount: u32,
    pub gatherers: Vec<ObjectId>,
    pub max_gatherers: usize,
}

impl ResourceSource {
    pub fn new(resource_type: ResourceType, amount: u32) -> Self {
        Self {
            resource_type,
            amount_remaining: amount,
            max_amount: amount,
            gatherers: Vec::new(),
            max_gatherers: 3, // Max 3 gatherers per source
        }
    }

    pub fn can_accept_gatherer(&self) -> bool {
        self.gatherers.len() < self.max_gatherers && self.amount_remaining > 0
    }

    pub fn add_gatherer(&mut self, gatherer_id: ObjectId) -> bool {
        if self.can_accept_gatherer() {
            self.gatherers.push(gatherer_id);
            true
        } else {
            false
        }
    }

    pub fn remove_gatherer(&mut self, gatherer_id: ObjectId) {
        self.gatherers.retain(|&id| id != gatherer_id);
    }

    pub fn gather(&mut self, amount: u32) -> u32 {
        let gathered = amount.min(self.amount_remaining);
        self.amount_remaining -= gathered;
        gathered
    }

    pub fn is_depleted(&self) -> bool {
        self.amount_remaining == 0
    }

    pub fn percentage_remaining(&self) -> f32 {
        if self.max_amount > 0 {
            self.amount_remaining as f32 / self.max_amount as f32
        } else {
            0.0
        }
    }
}

/// Resource management system
pub struct ResourceManager {
    pub supply_sources: HashMap<ObjectId, ResourceSource>,
}

impl ResourceManager {
    pub fn new() -> Self {
        Self {
            supply_sources: HashMap::new(),
        }
    }

    pub fn create_supply_source(&mut self, object_id: ObjectId, amount: u32) {
        let source = ResourceSource::new(ResourceType::Supplies, amount);
        self.supply_sources.insert(object_id, source);
    }

    pub fn find_nearest_supply_source(
        &self,
        position: Vec3,
        objects: &HashMap<ObjectId, Object>,
    ) -> Option<ObjectId> {
        // Pure residual acquire: nearest supply source that can accept a gatherer (3D).
        let candidates: Vec<_> = self
            .supply_sources
            .iter()
            .filter_map(|(&source_id, source)| {
                if !source.can_accept_gatherer() {
                    return None;
                }
                let source_obj = objects.get(&source_id)?;
                Some(
                    crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                        id: source_id,
                        team: source_obj.team,
                        position: source_obj.get_position(),
                        is_alive: source_obj.is_alive(),
                        is_neutral: source_obj.team == crate::game_logic::Team::Neutral,
                        under_construction: source_obj.status.under_construction,
                        combat_kind: true,
                        effectively_stealthed: false,
                        is_air: false,
                        eject_invulnerable: false,
                    },
                )
            })
            .collect();
        crate::game_logic::host_residual_acquire::pick_nearest_residual_target(
            ObjectId(u32::MAX),
            crate::game_logic::Team::Neutral,
            position,
            candidates,
            |_| f32::MAX,
            |_| true,
        )
        .map(|(id, _, _)| id)
    }

    pub fn assign_gatherer(&mut self, gatherer_id: ObjectId, source_id: ObjectId) -> bool {
        if let Some(source) = self.supply_sources.get_mut(&source_id) {
            source.add_gatherer(gatherer_id)
        } else {
            false
        }
    }

    pub fn unassign_gatherer(&mut self, gatherer_id: ObjectId, source_id: ObjectId) {
        if let Some(source) = self.supply_sources.get_mut(&source_id) {
            source.remove_gatherer(gatherer_id);
        }
    }

    pub fn gather_from_source(&mut self, source_id: ObjectId, amount: u32) -> u32 {
        if let Some(source) = self.supply_sources.get_mut(&source_id) {
            source.gather(amount)
        } else {
            0
        }
    }

    pub fn cleanup_depleted_sources(&mut self) -> Vec<ObjectId> {
        let mut depleted = Vec::new();

        self.supply_sources.retain(|&id, source| {
            if source.is_depleted() {
                depleted.push(id);
                false
            } else {
                true
            }
        });

        depleted
    }
}

impl Default for ResourceManager {
    fn default() -> Self {
        Self::new()
    }
}
