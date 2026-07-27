//! Wave 629: GameWorld owner/team-change ready residual log.
//!
//! When `writeback_owner_to_host` changes an object's team, it records here.
//! Host drains and runs `on_capture_object_residual` so GameWorld owns the
//! owner last-write while host owns capture kick/deselect/idle/score residual.
//!
//! Fail-closed: empty drain is valid (no owner changes this frame).

use crate::game_logic::{ObjectId, Team};
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct HostOwnerReadyEvent {
    pub object: ObjectId,
    pub previous_team: Team,
    pub new_team: Team,
}

thread_local! {
    static LOG: RefCell<Vec<HostOwnerReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostOwnerReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, previous_team: Team, new_team: Team) {
    LOG.with(|log| {
        log.borrow_mut().push(HostOwnerReadyEvent {
            object,
            previous_team,
            new_team,
        });
    });
}

pub fn drain() -> Vec<HostOwnerReadyEvent> {
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
        record(ObjectId(2), Team::GLA, Team::USA);
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].object.0, 2);
        assert_eq!(d[0].previous_team, Team::GLA);
        assert_eq!(d[0].new_team, Team::USA);
        assert!(drain().is_empty());
        clear();
    }
}
