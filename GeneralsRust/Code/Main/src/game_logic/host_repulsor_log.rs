//! Frame-local host repulsor log for GameWorld SetRepulsor parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostRepulsorEvent {
    pub object: ObjectId,
    pub active: bool,
    /// Remaining countdown frames (0 = permanent or cleared).
    pub until_frame: u32,
}

thread_local! {
    static LOG: RefCell<Vec<HostRepulsorEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, active: bool, until_frame: u32) {
    LOG.with(|log| {
        log.borrow_mut().push(HostRepulsorEvent {
            object,
            active,
            until_frame,
        });
    });
}

pub fn drain() -> Vec<HostRepulsorEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
