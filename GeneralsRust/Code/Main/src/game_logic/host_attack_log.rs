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
pub fn last_drain_snapshot() -> Vec<HostAttackEvent> {
    LAST_DRAIN.with(|last| last.borrow().clone())
}
