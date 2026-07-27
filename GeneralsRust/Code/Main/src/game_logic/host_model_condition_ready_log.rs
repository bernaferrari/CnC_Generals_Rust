//! Wave 633: GameWorld model-condition writeback ready residual log.
//!
//! When `writeback_model_condition_to_host` changes an object's model condition
//! bits, it records here. Host drains and applies presentation bookkeeping so
//! GameWorld owns the model-condition last-write while host owns drawable/
//! presentation residual.
//!
//! Fail-closed: empty drain is valid (no model-condition changes this frame).

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostModelConditionReadyEvent {
    pub object: ObjectId,
    pub previous_bits: u128,
    pub new_bits: u128,
}

thread_local! {
    static LOG: RefCell<Vec<HostModelConditionReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostModelConditionReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, previous_bits: u128, new_bits: u128) {
    LOG.with(|log| {
        log.borrow_mut().push(HostModelConditionReadyEvent {
            object,
            previous_bits,
            new_bits,
        });
    });
}

pub fn drain() -> Vec<HostModelConditionReadyEvent> {
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
        record(ObjectId(4), 0, 1u128 << 3);
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].object.0, 4);
        assert_eq!(d[0].previous_bits, 0);
        assert_eq!(d[0].new_bits, 1u128 << 3);
        assert!(drain().is_empty());
        clear();
    }
}
