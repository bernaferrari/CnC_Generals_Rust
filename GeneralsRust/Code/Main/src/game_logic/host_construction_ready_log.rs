//! Wave 617: GameWorld construction-ready residual log.
//!
//! Under CONSTRUCTION_AUTHORITY sole-tick, `writeback_construction_to_host` records
//! structures whose construction_percent reached 1.0 while still under_construction.
//! Host `update_construction` drains this log so GameWorld decides readiness;
//! host still applies completion side effects (HP, model conditions, EVA).
//!
//! Fail-closed: empty drain is valid (no completions this frame).

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct HostConstructionReadyEvent {
    pub structure: ObjectId,
    pub percent: f32,
}

thread_local! {
    static LOG: RefCell<Vec<HostConstructionReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostConstructionReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(structure: ObjectId, percent: f32) {
    LOG.with(|log| {
        log.borrow_mut()
            .push(HostConstructionReadyEvent { structure, percent });
    });
}

pub fn drain() -> Vec<HostConstructionReadyEvent> {
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
        record(ObjectId(3), 1.0);
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].structure.0, 3);
        assert!(drain().is_empty());
        clear();
    }
}
