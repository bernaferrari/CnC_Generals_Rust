//! Frame-local host contain capacity log for GameWorld SetContainCapacity parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostContainCapacityEvent {
    pub object: ObjectId,
    pub max_transport: usize,
    pub max_garrison: u16,
}

thread_local! {
    static LOG: RefCell<Vec<HostContainCapacityEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, max_transport: usize, max_garrison: u16) {
    LOG.with(|log| {
        log.borrow_mut().push(HostContainCapacityEvent {
            object,
            max_transport,
            max_garrison,
        });
    });
}

pub fn drain() -> Vec<HostContainCapacityEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
