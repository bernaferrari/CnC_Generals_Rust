//! Accepted weapon shots, delayed deliveries, and projectile trails for one world.

pub mod projectile_lifecycle;
pub mod projectile_stream;
mod shots;

pub use shots::{
    DelayedDamage, PendingCombatSnapshot, PendingProjectile, ProjectileLaunchContext,
    ProjectileSpeedUnit, ShotQueue, is_instant_weapon_speed,
};
