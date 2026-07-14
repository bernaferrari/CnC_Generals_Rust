//! Frame-local host ownership/team transfer log for GameWorld shadow parity.
//!
//! Capture, hijack, snipe-unmanned, car-bomb convert, and other `set_team` writes
//! feed `WorldMutation::TransferOwner` on the shadow session.

use super::{ObjectId, Team};
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostOwnerEvent {
    pub object: ObjectId,
    pub team: Team,
}

thread_local! {
    static LOG: RefCell<Vec<HostOwnerEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostOwnerEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, team: Team) {
    LOG.with(|log| {
        log.borrow_mut().push(HostOwnerEvent { object, team });
    });
}

pub fn drain() -> Vec<HostOwnerEvent> {
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
pub fn take_last_drain() -> Vec<HostOwnerEvent> {
    LAST_DRAIN.with(|last| std::mem::take(&mut *last.borrow_mut()))
}

/// Non-destructive peek (tests).
pub fn last_drain_snapshot() -> Vec<HostOwnerEvent> {
    LAST_DRAIN.with(|last| last.borrow().clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_drain() {
        clear();
        record(ObjectId(1), Team::USA);
        assert_eq!(drain().len(), 1);
        assert!(drain().is_empty());
        assert_eq!(last_drain_snapshot().len(), 1);
    }
}
