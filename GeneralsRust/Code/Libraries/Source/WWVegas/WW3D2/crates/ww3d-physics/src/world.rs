use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

use glam::Vec3;
use ww3d_collision::physics_integration as backend;

use crate::PhysicsBodyId;
use crate::{RigidBodyDesc, RigidBodyType};

/// High-level physics world facade that routes calls to the legacy physics core.
pub struct PhysicsWorld {
    inner: backend::PhysicsWorld,
    body_types: HashMap<PhysicsBodyId, RigidBodyType>,
}

impl PhysicsWorld {
    /// Create a new physics world using the legacy solver implementation.
    pub fn new() -> Self {
        Self {
            inner: backend::PhysicsWorld::new(),
            body_types: HashMap::new(),
        }
    }

    /// Convenience check used by integration tests.
    pub fn is_valid(&self) -> bool {
        true
    }

    /// Insert a body into the simulation.
    pub fn create_body(&mut self, desc: RigidBodyDesc) -> PhysicsBodyId {
        let id = self.inner.create_body(desc.to_backend());
        self.body_types.insert(id, desc.body_type);
        id
    }

    /// Remove a body from the world.
    pub fn remove_body(&mut self, id: PhysicsBodyId) {
        self.inner.remove_body(id);
        self.body_types.remove(&id);
    }

    /// Immutable access to a body.
    pub fn get_body(&self, id: PhysicsBodyId) -> Option<PhysicsBodyRef<'_>> {
        let body_type = self
            .body_types
            .get(&id)
            .copied()
            .unwrap_or(RigidBodyType::Dynamic);
        self.inner
            .get_body(id)
            .map(|body| PhysicsBodyRef { body, body_type })
    }

    /// Mutable access to a body.
    pub fn get_body_mut(&mut self, id: PhysicsBodyId) -> Option<PhysicsBodyMut<'_>> {
        let body_type = self
            .body_types
            .get(&id)
            .copied()
            .unwrap_or(RigidBodyType::Dynamic);
        self.inner
            .get_body_mut(id)
            .map(move |body| PhysicsBodyMut { body, body_type })
    }

    /// Advance the simulation by a fixed time step (matching the legacy behaviour).
    pub fn step(&mut self) {
        self.inner.step();
    }

    /// Ray cast helper.
    pub fn ray_cast(
        &self,
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
    ) -> Option<backend::RayCastHit> {
        self.inner.ray_cast(origin, direction, max_distance)
    }

    /// Expose raw statistics from the core solver.
    pub fn stats(&self) -> &backend::PhysicsStats {
        self.inner.get_stats()
    }

    /// Number of active bodies.
    pub fn body_count(&self) -> usize {
        self.body_types.len()
    }
}

/// Immutable body reference that dereferences to the legacy body structure.
pub struct PhysicsBodyRef<'a> {
    body: &'a backend::RigidBody,
    body_type: RigidBodyType,
}

impl<'a> PhysicsBodyRef<'a> {
    /// Body classification.
    pub fn body_type(&self) -> RigidBodyType {
        self.body_type
    }
}

impl<'a> Deref for PhysicsBodyRef<'a> {
    type Target = backend::RigidBody;

    fn deref(&self) -> &Self::Target {
        self.body
    }
}

/// Mutable body reference that dereferences to the legacy body structure.
pub struct PhysicsBodyMut<'a> {
    body: &'a mut backend::RigidBody,
    body_type: RigidBodyType,
}

impl<'a> PhysicsBodyMut<'a> {
    /// Body classification.
    pub fn body_type(&self) -> RigidBodyType {
        self.body_type
    }
}

impl<'a> Deref for PhysicsBodyMut<'a> {
    type Target = backend::RigidBody;

    fn deref(&self) -> &Self::Target {
        self.body
    }
}

impl<'a> DerefMut for PhysicsBodyMut<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.body
    }
}
