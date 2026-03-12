//! Registry managing guard behaviours for entities.

use super::guard::{GuardBehaviour, GuardEvent, GuardParameters};
use crate::runtime::SimulationEvent;
use crate::world::entities::{EntityId, Transform};
use crate::world::World;
use std::collections::HashMap;

/// Aggregated statistics describing guard controllers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GuardMetrics {
    /// Total guards registered for updates.
    pub registered: usize,
    /// Guards currently tracking a hostile target.
    pub engaged: usize,
}

struct GuardController {
    behaviour: GuardBehaviour,
}

impl GuardController {
    fn new(origin: Transform, params: GuardParameters) -> Self {
        Self {
            behaviour: GuardBehaviour::new(origin, params),
        }
    }
}

/// Stores and updates guard behaviours.
pub struct GuardRegistry {
    controllers: HashMap<EntityId, GuardController>,
}

impl GuardRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            controllers: HashMap::new(),
        }
    }

    /// Number of registered guards.
    pub fn len(&self) -> usize {
        self.controllers.len()
    }

    /// Register a guard behaviour for the given entity.
    pub fn register_guard(&mut self, entity: EntityId, origin: Transform, params: GuardParameters) {
        self.controllers
            .insert(entity, GuardController::new(origin, params));
    }

    /// Remove a guard controller when the entity is destroyed.
    pub fn remove_guard(&mut self, entity: EntityId) {
        self.controllers.remove(&entity);
    }

    /// Update guard transforms from the world state, dropping missing entities.
    pub fn sync_from_world(&mut self, world: &World) {
        self.controllers.retain(|id, controller| {
            if let Some(entity) = world.entity(*id) {
                controller.behaviour.set_transform(entity.transform);
                true
            } else {
                false
            }
        });
    }

    /// Run the guard behaviours, emitting events.
    pub fn tick(&mut self, world: &World, events: &mut Vec<SimulationEvent>) {
        self.controllers.retain(|id, controller| {
            if world.entity(*id).is_none() {
                return false;
            }

            let event = controller.behaviour.tick();
            match event {
                GuardEvent::None => true,
                GuardEvent::HostileEngaged { .. }
                | GuardEvent::ReturningToPost
                | GuardEvent::ReachedPost => {
                    events.push(SimulationEvent::GuardState { entity: *id, event });
                    true
                }
            }
        });
    }

    /// Direct hostile targeting, useful for tests and scripted events.
    pub fn spot_hostile(&mut self, guard: EntityId, target: EntityId) {
        if let Some(controller) = self.controllers.get_mut(&guard) {
            controller.behaviour.spot_hostile(target);
        }
    }

    /// Collect aggregate information about registered guards.
    pub fn metrics(&self) -> GuardMetrics {
        let engaged = self
            .controllers
            .values()
            .filter(|controller| controller.behaviour.is_engaged())
            .count();

        GuardMetrics {
            registered: self.controllers.len(),
            engaged,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn removing_missing_entities_prunes_registry() {
        let mut world = World::new(1);
        let transform = Transform::new([0.0, 0.0, 0.0], 0.0);
        let id = world.spawn_entity(
            crate::world::entities::TemplateRef::new("Guard"),
            None,
            transform,
            100.0,
        );

        let mut registry = GuardRegistry::new();
        registry.register_guard(id, transform, GuardParameters::default());

        registry.sync_from_world(&world);
        assert_eq!(registry.len(), 1);

        registry.tick(&world, &mut Vec::new());
        assert_eq!(registry.len(), 1);

        // Remove entity from world and ensure registry prunes it.
        world.remove_entity(id);
        registry.sync_from_world(&world);
        assert_eq!(registry.len(), 0);
    }
}
