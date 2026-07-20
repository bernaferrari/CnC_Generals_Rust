//! Frame-local host physics/motive residual for GameWorld SetPhysicsMotive parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostPhysicsMotiveEvent {
    pub object: ObjectId,
    pub motive_frames_remaining: u32,
    pub physics_mass: f32,
    pub physics_accel: [f32; 3],
    pub forward_friction: f32,
    pub lateral_friction: f32,
    pub z_friction: f32,
    pub can_path_through_units: bool,
    pub ignore_collisions_until_frame: u32,
    pub is_panicking: bool,
    pub move_away_frames: u32,
}

thread_local! {
    static LOG: RefCell<Vec<HostPhysicsMotiveEvent>> = RefCell::new(Vec::new());
}

pub fn record(
    object: ObjectId,
    motive_frames_remaining: u32,
    physics_mass: f32,
    physics_accel: [f32; 3],
    forward_friction: f32,
    lateral_friction: f32,
    z_friction: f32,
    can_path_through_units: bool,
    ignore_collisions_until_frame: u32,
    is_panicking: bool,
    move_away_frames: u32,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostPhysicsMotiveEvent {
            object,
            motive_frames_remaining,
            physics_mass,
            physics_accel,
            forward_friction,
            lateral_friction,
            z_friction,
            can_path_through_units,
            ignore_collisions_until_frame,
            is_panicking,
            move_away_frames,
        });
    });
}

pub fn drain() -> Vec<HostPhysicsMotiveEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
