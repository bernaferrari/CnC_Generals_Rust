use glam::{Quat, Vec3};
use ww3d_collision::physics_integration as backend;

use crate::CollisionShape;

/// High-level body classification matching the legacy WW3D engine semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RigidBodyType {
    /// Immovable body that never responds to forces.
    Static,
    /// Moves according to explicit velocities but ignores forces.
    Kinematic,
    /// Fully simulated body influenced by forces and impulses.
    Dynamic,
}

impl Default for RigidBodyType {
    fn default() -> Self {
        Self::Dynamic
    }
}

/// Lightweight standalone rigid body used for quick construction/tests.
#[derive(Debug, Clone)]
pub struct RigidBody {
    body_type: RigidBodyType,
    position: Vec3,
    rotation: Quat,
    mass: f32,
}

impl RigidBody {
    /// Create a new standalone rigid body description.
    pub fn new(body_type: RigidBodyType, position: Vec3, rotation: Quat, mass: f32) -> Self {
        let clamped_mass = if matches!(body_type, RigidBodyType::Static | RigidBodyType::Kinematic)
        {
            0.0
        } else {
            mass.max(0.0)
        };

        Self {
            body_type,
            position,
            rotation,
            mass: clamped_mass,
        }
    }

    /// Mass in kilograms.
    pub fn mass(&self) -> f32 {
        self.mass
    }

    /// Initial position in world space.
    pub fn position(&self) -> Vec3 {
        self.position
    }

    /// Initial orientation in world space.
    pub fn rotation(&self) -> Quat {
        self.rotation
    }

    /// Body classification.
    pub fn body_type(&self) -> RigidBodyType {
        self.body_type
    }
}

/// Description used when inserting bodies into the shared physics world.
#[derive(Debug, Clone)]
pub struct RigidBodyDesc {
    pub body_type: RigidBodyType,
    pub position: Vec3,
    pub rotation: Quat,
    pub linear_velocity: Vec3,
    pub angular_velocity: Vec3,
    pub mass: f32,
    pub shape: CollisionShape,
    pub restitution: f32,
    pub friction: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
}

impl Default for RigidBodyDesc {
    fn default() -> Self {
        Self {
            body_type: RigidBodyType::Dynamic,
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            linear_velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            mass: 1.0,
            shape: CollisionShape::Sphere { radius: 1.0 },
            restitution: 0.3,
            friction: 0.5,
            linear_damping: 0.05,
            angular_damping: 0.05,
        }
    }
}

impl RigidBodyDesc {
    pub(crate) fn to_backend(&self) -> backend::RigidBodyDesc {
        let mass = match self.body_type {
            RigidBodyType::Static | RigidBodyType::Kinematic => 0.0,
            RigidBodyType::Dynamic => self.mass.max(0.0),
        };

        backend::RigidBodyDesc {
            position: self.position,
            rotation: self.rotation,
            linear_velocity: self.linear_velocity,
            angular_velocity: self.angular_velocity,
            mass,
            shape: self.shape.clone(),
            restitution: self.restitution,
            friction: self.friction,
            linear_damping: self.linear_damping,
            angular_damping: self.angular_damping,
        }
    }
}
