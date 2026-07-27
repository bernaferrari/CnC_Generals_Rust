//! Wave 643: GameWorld combat-attack writeback ready residual log.
//!
//! When `writeback_combat_attack_to_host` changes attack-substate fields, it
//! records here. Host drains and applies presentation bookkeeping via
//! record_host_combat_attack so GameWorld owns the combat-attack last-write
//! while host owns residual side effects.
//!
//! Fail-closed: empty drain is valid (no combat-attack changes this frame).

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostCombatAttackReadyEvent {
    pub object: ObjectId,
}

thread_local! {
    static LOG: RefCell<Vec<HostCombatAttackReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostCombatAttackReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId) {
    LOG.with(|log| {
        log.borrow_mut().push(HostCombatAttackReadyEvent { object });
    });
}

pub fn drain() -> Vec<HostCombatAttackReadyEvent> {
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
        record(ObjectId(19));
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].object.0, 19);
        assert!(drain().is_empty());
        clear();
    }
}
