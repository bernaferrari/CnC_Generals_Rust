//! Frame-local host formation id/offset log for GameWorld SetFormation parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostFormationEvent {
    pub object: ObjectId,
    pub formation_id: u32,
    /// Host XZ offset residual.
    pub formation_offset: [f32; 2],
}

thread_local! {
    static LOG: RefCell<Vec<HostFormationEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, formation_id: u32, formation_offset: [f32; 2]) {
    LOG.with(|log| {
        log.borrow_mut().push(HostFormationEvent {
            object,
            formation_id,
            formation_offset,
        });
    });
}

pub fn drain() -> Vec<HostFormationEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
