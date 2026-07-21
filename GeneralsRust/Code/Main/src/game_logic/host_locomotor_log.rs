//! Frame-local host locomotor residual for GameWorld SetLocomotor parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostLocomotorEvent {
    pub object: ObjectId,
    pub is_approach_path: bool,
    pub on_invalid_movement_terrain: bool,
    pub was_airborne_last_frame: bool,
    pub can_move_backward: bool,
    pub moving_backwards: bool,
    pub no_slow_down_as_approaching_dest: bool,
    pub turn_pivot_offset: f32,
    pub wander_width_factor: f32,
    pub loco_apply_2d_friction_airborne: bool,
    pub loco_extra_2d_friction: f32,
    pub loco_preferred_height: f32,
    pub loco_preferred_height_damping: f32,
    pub loco_appearance_ordinal: u8,
    pub loco_behavior_z_ordinal: u8,
    pub min_turn_speed: f32,
    pub physics_turning_ordinal: i8,
}

thread_local! {
    static LOG: RefCell<Vec<HostLocomotorEvent>> = RefCell::new(Vec::new());
}

pub fn record(
    object: ObjectId,
    is_approach_path: bool,
    on_invalid_movement_terrain: bool,
    was_airborne_last_frame: bool,
    can_move_backward: bool,
    moving_backwards: bool,
    no_slow_down_as_approaching_dest: bool,
    turn_pivot_offset: f32,
    wander_width_factor: f32,
    loco_apply_2d_friction_airborne: bool,
    loco_extra_2d_friction: f32,
    loco_preferred_height: f32,
    loco_preferred_height_damping: f32,
    loco_appearance_ordinal: u8,
    loco_behavior_z_ordinal: u8,
    min_turn_speed: f32,
    physics_turning_ordinal: i8,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostLocomotorEvent {
            object,
            is_approach_path,
            on_invalid_movement_terrain,
            was_airborne_last_frame,
            can_move_backward,
            moving_backwards,
            no_slow_down_as_approaching_dest,
            turn_pivot_offset,
            wander_width_factor,
            loco_apply_2d_friction_airborne,
            loco_extra_2d_friction,
            loco_preferred_height,
            loco_preferred_height_damping,
            loco_appearance_ordinal,
            loco_behavior_z_ordinal,
            min_turn_speed,
            physics_turning_ordinal,
        });
    });
}

pub fn drain() -> Vec<HostLocomotorEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
