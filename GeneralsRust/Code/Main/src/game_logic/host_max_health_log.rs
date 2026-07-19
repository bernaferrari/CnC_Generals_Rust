//! Frame-local host max-health log for GameWorld SetMaxHealth parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostMaxHealthEvent {
    pub object: ObjectId,
    pub max_health: f32,
}

thread_local! {
    static LOG: RefCell<Vec<HostMaxHealthEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, max_health: f32) {
    LOG.with(|log| {
        log.borrow_mut().push(HostMaxHealthEvent {
            object,
            max_health: max_health.max(1.0),
        });
    });
}

pub fn drain() -> Vec<HostMaxHealthEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
