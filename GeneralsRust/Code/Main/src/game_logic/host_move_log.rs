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
    let v = LOG.with(|log| std::mem::take(&mut *log.borrow_mut()));
    // Keep last non-empty batch for PresentationFrame after shadow session.
    if !v.is_empty() {
        LAST_DRAIN.with(|last| *last.borrow_mut() = v.clone());
    }
    v
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
    LAST_DRAIN.with(|last| last.borrow_mut().clear());
}

/// Take events from the most recent non-empty `drain()` (PresentationFrame sole consumer).
pub fn take_last_drain() -> Vec<HostMoveEvent> {
    LAST_DRAIN.with(|last| std::mem::take(&mut *last.borrow_mut()))
}

/// Non-destructive peek (tests).
pub fn last_drain_snapshot() -> Vec<HostMoveEvent> {
    LAST_DRAIN.with(|last| last.borrow().clone())
}
