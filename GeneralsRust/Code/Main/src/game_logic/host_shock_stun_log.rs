//! Frame-local host shock/stun residual for GameWorld SetShockStun parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostShockStunEvent {
    pub object: ObjectId,
    pub shock_stun_frames: u32,
    pub shock_yaw_rate: f32,
    pub shock_pitch_rate: f32,
    pub shock_roll_rate: f32,
    pub shock_up_z: f32,
    pub shock_allow_bounce: bool,
    pub shock_grounded_once: bool,
    pub shock_was_airborne: bool,
    pub cell_is_cliff: bool,
    pub cell_is_underwater: bool,
}

thread_local! {
    static LOG: RefCell<Vec<HostShockStunEvent>> = RefCell::new(Vec::new());
}

pub fn record(
    object: ObjectId,
    shock_stun_frames: u32,
    shock_yaw_rate: f32,
    shock_pitch_rate: f32,
    shock_roll_rate: f32,
    shock_up_z: f32,
    shock_allow_bounce: bool,
    shock_grounded_once: bool,
    shock_was_airborne: bool,
    cell_is_cliff: bool,
    cell_is_underwater: bool,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostShockStunEvent {
            object,
            shock_stun_frames,
            shock_yaw_rate,
            shock_pitch_rate,
            shock_roll_rate,
            shock_up_z,
            shock_allow_bounce,
            shock_grounded_once,
            shock_was_airborne,
            cell_is_cliff,
            cell_is_underwater,
        });
    });
}

pub fn drain() -> Vec<HostShockStunEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
