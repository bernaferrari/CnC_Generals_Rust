//! Frame-local host construction progress log for GameWorld SetConstruction parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostConstructionProgressEvent {
    pub object: ObjectId,
    /// -1.0 .. 1.0 (sell deconstruction goes negative; finish at -0.5)
    pub percent: f32,
    pub under_construction: bool,
    /// Host residual: base_rate * dozers * power (units: fraction per second).
    /// Used by GameWorld sole-tick under CONSTRUCTION_AUTHORITY.
    pub effective_rate: f32,
}

thread_local! {
    static LOG: RefCell<Vec<HostConstructionProgressEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, percent: f32, under_construction: bool, effective_rate: f32) {
    LOG.with(|log| {
        log.borrow_mut().push(HostConstructionProgressEvent {
            object,
            percent: percent.clamp(-1.0, 1.0),
            under_construction,
            effective_rate,
        });
    });
}

pub fn drain() -> Vec<HostConstructionProgressEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
