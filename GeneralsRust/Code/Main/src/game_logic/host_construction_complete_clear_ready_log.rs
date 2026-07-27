//! Wave 626: GameWorld construction-complete-clear ready residual log.
//!
//! Under CONSTRUCTION_AUTHORITY sole-tick, `writeback_rebuild_producer_to_host`
//! records producers whose CONSTRUCTION_COMPLETE clear deadline has elapsed.
//! Host drains and clears the model bit so GameWorld owns the deadline while
//! host owns visual residual.
//!
//! Fail-closed: empty drain is valid (no clears this frame).

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct HostConstructionCompleteClearReadyEvent {
    pub producer: ObjectId,
    pub clear_frame: u32,
}

thread_local! {
    static LOG: RefCell<Vec<HostConstructionCompleteClearReadyEvent>> =
        RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostConstructionCompleteClearReadyEvent>> =
        RefCell::new(Vec::new());
}

pub fn record(producer: ObjectId, clear_frame: u32) {
    LOG.with(|log| {
        log.borrow_mut()
            .push(HostConstructionCompleteClearReadyEvent {
                producer,
                clear_frame,
            });
    });
}

pub fn drain() -> Vec<HostConstructionCompleteClearReadyEvent> {
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
        record(ObjectId(21), 90);
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].producer.0, 21);
        assert_eq!(d[0].clear_frame, 90);
        assert!(drain().is_empty());
        clear();
    }
}
