//! Wave 628: GameWorld contain/garrison membership ready residual log.
//!
//! When `writeback_contain_to_host` changes `contained_by` or garrison/occupant
//! lists, it records here. Host drains and applies AI/status residual (enter
//! Garrisoned / exit Idle counters) so GameWorld owns membership while host
//! owns residual bookkeeping.
//!
//! Fail-closed: empty drain is valid (no membership changes this frame).

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct HostContainReadyEvent {
    pub object: ObjectId,
    /// Previous contained_by host id (0 = none).
    pub previous_contained_by: u32,
    /// New contained_by host id (0 = none).
    pub new_contained_by: u32,
    /// True when this object is a container whose garrison list changed.
    pub garrison_list_changed: bool,
    pub previous_garrison_count: u16,
    pub new_garrison_count: u16,
}

thread_local! {
    static LOG: RefCell<Vec<HostContainReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostContainReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(
    object: ObjectId,
    previous_contained_by: u32,
    new_contained_by: u32,
    garrison_list_changed: bool,
    previous_garrison_count: u16,
    new_garrison_count: u16,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostContainReadyEvent {
            object,
            previous_contained_by,
            new_contained_by,
            garrison_list_changed,
            previous_garrison_count,
            new_garrison_count,
        });
    });
}

pub fn drain() -> Vec<HostContainReadyEvent> {
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
        record(ObjectId(8), 0, 3, false, 0, 0);
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].object.0, 8);
        assert_eq!(d[0].new_contained_by, 3);
        assert!(drain().is_empty());
        clear();
    }
}
