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

pub fn drain() -> Vec<HostExperienceEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
