//! Frame-local host ground-attack target location log for GameWorld SetTargetLocation parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostTargetLocationEvent {
    pub object: ObjectId,
    /// None clears ground-attack aim point.
    pub location: Option<[f32; 3]>,
}

thread_local! {
    static LOG: RefCell<Vec<HostTargetLocationEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, location: Option<[f32; 3]>) {
    LOG.with(|log| {
        log.borrow_mut()
            .push(HostTargetLocationEvent { object, location });
    });
}

pub fn drain() -> Vec<HostTargetLocationEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
