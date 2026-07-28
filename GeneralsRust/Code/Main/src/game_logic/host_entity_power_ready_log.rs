//! Wave 674: GameWorld entity power writeback ready residual log.
//!
//! When `writeback_entity_power_to_host` changes fields, it records here.
//! Host drains and applies presentation bookkeeping so GameWorld owns the
//! entity power last-write while host owns residual side effects.
//!
//! Fail-closed: empty drain is valid (no entity power changes this frame).

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostEntityPowerReadyEvent {
    pub object: ObjectId,
}

thread_local! {
    static LOG: RefCell<Vec<HostEntityPowerReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostEntityPowerReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId) {
    LOG.with(|log| {
        log.borrow_mut().push(HostEntityPowerReadyEvent { object });
    });
}

pub fn drain() -> Vec<HostEntityPowerReadyEvent> {
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
        record(ObjectId(675));
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].object.0, 675);
        assert!(drain().is_empty());
        clear();
    }
}
