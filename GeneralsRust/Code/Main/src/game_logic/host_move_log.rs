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
    static LAST_DRAIN: RefCell<Vec<HostMoveEvent>> = RefCell::new(Vec::new());
}

pub fn record(unit: ObjectId, destination: Option<[f32; 3]>) {
    LOG.with(|log| {
        log.borrow_mut().push(HostMoveEvent { unit, destination });
    });
}

pub fn drain() -> Vec<HostMoveEvent> {
    LOG.with(|log| {
        let v = std::mem::take(&mut *log.borrow_mut());
        LAST_DRAIN.with(|last| *last.borrow_mut() = v.clone());
        v
    })
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
    LAST_DRAIN.with(|last| last.borrow_mut().clear());
}

/// Events from the most recent `drain()` (PresentationFrame after shadow session).
pub fn last_drain_snapshot() -> Vec<HostMoveEvent> {
    LAST_DRAIN.with(|last| last.borrow().clone())
}
