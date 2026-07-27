//! Wave 637: GameWorld movement writeback ready residual log.
//!
//! When `writeback_movement_to_host` changes velocity/path/flags, it records
//! here. Host drains and applies movement presentation bookkeeping so GameWorld
//! owns the movement last-write while host owns path residual.
//!
//! Fail-closed: empty drain is valid (no movement changes this frame).

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostMovementReadyEvent {
    pub object: ObjectId,
}

thread_local! {
    static LOG: RefCell<Vec<HostMovementReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostMovementReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId) {
    LOG.with(|log| {
        log.borrow_mut().push(HostMovementReadyEvent { object });
    });
}

pub fn drain() -> Vec<HostMovementReadyEvent> {
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
        record(ObjectId(33));
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].object.0, 33);
        assert!(drain().is_empty());
        clear();
    }
}
