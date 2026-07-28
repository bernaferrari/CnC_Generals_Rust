//! Frame-local Stinger hive respawn logs for GW shadow parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct StingerHiveRespawnEvent {
    pub id: ObjectId,
    pub hive_slave_count: u8,
    pub hive_slave_hp: f32,
    pub hive_slave_respawn_frame: u32,
    pub slaves_alive: [bool; 3],
    pub slaves_hp: [f32; 3],
}

thread_local! {
    static EVENTS: RefCell<Vec<StingerHiveRespawnEvent>> = RefCell::new(Vec::new());
}

pub fn record(ev: StingerHiveRespawnEvent) {
    EVENTS.with(|l| l.borrow_mut().push(ev));
}

pub fn drain() -> Vec<StingerHiveRespawnEvent> {
    EVENTS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn clear() {
    EVENTS.with(|l| l.borrow_mut().clear());
}
