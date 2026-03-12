//! Entity storage and helpers mirroring the legacy object/thing system.
//!
//! The original engine routes almost everything through the global
//! `ObjectManager`.  Here we provide a modern, owned representation that still
//! uses familiar terminology (entity, template, owner) so porting code can stay
//! close to the C++ layout while benefiting from Rust's safety.

use crate::world::PlayerId;
use nalgebra::Point3;
use std::collections::HashMap;

/// Identifier assigned to entities/things in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityId(u32);

impl EntityId {
    /// First valid entity identifier.
    pub const FIRST: EntityId = EntityId(1);

    /// Raw numeric accessor.
    pub fn get(self) -> u32 {
        self.0
    }
}

/// Runtime description of a template. In the legacy engine this maps to
/// `ThingTemplate`.  We keep the fields intentionally small until the
/// higher-level systems are ported.
#[derive(Debug, Clone)]
pub struct TemplateRef {
    /// Stable name (matches C++ `ThingTemplate::GetName()`).
    pub name: String,
    /// Optional path to the definition file.
    pub source: Option<String>,
}

impl TemplateRef {
    /// Create a new template reference.
    pub fn new<S: Into<String>>(name: S) -> Self {
        Self {
            name: name.into(),
            source: None,
        }
    }
}

/// Minimal spatial information for an entity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    /// World-space position (X/Y/Z).
    pub position: Point3<f32>,
    /// Facing angle in radians.
    pub orientation: f32,
}

impl Transform {
    /// Create a new transform.
    pub fn new(position: [f32; 3], orientation: f32) -> Self {
        Self {
            position: Point3::from(position),
            orientation,
        }
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Point3::new(0.0, 0.0, 0.0),
            orientation: 0.0,
        }
    }
}

/// Core runtime data for an entity.
#[derive(Debug, Clone)]
pub struct Entity {
    /// Unique identifier.
    pub id: EntityId,
    /// Template metadata.
    pub template: TemplateRef,
    /// Owning player (if any).
    pub owner: Option<PlayerId>,
    /// Spatial state.
    pub transform: Transform,
    /// Current hitpoints.
    pub health: f32,
}

impl Entity {
    /// Convenience accessor for the template name.
    pub fn template_name(&self) -> &str {
        &self.template.name
    }
}

/// Store responsible for allocating and tracking entities.
#[derive(Debug, Default, Clone)]
pub struct EntityStore {
    next_id: u32,
    alive: HashMap<EntityId, Entity>,
}

impl EntityStore {
    /// Create a new store.
    pub fn new() -> Self {
        Self {
            next_id: EntityId::FIRST.get(),
            alive: HashMap::new(),
        }
    }

    /// Number of living entities.
    pub fn len(&self) -> usize {
        self.alive.len()
    }

    /// Returns true if no entities are alive.
    pub fn is_empty(&self) -> bool {
        self.alive.is_empty()
    }

    /// Iterate over entities.
    pub fn iter(&self) -> impl Iterator<Item = &Entity> {
        self.alive.values()
    }

    /// Get a specific entity.
    pub fn get(&self, id: EntityId) -> Option<&Entity> {
        self.alive.get(&id)
    }

    /// Mutable accessor.
    pub fn get_mut(&mut self, id: EntityId) -> Option<&mut Entity> {
        self.alive.get_mut(&id)
    }

    /// Spawn a new entity using the provided template and initial state.
    pub fn spawn(
        &mut self,
        template: TemplateRef,
        owner: Option<PlayerId>,
        transform: Transform,
        health: f32,
    ) -> EntityId {
        let id = EntityId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1).max(EntityId::FIRST.get());

        let entity = Entity {
            id,
            template,
            owner,
            transform,
            health,
        };

        self.alive.insert(id, entity);
        id
    }

    /// Remove an entity. Returns the removed entity if it was alive.
    pub fn remove(&mut self, id: EntityId) -> Option<Entity> {
        self.alive.remove(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_and_remove_entity() {
        let mut store = EntityStore::new();
        assert_eq!(store.len(), 0);

        let id = store.spawn(
            TemplateRef::new("GLAInfantryRebel"),
            Some(PlayerId::FIRST),
            Transform::new([10.0, 5.0, 0.0], 1.57),
            100.0,
        );

        let entity = store.get(id).expect("entity spawned");
        assert_eq!(entity.template_name(), "GLAInfantryRebel");
        assert_eq!(entity.owner, Some(PlayerId::FIRST));

        let removed = store.remove(id).expect("removed entity");
        assert_eq!(removed.id, id);
        assert!(store.is_empty());
    }
}
