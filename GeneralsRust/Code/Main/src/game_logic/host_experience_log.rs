//! Frame-local host experience log for GameWorld SetExperience parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostExperienceEvent {
    pub object: ObjectId,
    pub points: f32,
}

thread_local! {
    static LOG: RefCell<Vec<HostExperienceEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, points: f32) {
    LOG.with(|log| {
        log.borrow_mut().push(HostExperienceEvent {
            object,
            points: points.max(0.0),
        });
    });
}

pub fn has_pending(object: ObjectId) -> bool {
    LOG.with(|log| log.borrow().iter().any(|e| e.object == object))
}

pub fn drain() -> Vec<HostExperienceEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

/// Move the queue out for a whole-world staging boundary without dropping the
/// active world's pending presentation mutations.
pub(crate) fn take_for_world_stage() -> Vec<HostExperienceEvent> {
    drain()
}

/// Restore or install the queue owned by a whole-world staging boundary.
pub(crate) fn replace_for_world_stage(next: Vec<HostExperienceEvent>) -> Vec<HostExperienceEvent> {
    LOG.with(|log| std::mem::replace(&mut *log.borrow_mut(), next))
}

pub fn len() -> usize {
    LOG.with(|log| log.borrow().len())
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
