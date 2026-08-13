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

pub fn has_pending(object: ObjectId) -> bool {
    LOG.with(|log| log.borrow().iter().any(|e| e.object == object))
}

pub fn drain() -> Vec<HostTargetLocationEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

/// Move the queue out for a whole-world staging boundary without dropping the
/// active world's pending presentation mutations.
pub(crate) fn take_for_world_stage() -> Vec<HostTargetLocationEvent> {
    drain()
}

/// Restore or install the queue owned by a whole-world staging boundary.
pub(crate) fn replace_for_world_stage(
    next: Vec<HostTargetLocationEvent>,
) -> Vec<HostTargetLocationEvent> {
    LOG.with(|log| std::mem::replace(&mut *log.borrow_mut(), next))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}

pub fn len() -> usize {
    LOG.with(|log| log.borrow().len())
}
