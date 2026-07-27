//! Wave 639: GameWorld move-target writeback ready residual log.
//!
//! When `writeback_move_targets_to_host` changes destination, it records here.
//! Host drains and applies movement residual (AI/status/movement log) so
//! GameWorld owns the move-target last-write while host owns residual side
//! effects.
//!
//! Fail-closed: empty drain is valid (no move-target changes this frame).

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostMoveTargetReadyEvent {
    pub object: ObjectId,
    pub previous_target: Option<[f32; 3]>,
    pub new_target: Option<[f32; 3]>,
}

thread_local! {
    static LOG: RefCell<Vec<HostMoveTargetReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostMoveTargetReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, previous_target: Option<[f32; 3]>, new_target: Option<[f32; 3]>) {
    LOG.with(|log| {
        log.borrow_mut().push(HostMoveTargetReadyEvent {
            object,
            previous_target,
            new_target,
        });
    });
}

pub fn drain() -> Vec<HostMoveTargetReadyEvent> {
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
        record(ObjectId(8), None, Some([1.0, 0.0, 2.0]));
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].object.0, 8);
        assert_eq!(d[0].previous_target, None);
        assert_eq!(d[0].new_target, Some([1.0, 0.0, 2.0]));
        assert!(drain().is_empty());
        clear();
    }
}
