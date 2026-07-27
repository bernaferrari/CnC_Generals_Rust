//! Wave 644: GameWorld command-set writeback ready residual log.
//!
//! When `writeback_command_set_to_host` changes command-set override, it records
//! here. Host drains and applies presentation bookkeeping via
//! record_host_command_set so GameWorld owns the command-set last-write while
//! host owns residual side effects.
//!
//! Fail-closed: empty drain is valid (no command-set changes this frame).

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostCommandSetReadyEvent {
    pub object: ObjectId,
}

thread_local! {
    static LOG: RefCell<Vec<HostCommandSetReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostCommandSetReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId) {
    LOG.with(|log| {
        log.borrow_mut().push(HostCommandSetReadyEvent { object });
    });
}

pub fn drain() -> Vec<HostCommandSetReadyEvent> {
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
        record(ObjectId(7));
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].object.0, 7);
        assert!(drain().is_empty());
        clear();
    }
}
