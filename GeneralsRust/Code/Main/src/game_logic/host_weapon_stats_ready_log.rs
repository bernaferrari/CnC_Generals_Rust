//! Wave 635: GameWorld weapon-stats writeback ready residual log.
//!
//! When `writeback_weapon_stats_to_host` changes weapon stats, it records here.
//! Host drains and applies presentation bookkeeping via record_host_weapon_stats
//! so GameWorld owns the weapon-stats last-write while host owns presentation
//! residual.
//!
//! Fail-closed: empty drain is valid (no weapon-stats changes this frame).

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostWeaponStatsReadyEvent {
    pub object: ObjectId,
}

thread_local! {
    static LOG: RefCell<Vec<HostWeaponStatsReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostWeaponStatsReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId) {
    LOG.with(|log| {
        log.borrow_mut().push(HostWeaponStatsReadyEvent { object });
    });
}

pub fn drain() -> Vec<HostWeaponStatsReadyEvent> {
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
        record(ObjectId(17));
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].object.0, 17);
        assert!(drain().is_empty());
        clear();
    }
}
