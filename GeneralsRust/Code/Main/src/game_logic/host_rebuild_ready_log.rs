//! Wave 620: GameWorld rebuild-hole ready residual log.
//!
//! Under CONSTRUCTION_AUTHORITY sole-tick, `writeback_rebuild_producer_to_host`
//! records rebuild holes whose ready frame has been reached and that are not
//! already reconstructing. Host `update_rebuild_holes` drains this log so
//! GameWorld decides readiness; host still spawns worker + reconstructing
//! structure (ObjectId authority).
//!
//! Fail-closed: empty drain is valid (no rebuild starts this frame).

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct HostRebuildReadyEvent {
    pub hole: ObjectId,
    pub ready_frame: u32,
}

thread_local! {
    static LOG: RefCell<Vec<HostRebuildReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostRebuildReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(hole: ObjectId, ready_frame: u32) {
    LOG.with(|log| {
        log.borrow_mut()
            .push(HostRebuildReadyEvent { hole, ready_frame });
    });
}

pub fn drain() -> Vec<HostRebuildReadyEvent> {
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
        record(ObjectId(11), 90);
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].hole.0, 11);
        assert_eq!(d[0].ready_frame, 90);
        assert!(drain().is_empty());
        clear();
    }
}
