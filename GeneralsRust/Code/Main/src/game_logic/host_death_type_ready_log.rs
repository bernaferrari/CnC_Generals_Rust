//! Wave 632: GameWorld death-type writeback ready residual log.
//!
//! When `writeback_death_type_to_host` changes an object's death type, it
//! records here. Host drains and applies death-type bookkeeping residual so
//! GameWorld owns the death-type last-write while host owns destroy/pilot
//! presentation bookkeeping.
//!
//! Fail-closed: empty drain is valid (no death-type changes this frame).

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostDeathTypeReadyEvent {
    pub object: ObjectId,
    pub previous_ordinal: u8,
    pub new_ordinal: u8,
}

thread_local! {
    static LOG: RefCell<Vec<HostDeathTypeReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostDeathTypeReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, previous_ordinal: u8, new_ordinal: u8) {
    LOG.with(|log| {
        log.borrow_mut().push(HostDeathTypeReadyEvent {
            object,
            previous_ordinal,
            new_ordinal,
        });
    });
}

pub fn drain() -> Vec<HostDeathTypeReadyEvent> {
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
        record(ObjectId(9), 0, 3);
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].object.0, 9);
        assert_eq!(d[0].previous_ordinal, 0);
        assert_eq!(d[0].new_ordinal, 3);
        assert!(drain().is_empty());
        clear();
    }
}
