//! Frame-local host guard log for GameWorld SetGuard parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostGuardEvent {
    pub object: ObjectId,
    /// Guard area anchor (world XYZ). None clears area guard.
    pub position: Option<[f32; 3]>,
    /// Guard object target as host object id (0 = none).
    pub target_host: u32,
    /// C++ GuardArea radius residual (world units).
    pub radius: f32,
}

thread_local! {
    static LOG: RefCell<Vec<HostGuardEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, position: Option<[f32; 3]>, target_host: u32, radius: f32) {
    LOG.with(|log| {
        log.borrow_mut().push(HostGuardEvent {
            object,
            position,
            target_host,
            radius: radius.max(0.0),
        });
    });
}

pub fn drain() -> Vec<HostGuardEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
