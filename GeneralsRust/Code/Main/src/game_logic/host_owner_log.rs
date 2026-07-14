//! Frame-local host ownership/team transfer log for GameWorld shadow parity.
//!
//! Capture, hijack, snipe-unmanned, car-bomb convert, and other `set_team` writes
//! feed `WorldMutation::TransferOwner` on the shadow session.

use super::{ObjectId, Team};
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostOwnerEvent {
    pub object: ObjectId,
    pub team: Team,
}

thread_local! {
    static LOG: RefCell<Vec<HostOwnerEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, team: Team) {
    LOG.with(|log| {
        log.borrow_mut().push(HostOwnerEvent { object, team });
    });
}

pub fn drain() -> Vec<HostOwnerEvent> {
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
        record(ObjectId(1), Team::USA);
        assert_eq!(drain().len(), 1);
        assert!(drain().is_empty());
    }
}
