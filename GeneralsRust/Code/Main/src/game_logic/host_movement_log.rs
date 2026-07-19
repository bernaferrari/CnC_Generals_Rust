//! Frame-local host movement residual log for GameWorld SetMovement parity.

use super::ObjectId;
use glam::Vec3;
use std::cell::RefCell;

#[derive(Debug, Clone, PartialEq)]
pub struct HostMovementEvent {
    pub object: ObjectId,
    pub velocity: [f32; 3],
    pub max_speed: f32,
    pub path_index: u16,
    pub path_len: u16,
    /// Waypoints truncated for channel volume.
    pub path_waypoints: Vec<[f32; 3]>,
}

thread_local! {
    static LOG: RefCell<Vec<HostMovementEvent>> = RefCell::new(Vec::new());
}

pub fn record(
    object: ObjectId,
    velocity: Vec3,
    max_speed: f32,
    path_index: usize,
    path: &[Vec3],
) {
    let path_waypoints: Vec<[f32; 3]> = path
        .iter()
        .take(64)
        .map(|p| [p.x, p.y, p.z])
        .collect();
    LOG.with(|log| {
        log.borrow_mut().push(HostMovementEvent {
            object,
            velocity: [velocity.x, velocity.y, velocity.z],
            max_speed,
            path_index: path_index.min(u16::MAX as usize) as u16,
            path_len: path.len().min(u16::MAX as usize) as u16,
            path_waypoints,
        });
    });
}

pub fn drain() -> Vec<HostMovementEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
