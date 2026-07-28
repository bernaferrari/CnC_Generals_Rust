//! Wave 655: GameWorld selection radius writeback ready residual log.
//!
//! When `writeback_selection_radius_to_host` changes fields, it records here.
//! Host drains and applies presentation bookkeeping so GameWorld owns the
//! selection radius last-write while host owns residual side effects.
//!
//! Fail-closed: empty drain is valid (no selection radius changes this frame).

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostSelectionRadiusReadyEvent {
    pub object: ObjectId,
}

thread_local! {
    static LOG: RefCell<Vec<HostSelectionRadiusReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostSelectionRadiusReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId) {
    LOG.with(|log| {
        log.borrow_mut()
            .push(HostSelectionRadiusReadyEvent { object });
    });
}

pub fn drain() -> Vec<HostSelectionRadiusReadyEvent> {
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
        record(ObjectId(655));
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].object.0, 655);
        assert!(drain().is_empty());
        clear();
    }
}
