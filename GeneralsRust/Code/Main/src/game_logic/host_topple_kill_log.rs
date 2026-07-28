//! Frame-local ToppleUpdate kill log for GameWorld shadow parity.
//!
//! Under coupled dual-tick, GW sole-ticks topple fall and records kill-when-down
//! here so host can destroy without dual-ticking the topple state machine.

use super::ObjectId;
use std::cell::RefCell;

thread_local! {
    static LOG: RefCell<Vec<ObjectId>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId) {
    LOG.with(|log| log.borrow_mut().push(object));
}

pub fn has_pending(object: ObjectId) -> bool {
    LOG.with(|log| log.borrow().iter().any(|&id| id == object))
}

pub fn drain() -> Vec<ObjectId> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
