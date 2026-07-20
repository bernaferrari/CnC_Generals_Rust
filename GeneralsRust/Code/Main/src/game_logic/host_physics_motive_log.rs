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
    pub aerodynamic_friction: f32,
    pub extra_friction: f32,
    pub apply_friction_2d_when_airborne: bool,
    pub center_of_mass_offset: f32,
    pub pitch_roll_yaw_factor: f32,
    pub move_away_destination: Option<[f32; 3]>,
    pub request_other_move_away_id: Option<u32>,
    pub immune_to_falling_damage: bool,
    pub physics_current_overlap_id: Option<u32>,
    pub physics_previous_overlap_id: Option<u32>,
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
    aerodynamic_friction: f32,
    extra_friction: f32,
    apply_friction_2d_when_airborne: bool,
    center_of_mass_offset: f32,
    pitch_roll_yaw_factor: f32,
    move_away_destination: Option<[f32; 3]>,
    request_other_move_away_id: Option<u32>,
    immune_to_falling_damage: bool,
    physics_current_overlap_id: Option<u32>,
    physics_previous_overlap_id: Option<u32>,
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
            aerodynamic_friction,
            extra_friction,
            apply_friction_2d_when_airborne,
            center_of_mass_offset,
            pitch_roll_yaw_factor,
            move_away_destination,
            request_other_move_away_id,
            immune_to_falling_damage,
            physics_current_overlap_id,
            physics_previous_overlap_id,
        });
    });
}

pub fn drain() -> Vec<HostPhysicsMotiveEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
