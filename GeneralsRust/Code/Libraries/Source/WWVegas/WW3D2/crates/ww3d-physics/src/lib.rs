//! High-level physics facade for the WW3D engine.
//!
//! This crate wraps the original physics implementation that lives inside the
//! `ww3d-collision` crate, exposing a modern Rust API that mirrors the original
//! C++ behaviour while keeping the math layer based on `glam`.

mod body;
mod world;

pub use body::{RigidBody, RigidBodyDesc, RigidBodyType};
pub use world::{PhysicsBodyMut, PhysicsBodyRef, PhysicsWorld};

pub use ww3d_collision::physics_integration::{
    CollisionShape, PhysicsBodyId, PhysicsStats, RayCastHit,
};

pub use glam::{Mat4, Quat, Vec3};
