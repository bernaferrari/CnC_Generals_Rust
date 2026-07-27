//! Wave 641: GameWorld stored-supplies writeback ready residual log.
//!
//! When `writeback_stored_supplies_to_host` changes unit-carried supplies, it
//! records here. Host drains and applies presentation bookkeeping so GameWorld
//! owns the stored-supplies last-write while host owns residual side effects.
//!
//! Fail-closed: empty drain is valid (no stored-supplies changes this frame).

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostStoredSuppliesReadyEvent {
    pub object: ObjectId,
    pub previous_supplies: u32,
    pub new_supplies: u32,
}

thread_local! {
    static LOG: RefCell<Vec<HostStoredSuppliesReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostStoredSuppliesReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, previous_supplies: u32, new_supplies: u32) {
    LOG.with(|log| {
        log.borrow_mut().push(HostStoredSuppliesReadyEvent {
            object,
            previous_supplies,
            new_supplies,
        });
    });
}

pub fn drain() -> Vec<HostStoredSuppliesReadyEvent> {
    LOG.with(|log| {
        let events = std::mem::take(&mut *log.borrow_mut());
        LAST_DRAIN.with(|last| *last.borrow_mut() = events.clone());
        events
    })
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
    LAST_DRAIN.with(|last| last.borrow_mut().clear());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_drain_roundtrip() {
        clear();
        record(ObjectId(3), 100, 250);
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].object.0, 3);
        assert_eq!(d[0].previous_supplies, 100);
        assert_eq!(d[0].new_supplies, 250);
        assert!(drain().is_empty());
        clear();
    }
}
