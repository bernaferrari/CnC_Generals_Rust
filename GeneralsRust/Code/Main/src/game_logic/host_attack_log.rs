//! Frame-local host attack-target log for GameWorld shadow parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostAttackEvent {
    pub attacker: ObjectId,
    pub target: Option<ObjectId>,
}

thread_local! {
    static LOG: RefCell<Vec<HostAttackEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostAttackEvent>> = RefCell::new(Vec::new());
}

pub fn record(attacker: ObjectId, target: Option<ObjectId>) {
    LOG.with(|log| {
        log.borrow_mut().push(HostAttackEvent { attacker, target });
    });
}

pub fn drain() -> Vec<HostAttackEvent> {
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
pub fn take_last_drain() -> Vec<HostAttackEvent> {
    LAST_DRAIN.with(|last| std::mem::take(&mut *last.borrow_mut()))
}

/// Non-destructive peek (tests).
pub fn last_drain_snapshot() -> Vec<HostAttackEvent> {
    LAST_DRAIN.with(|last| last.borrow().clone())
}
