//! Wave 625: GameWorld radar-extend complete ready residual log.
//!
//! When `writeback_radar_extend_to_host` observes a flip to
//! `radar_extend_complete`, it records here. Host drains to apply upgraded
//! model-condition residual and count completes so GameWorld owns the complete
//! flag while host owns visual/counter side effects.
//!
//! Fail-closed: empty drain is valid (no radar completes this frame).

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct HostRadarExtendReadyEvent {
    pub structure: ObjectId,
}

thread_local! {
    static LOG: RefCell<Vec<HostRadarExtendReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostRadarExtendReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(structure: ObjectId) {
    LOG.with(|log| {
        log.borrow_mut()
            .push(HostRadarExtendReadyEvent { structure });
    });
}

pub fn drain() -> Vec<HostRadarExtendReadyEvent> {
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
        record(ObjectId(13));
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].structure.0, 13);
        assert!(drain().is_empty());
        clear();
    }
}
