//! Frame-local host projectile flight log for GameWorld SetProjectileFlight parity.

use super::ObjectId;
use gamelogic::world::ProjectileFlightState;
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
    pub flight_state: ProjectileFlightState,
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
    record_with_flight_state(
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
        ProjectileFlightState::InFlight,
        active,
    );
}

/// Record a projectile residual with its exact host-owned flight state.
///
/// Callers without parsed projectile behavior keep using [`record`], which is
/// intentionally normal in-flight state.  Only a verified MissileAIUpdate
/// KILL_SELF transition may publish `MissileKillSelfHold`.
pub fn record_with_flight_state(
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
    flight_state: ProjectileFlightState,
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
            flight_state,
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
        let flight_state = if p.is_missile_kill_self_holding() {
            ProjectileFlightState::MissileKillSelfHold
        } else {
            ProjectileFlightState::InFlight
        };
        record_with_flight_state(
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
            flight_state,
            true,
        );
    }
}

/// Publish an actual host removal so the coupled GameWorld residual cannot
/// survive after an authored host lifecycle completed.
pub fn record_retired(host_id: u32) {
    record(
        host_id,
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 0.0],
        0.0,
        0,
        0,
        0.0,
        0.0,
        0.0,
        false,
        false,
    );
}

pub fn has_pending(host_id: u32) -> bool {
    LOG.with(|log| log.borrow().iter().any(|e| e.host_id == host_id))
}

pub fn drain() -> Vec<HostProjectileEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}

pub fn len() -> usize {
    LOG.with(|log| log.borrow().len())
}
