//! Frame-local host AttackedBy log (C++ ActiveBody.cpp:574-583).
//!
//! `Object::take_damage` records the victim and source object. GameLogic
//! drains the log and calls `Player::set_attacked_by` so PLAYER_ATTACKED_BY
//! / SKIRMISH_PLAYER_HAS_BEEN_ATTACKED_BY_PLAYER scripts can fire.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostAttackedByEvent {
    pub victim: ObjectId,
    pub source: ObjectId,
}

thread_local! {
    static LOG: RefCell<Vec<HostAttackedByEvent>> = const { RefCell::new(Vec::new()) };
}

pub fn record(victim: ObjectId, source: ObjectId) {
    if victim == source {
        return;
    }
    LOG.with(|log| {
        log.borrow_mut()
            .push(HostAttackedByEvent { victim, source });
    });
}

pub fn drain() -> Vec<HostAttackedByEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_drain() {
        clear();
        record(ObjectId(1), ObjectId(2));
        record(ObjectId(1), ObjectId(1));
        let ev = drain();
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0].source, ObjectId(2));
        assert!(drain().is_empty());
    }
}
