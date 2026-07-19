//! Frame-local host command-set override log for GameWorld SetCommandSet parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCommandSetEvent {
    pub object: ObjectId,
    /// Empty string clears override.
    pub command_set: String,
}

thread_local! {
    static LOG: RefCell<Vec<HostCommandSetEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, command_set: Option<String>) {
    LOG.with(|log| {
        log.borrow_mut().push(HostCommandSetEvent {
            object,
            command_set: command_set.unwrap_or_default(),
        });
    });
}

pub fn drain() -> Vec<HostCommandSetEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
