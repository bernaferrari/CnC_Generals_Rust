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
    pub waiting_for_path: bool,
    pub locomotor_surfaces: u32,
    pub is_attack_path: bool,
    pub is_blocked_and_stuck: bool,
    pub is_braking: bool,
    pub is_safe_path: bool,
    pub queue_for_path_frames: u32,
    pub path_timestamp: u32,
    pub cur_max_blocked_speed: f32,
    pub num_frames_blocked: u32,
    pub is_blocked: bool,
    /// Host ObjectId.0 for move-away-from target.
    pub move_away_from_id: Option<u32>,
    /// Host ObjectId.0 for requested victim.
    pub requested_victim_id: Option<u32>,
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
    waiting_for_path: bool,
    locomotor_surfaces: u32,
    is_attack_path: bool,
    is_blocked_and_stuck: bool,
    is_braking: bool,
    is_safe_path: bool,
    queue_for_path_frames: u32,
    path_timestamp: u32,
    cur_max_blocked_speed: f32,
    num_frames_blocked: u32,
    is_blocked: bool,
    move_away_from_id: Option<u32>,
    requested_victim_id: Option<u32>,
) {
    let path_waypoints: Vec<[f32; 3]> = path.iter().take(64).map(|p| [p.x, p.y, p.z]).collect();
    LOG.with(|log| {
        log.borrow_mut().push(HostMovementEvent {
            object,
            velocity: [velocity.x, velocity.y, velocity.z],
            max_speed,
            path_index: path_index.min(u16::MAX as usize) as u16,
            path_len: path.len().min(u16::MAX as usize) as u16,
            path_waypoints,
            waiting_for_path,
            locomotor_surfaces,
            is_attack_path,
            is_blocked_and_stuck,
            is_braking,
            is_safe_path,
            queue_for_path_frames,
            path_timestamp,
            cur_max_blocked_speed,
            num_frames_blocked,
            is_blocked,
            move_away_from_id,
            requested_victim_id,
        });
    });
}

pub fn drain() -> Vec<HostMovementEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
