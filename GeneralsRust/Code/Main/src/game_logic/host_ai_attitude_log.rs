//! Frame-local host AI attitude log for GameWorld SetAiAttitude parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostAiAttitudeEvent {
    pub object: ObjectId,
    /// Host AI attitude residual as i8 (-2 Sleep .. +2 Aggressive).
    pub attitude: i8,
}

thread_local! {
    static LOG: RefCell<Vec<HostAiAttitudeEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, attitude: i8) {
    LOG.with(|log| {
        log.borrow_mut().push(HostAiAttitudeEvent {
            object,
            attitude: attitude.clamp(-2, 2),
        });
    });
}

pub fn has_pending(object: ObjectId) -> bool {
    LOG.with(|log| log.borrow().iter().any(|e| e.object == object))
}

pub fn drain() -> Vec<HostAiAttitudeEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

/// Move the queue out for a whole-world staging boundary without dropping the
/// active world's pending presentation mutations.
pub(crate) fn take_for_world_stage() -> Vec<HostAiAttitudeEvent> {
    drain()
}

/// Restore or install the queue owned by a whole-world staging boundary.
pub(crate) fn replace_for_world_stage(next: Vec<HostAiAttitudeEvent>) -> Vec<HostAiAttitudeEvent> {
    LOG.with(|log| std::mem::replace(&mut *log.borrow_mut(), next))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}

pub fn len() -> usize {
    LOG.with(|log| log.borrow().len())
}
