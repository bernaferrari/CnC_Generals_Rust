//! Wave 619: GameWorld sell-finish ready residual log.
//!
//! Under CONSTRUCTION_AUTHORITY sole-tick, sell deconstruction percent is
//! advanced in GameWorld (negative construction_percent). When writeback
//! observes a sold structure at or below SELL_FINISH (-0.5), it records here.
//! Host `update_sell_list` drains this log so GameWorld decides sell readiness;
//! host still applies refund/destroy/radar side effects.
//!
//! Fail-closed: empty drain is valid (no sell finishes this frame).

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct HostSellReadyEvent {
    pub structure: ObjectId,
    pub percent: f32,
}

thread_local! {
    static LOG: RefCell<Vec<HostSellReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostSellReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(structure: ObjectId, percent: f32) {
    LOG.with(|log| {
        log.borrow_mut()
            .push(HostSellReadyEvent { structure, percent });
    });
}

pub fn drain() -> Vec<HostSellReadyEvent> {
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
        record(ObjectId(7), -0.5);
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].structure.0, 7);
        assert!((d[0].percent + 0.5).abs() < 1e-6);
        assert!(drain().is_empty());
        clear();
    }
}
