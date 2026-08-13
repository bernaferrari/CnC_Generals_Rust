//! Frame-local host AI-state log for GameWorld SetAiState parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostAiStateEvent {
    pub object: ObjectId,
    /// Matches GameWorldShadow::host_ai_state_ordinal.
    pub ordinal: u8,
}

thread_local! {
    static LOG: RefCell<Vec<HostAiStateEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, ordinal: u8) {
    LOG.with(|log| {
        log.borrow_mut().push(HostAiStateEvent { object, ordinal });
    });
}

pub fn has_pending(object: ObjectId) -> bool {
    LOG.with(|log| log.borrow().iter().any(|e| e.object == object))
}

pub fn drain() -> Vec<HostAiStateEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

/// Move the queue out for a whole-world staging boundary without dropping the
/// active world's pending presentation mutations.
pub(crate) fn take_for_world_stage() -> Vec<HostAiStateEvent> {
    drain()
}

/// Restore or install the queue owned by a whole-world staging boundary.
pub(crate) fn replace_for_world_stage(next: Vec<HostAiStateEvent>) -> Vec<HostAiStateEvent> {
    LOG.with(|log| std::mem::replace(&mut *log.borrow_mut(), next))
}

pub fn len() -> usize {
    LOG.with(|log| log.borrow().len())
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
