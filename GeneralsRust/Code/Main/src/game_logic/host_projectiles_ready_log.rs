//! Wave 678: GameWorld projectiles writeback ready residual log.
//!
//! When `writeback_projectiles_to_host` changes combat projectiles, it records here.
//! Host drains and applies presentation bookkeeping so GameWorld owns the
//! projectiles last-write while host owns residual side effects.
//!
//! Fail-closed: empty drain is valid (no projectiles changes this frame).

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostProjectilesReadyEvent {
    pub object: ObjectId,
    /// true when host combat projectile was removed because GW no longer has it
    pub removed: bool,
}

thread_local! {
    static LOG: RefCell<Vec<HostProjectilesReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostProjectilesReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, removed: bool) {
    LOG.with(|log| {
        log.borrow_mut()
            .push(HostProjectilesReadyEvent { object, removed });
    });
}

pub fn drain() -> Vec<HostProjectilesReadyEvent> {
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
        record(ObjectId(678), false);
        record(ObjectId(678 + 1), true);
        let d = drain();
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].object.0, 678);
        assert!(!d[0].removed);
        assert!(d[1].removed);
        assert!(drain().is_empty());
        clear();
    }
}
