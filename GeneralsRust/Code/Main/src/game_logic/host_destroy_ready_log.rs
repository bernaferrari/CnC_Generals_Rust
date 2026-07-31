//! Wave 621: GameWorld destroy-ready residual log.
//!
//! Under DAMAGE_AUTHORITY, `writeback_health_to_host` records objects whose
//! HP last-write is lethal (health <= 0 / destroyed). Host `process_destroy_list`
//! drains this log and marks destruction so die-module side effects still run
//! on the host ObjectId path.
//!
//! Fail-closed: empty drain is valid (no lethal writebacks this frame).

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct HostDestroyReadyEvent {
    pub object: ObjectId,
    pub health: f32,
}

thread_local! {
    static LOG: RefCell<Vec<HostDestroyReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostDestroyReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, health: f32) {
    LOG.with(|log| {
        log.borrow_mut()
            .push(HostDestroyReadyEvent { object, health });
    });
}

pub fn drain() -> Vec<HostDestroyReadyEvent> {
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

/// Wave 912: residual probe — true when host destroy-ready log has events.
#[inline]
pub fn has_pending() -> bool {
    LOG.with(|log| !log.borrow().is_empty())
}

/// Wave 912: residual probe — pending destroy-ready event count.
#[inline]
pub fn pending_count() -> usize {
    LOG.with(|log| log.borrow().len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_drain_roundtrip() {
        clear();
        record(ObjectId(42), 0.0);
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].object.0, 42);
        assert!(d[0].health <= 0.0);
        assert!(drain().is_empty());
        clear();
    }
}
