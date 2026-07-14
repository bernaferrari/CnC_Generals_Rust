//! Frame-local host move-destination log for GameWorld shadow parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostMoveEvent {
    pub unit: ObjectId,
    /// None clears the move target (stop).
    pub destination: Option<[f32; 3]>,
}

thread_local! {
    static LOG: RefCell<Vec<HostMoveEvent>> = RefCell::new(Vec::new());
}

pub fn record(unit: ObjectId, destination: Option<[f32; 3]>) {
    LOG.with(|log| {
        log.borrow_mut().push(HostMoveEvent { unit, destination });
    });
}

pub fn drain() -> Vec<HostMoveEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
