//! Frame-local host selection_radius log for GameWorld SetSelectionRadius parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostSelectionRadiusEvent {
    pub object: ObjectId,
    pub selection_radius: f32,
}

thread_local! {
    static LOG: RefCell<Vec<HostSelectionRadiusEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, selection_radius: f32) {
    LOG.with(|log| {
        log.borrow_mut().push(HostSelectionRadiusEvent {
            object,
            selection_radius,
        });
    });
}

pub fn drain() -> Vec<HostSelectionRadiusEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
