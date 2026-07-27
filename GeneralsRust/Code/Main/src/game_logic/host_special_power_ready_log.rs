//! Wave 618: GameWorld special-power ready residual log.
//!
//! Under SPECIAL_POWER_AUTHORITY sole-tick, `writeback_special_power_to_host`
//! records objects whose SP countdown reached ready. Host `tick_timers` / EVA
//! paths can drain this log so GameWorld decides readiness flips.
//!
//! Fail-closed: empty drain is valid (no ready flips this frame).

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct HostSpecialPowerReadyEvent {
    pub object: ObjectId,
    pub cooldown_remaining: f32,
}

thread_local! {
    static LOG: RefCell<Vec<HostSpecialPowerReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostSpecialPowerReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, cooldown_remaining: f32) {
    LOG.with(|log| {
        log.borrow_mut().push(HostSpecialPowerReadyEvent {
            object,
            cooldown_remaining,
        });
    });
}

pub fn drain() -> Vec<HostSpecialPowerReadyEvent> {
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
        record(ObjectId(9), 0.0);
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].object.0, 9);
        assert!(drain().is_empty());
        clear();
    }
}
