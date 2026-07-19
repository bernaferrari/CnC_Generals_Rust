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

pub fn drain() -> Vec<HostAiAttitudeEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
