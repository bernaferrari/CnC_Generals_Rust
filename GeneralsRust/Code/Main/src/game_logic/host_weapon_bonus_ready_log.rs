//! Wave 658: GameWorld weapon bonus writeback ready residual log.
//!
//! When `writeback_weapon_bonus_to_host` changes fields, it records here.
//! Host drains and applies presentation bookkeeping so GameWorld owns the
//! weapon bonus last-write while host owns residual side effects.
//!
//! Fail-closed: empty drain is valid (no weapon bonus changes this frame).

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostWeaponBonusReadyEvent {
    pub object: ObjectId,
}

thread_local! {
    static LOG: RefCell<Vec<HostWeaponBonusReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostWeaponBonusReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId) {
    LOG.with(|log| {
        log.borrow_mut().push(HostWeaponBonusReadyEvent { object });
    });
}

pub fn drain() -> Vec<HostWeaponBonusReadyEvent> {
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
        record(ObjectId(658));
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].object.0, 658);
        assert!(drain().is_empty());
        clear();
    }
}
