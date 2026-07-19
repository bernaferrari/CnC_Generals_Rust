//! Frame-local host hive-slave log for GameWorld SetHiveSlaves parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostHiveEvent {
    pub object: ObjectId,
    pub slave_count: u8,
    pub slave_hp: f32,
}

thread_local! {
    static LOG: RefCell<Vec<HostHiveEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, slave_count: u8, slave_hp: f32) {
    LOG.with(|log| {
        log.borrow_mut().push(HostHiveEvent {
            object,
            slave_count,
            slave_hp: slave_hp.max(0.0),
        });
    });
}

pub fn drain() -> Vec<HostHiveEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
