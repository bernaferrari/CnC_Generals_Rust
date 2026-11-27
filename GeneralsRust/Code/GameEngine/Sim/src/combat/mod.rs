//! Weapon delivery, projectile flight, and impacts for one simulation.

pub mod bezier;
mod damage_event;
pub mod missile_motion;
pub mod projectile;
pub mod projectile_arena;
pub mod projectile_contact;
pub mod projectile_flight;
pub mod projectile_impact;
pub mod projectile_lifecycle;
pub mod projectile_stream;
mod shots;

pub use damage_event::DamageEvent;
pub use shots::{
    DelayedDamage, PendingCombatSnapshot, PendingProjectile, ProjectileLaunchContext,
    ProjectileSpeedUnit, ShotQueue, is_instant_weapon_speed,
};
