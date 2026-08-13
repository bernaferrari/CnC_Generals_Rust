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

pub fn has_pending(object: ObjectId) -> bool {
    LOG.with(|log| log.borrow().iter().any(|e| e.object == object))
}

pub fn drain() -> Vec<HostCommandSetEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

/// Move the queue out for a whole-world staging boundary without dropping the
/// active world's pending presentation mutations.
pub(crate) fn take_for_world_stage() -> Vec<HostCommandSetEvent> {
    drain()
}

/// Restore or install the queue owned by a whole-world staging boundary.
pub(crate) fn replace_for_world_stage(next: Vec<HostCommandSetEvent>) -> Vec<HostCommandSetEvent> {
    LOG.with(|log| std::mem::replace(&mut *log.borrow_mut(), next))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}

pub fn len() -> usize {
    LOG.with(|log| log.borrow().len())
}
