//! Frame-local host fire-intent log for GameWorld SetFireIntent parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostFireIntentEvent {
    pub object: ObjectId,
    pub last_fire_victim_host: u32,
    pub last_fire_slot: u8,
    pub last_fire_damage: f32,
    pub last_fire_range: f32,
    pub last_fire_sim_time: f32,
    pub last_fire_frame: u32,
    pub fire_intent_count: u32,
}

thread_local! {
    static LOG: RefCell<Vec<HostFireIntentEvent>> = RefCell::new(Vec::new());
}

pub fn record(
    object: ObjectId,
    last_fire_victim_host: u32,
    last_fire_slot: u8,
    last_fire_damage: f32,
    last_fire_range: f32,
    last_fire_sim_time: f32,
    last_fire_frame: u32,
    fire_intent_count: u32,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostFireIntentEvent {
            object,
            last_fire_victim_host,
            last_fire_slot,
            last_fire_damage,
            last_fire_range,
            last_fire_sim_time,
            last_fire_frame,
            fire_intent_count,
        });
    });
}

pub fn drain() -> Vec<HostFireIntentEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
