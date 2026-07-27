//! Wave 638: GameWorld attack-target writeback ready residual log.
//!
//! When `writeback_attack_targets_to_host` changes an object's attack target, it
//! records here. Host drains and applies attack residual (AI/status/attack log)
//! so GameWorld owns the target last-write while host owns residual side effects.
//!
//! Fail-closed: empty drain is valid (no attack-target changes this frame).

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostAttackTargetReadyEvent {
    pub object: ObjectId,
    pub previous_target: Option<ObjectId>,
    pub new_target: Option<ObjectId>,
}

thread_local! {
    static LOG: RefCell<Vec<HostAttackTargetReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostAttackTargetReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, previous_target: Option<ObjectId>, new_target: Option<ObjectId>) {
    LOG.with(|log| {
        log.borrow_mut().push(HostAttackTargetReadyEvent {
            object,
            previous_target,
            new_target,
        });
    });
}

pub fn drain() -> Vec<HostAttackTargetReadyEvent> {
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
        record(ObjectId(5), None, Some(ObjectId(9)));
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].object.0, 5);
        assert_eq!(d[0].previous_target, None);
        assert_eq!(d[0].new_target, Some(ObjectId(9)));
        assert!(drain().is_empty());
        clear();
    }
}
