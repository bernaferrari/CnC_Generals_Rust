//! Frame-local host bounce/land residual for GameWorld SetBounceLand parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, PartialEq)]
pub struct HostBounceLandEvent {
    pub object: ObjectId,
    pub kill_when_resting_on_ground: bool,
    pub bounce_land_events: u32,
    pub last_bounce_fall_dy: f32,
    pub bounce_sound_name: String,
    pub last_bounce_volume: f32,
    pub bounce_audio_pending: u32,
    pub allow_collide_force: bool,
    pub last_collidee_id: Option<u32>,
    pub ignore_collisions_with_id: Option<u32>,
}

thread_local! {
    static LOG: RefCell<Vec<HostBounceLandEvent>> = RefCell::new(Vec::new());
}

pub fn record(
    object: ObjectId,
    kill_when_resting_on_ground: bool,
    bounce_land_events: u32,
    last_bounce_fall_dy: f32,
    bounce_sound_name: String,
    last_bounce_volume: f32,
    bounce_audio_pending: u32,
    allow_collide_force: bool,
    last_collidee_id: Option<u32>,
    ignore_collisions_with_id: Option<u32>,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostBounceLandEvent {
            object,
            kill_when_resting_on_ground,
            bounce_land_events,
            last_bounce_fall_dy,
            bounce_sound_name,
            last_bounce_volume,
            bounce_audio_pending,
            allow_collide_force,
            last_collidee_id,
            ignore_collisions_with_id,
        });
    });
}

pub fn drain() -> Vec<HostBounceLandEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
