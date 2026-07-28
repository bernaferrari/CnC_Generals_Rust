//! Wave 675: GameWorld building type writeback ready residual log.
//!
//! When `writeback_building_type_to_host` changes fields, it records here.
//! Host drains and applies presentation bookkeeping so GameWorld owns the
//! building type last-write while host owns residual side effects.
//!
//! Fail-closed: empty drain is valid (no building type changes this frame).

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostBuildingTypeReadyEvent {
    pub object: ObjectId,
}

thread_local! {
    static LOG: RefCell<Vec<HostBuildingTypeReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostBuildingTypeReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId) {
    LOG.with(|log| {
        log.borrow_mut().push(HostBuildingTypeReadyEvent { object });
    });
}

pub fn drain() -> Vec<HostBuildingTypeReadyEvent> {
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
        record(ObjectId(664));
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].object.0, 664);
        assert!(drain().is_empty());
        clear();
    }
}
