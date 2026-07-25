//! Frame-local host rally-point log for GameWorld SetRallyPoint parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostRallyEvent {
    pub object: ObjectId,
    /// None clears the structure rally point.
    pub position: Option<[f32; 3]>,
}

thread_local! {
    static LOG: RefCell<Vec<HostRallyEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, position: Option<[f32; 3]>) {
    LOG.with(|log| {
        log.borrow_mut().push(HostRallyEvent { object, position });
    });
}

pub fn drain() -> Vec<HostRallyEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}

pub fn snapshot() -> Vec<HostRallyEvent> {
    LOG.with(|log| log.borrow().clone())
}
