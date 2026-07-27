//! Wave 630: GameWorld AI-state change ready residual log.
//!
//! When `writeback_ai_state_to_host` changes an object's AI state ordinal, it
//! records here. Host drains and applies combat-status residual (moving/
//! attacking flags) so GameWorld owns the AI state last-write while host owns
//! status bookkeeping.
//!
//! Fail-closed: empty drain is valid (no AI state changes this frame).

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct HostAiStateReadyEvent {
    pub object: ObjectId,
    pub previous_ordinal: u8,
    pub new_ordinal: u8,
}

thread_local! {
    static LOG: RefCell<Vec<HostAiStateReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostAiStateReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, previous_ordinal: u8, new_ordinal: u8) {
    LOG.with(|log| {
        log.borrow_mut().push(HostAiStateReadyEvent {
            object,
            previous_ordinal,
            new_ordinal,
        });
    });
}

pub fn drain() -> Vec<HostAiStateReadyEvent> {
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
        record(ObjectId(6), 0, 2);
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].object.0, 6);
        assert_eq!(d[0].previous_ordinal, 0);
        assert_eq!(d[0].new_ordinal, 2);
        assert!(drain().is_empty());
        clear();
    }
}
