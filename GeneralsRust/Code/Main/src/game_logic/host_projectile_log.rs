//! Frame-local host projectile flight log for GameWorld SetProjectileFlight parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostProjectileEvent {
    pub host_id: u32,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub target_position: [f32; 3],
    pub damage: f32,
    pub shooter_host: u32,
    pub target_host: u32,
    pub speed: f32,
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub is_homing: bool,
    pub active: bool,
}

thread_local! {
    static LOG: RefCell<Vec<HostProjectileEvent>> = RefCell::new(Vec::new());
}

pub fn record(
    host_id: u32,
    position: [f32; 3],
    velocity: [f32; 3],
    target_position: [f32; 3],
    damage: f32,
    shooter_host: u32,
    target_host: u32,
    speed: f32,
    lifetime: f32,
    max_lifetime: f32,
    is_homing: bool,
    active: bool,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostProjectileEvent {
            host_id,
            position,
            velocity,
            target_position,
            damage,
            shooter_host,
            target_host,
            speed,
            lifetime,
            max_lifetime,
            is_homing,
            active,
        });
    });
}

/// Snapshot all active combat projectiles into the frame log.
pub fn record_snapshot<'a, I>(projectiles: I)
where
    I: IntoIterator<Item = &'a crate::game_logic::combat::Projectile>,
{
    for p in projectiles {
        record(
            p.id.0,
            [p.position.x, p.position.y, p.position.z],
            [p.velocity.x, p.velocity.y, p.velocity.z],
            [
                p.target_position.x,
                p.target_position.y,
                p.target_position.z,
            ],
            p.damage,
            p.shooter_id.0,
            p.target_id.map(|t| t.0).unwrap_or(0),
            p.speed,
            p.lifetime,
            p.max_lifetime,
            p.is_homing,
            true,
        );
    }
}

pub fn drain() -> Vec<HostProjectileEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
