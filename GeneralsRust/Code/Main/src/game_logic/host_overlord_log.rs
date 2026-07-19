//! Frame-local host Overlord/Helix addon log for GameWorld SetOverlordAddon parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostOverlordEvent {
    pub object: ObjectId,
    pub has_gattling: bool,
    pub has_propaganda: bool,
    /// Host Option slots; `u16::MAX` means None/not a bunker residual.
    pub bunker_capacity: u16,
    pub is_helix_transport: bool,
}

thread_local! {
    static LOG: RefCell<Vec<HostOverlordEvent>> = RefCell::new(Vec::new());
}

pub fn record(
    object: ObjectId,
    has_gattling: bool,
    has_propaganda: bool,
    bunker_capacity: u16,
    is_helix_transport: bool,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostOverlordEvent {
            object,
            has_gattling,
            has_propaganda,
            bunker_capacity,
            is_helix_transport,
        });
    });
}

pub fn drain() -> Vec<HostOverlordEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
