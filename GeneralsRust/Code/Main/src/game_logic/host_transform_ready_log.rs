//! Wave 636: GameWorld transform writeback ready residual log.
//!
//! When `writeback_transforms_to_host` changes position/orientation, it records
//! here. Host drains and applies movement/presentation bookkeeping so GameWorld
//! owns the transform last-write while host owns drawable residual.
//!
//! Fail-closed: empty drain is valid (no transform changes this frame).

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostTransformReadyEvent {
    pub object: ObjectId,
}

thread_local! {
    static LOG: RefCell<Vec<HostTransformReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostTransformReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId) {
    LOG.with(|log| {
        log.borrow_mut().push(HostTransformReadyEvent { object });
    });
}

pub fn drain() -> Vec<HostTransformReadyEvent> {
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
        record(ObjectId(21));
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].object.0, 21);
        assert!(drain().is_empty());
        clear();
    }
}
