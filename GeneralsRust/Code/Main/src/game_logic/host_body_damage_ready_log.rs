//! Wave 623: GameWorld body-damage state-change ready residual log.
//!
//! Under DAMAGE_AUTHORITY, `writeback_body_damage_to_host` records objects whose
//! BodyDamageType last-write changed. Host drains this log to apply model-condition
//! bits + BoneFX/TransitionDamageFX residual so GameWorld decides body state while
//! host owns visual/FX side effects.
//!
//! Fail-closed: empty drain is valid (no body-damage transitions this frame).

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct HostBodyDamageReadyEvent {
    pub object: ObjectId,
    pub previous_ordinal: u8,
    pub new_ordinal: u8,
}

thread_local! {
    static LOG: RefCell<Vec<HostBodyDamageReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostBodyDamageReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, previous_ordinal: u8, new_ordinal: u8) {
    LOG.with(|log| {
        log.borrow_mut().push(HostBodyDamageReadyEvent {
            object,
            previous_ordinal: previous_ordinal.min(3),
            new_ordinal: new_ordinal.min(3),
        });
    });
}

pub fn drain() -> Vec<HostBodyDamageReadyEvent> {
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
        record(ObjectId(9), 0, 2);
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].object.0, 9);
        assert_eq!(d[0].previous_ordinal, 0);
        assert_eq!(d[0].new_ordinal, 2);
        assert!(drain().is_empty());
        clear();
    }
}
