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

pub fn drain() -> Vec<HostAiStateEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
