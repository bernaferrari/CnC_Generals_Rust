//! Frame-local host construction progress log for GameWorld SetConstruction parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostConstructionProgressEvent {
    pub object: ObjectId,
    /// 0.0 .. 1.0
    pub percent: f32,
    pub under_construction: bool,
}

thread_local! {
    static LOG: RefCell<Vec<HostConstructionProgressEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, percent: f32, under_construction: bool) {
    LOG.with(|log| {
        log.borrow_mut().push(HostConstructionProgressEvent {
            object,
            percent: percent.clamp(0.0, 1.0),
            under_construction,
        });
    });
}

pub fn drain() -> Vec<HostConstructionProgressEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
