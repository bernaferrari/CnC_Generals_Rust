//! Wave 634: GameWorld combat-status writeback ready residual log.
//!
//! When `writeback_combat_status_to_host` changes object status flags, it
//! records here. Host drains and applies presentation bookkeeping via
//! host_status_log so GameWorld owns the combat-status last-write while host
//! owns status presentation residual.
//!
//! Fail-closed: empty drain is valid (no combat-status changes this frame).

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostCombatStatusReadyEvent {
    pub object: ObjectId,
}

thread_local! {
    static LOG: RefCell<Vec<HostCombatStatusReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostCombatStatusReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId) {
    LOG.with(|log| {
        log.borrow_mut().push(HostCombatStatusReadyEvent { object });
    });
}

pub fn drain() -> Vec<HostCombatStatusReadyEvent> {
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
        record(ObjectId(11));
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].object.0, 11);
        assert!(drain().is_empty());
        clear();
    }
}
