//! Frame-local host entity power log for GameWorld SetEntityPower parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostEntityPowerEvent {
    pub object: ObjectId,
    pub power_provided: i32,
    pub power_consumed: i32,
}

thread_local! {
    static LOG: RefCell<Vec<HostEntityPowerEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, power_provided: i32, power_consumed: i32) {
    LOG.with(|log| {
        log.borrow_mut().push(HostEntityPowerEvent {
            object,
            power_provided,
            power_consumed,
        });
    });
}

pub fn drain() -> Vec<HostEntityPowerEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
