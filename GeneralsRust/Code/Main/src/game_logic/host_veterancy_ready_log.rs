//! Wave 622: GameWorld veterancy level-up ready residual log.
//!
//! Under DAMAGE_AUTHORITY, `writeback_experience_to_host` records objects whose
//! veterancy level last-write changed. Host drains this log to apply
//! `apply_veterancy_bonuses` (HP/weapon scales) so GameWorld decides rank while
//! host still owns residual combat stats.
//!
//! Fail-closed: empty drain is valid (no level-ups this frame).

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct HostVeterancyReadyEvent {
    pub object: ObjectId,
    pub previous_ordinal: u8,
    pub new_ordinal: u8,
    pub points: f32,
}

thread_local! {
    static LOG: RefCell<Vec<HostVeterancyReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostVeterancyReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, previous_ordinal: u8, new_ordinal: u8, points: f32) {
    LOG.with(|log| {
        log.borrow_mut().push(HostVeterancyReadyEvent {
            object,
            previous_ordinal,
            new_ordinal,
            points,
        });
    });
}

pub fn drain() -> Vec<HostVeterancyReadyEvent> {
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
        record(ObjectId(5), 0, 1, 100.0);
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].object.0, 5);
        assert_eq!(d[0].previous_ordinal, 0);
        assert_eq!(d[0].new_ordinal, 1);
        assert!(drain().is_empty());
        clear();
    }
}
