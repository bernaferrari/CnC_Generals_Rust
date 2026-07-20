//! Frame-local host DeathType residual for GameWorld SetDeathType parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostDeathTypeEvent {
    pub object: ObjectId,
    /// HostDeathType ordinal.
    pub death_type: u8,
}

thread_local! {
    static LOG: RefCell<Vec<HostDeathTypeEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, death_type: u8) {
    LOG.with(|log| {
        log.borrow_mut()
            .push(HostDeathTypeEvent { object, death_type });
    });
}

pub fn drain() -> Vec<HostDeathTypeEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
