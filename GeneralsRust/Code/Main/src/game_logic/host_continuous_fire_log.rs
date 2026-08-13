//! Frame-local host continuous-fire log for GameWorld SetContinuousFire parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostContinuousFireEvent {
    pub object: ObjectId,
    pub level: u8,
    pub consecutive: u16,
    pub coast_until_frame: u32,
}

thread_local! {
    static LOG: RefCell<Vec<HostContinuousFireEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, level: u8, consecutive: u16, coast_until_frame: u32) {
    LOG.with(|log| {
        log.borrow_mut().push(HostContinuousFireEvent {
            object,
            level,
            consecutive,
            coast_until_frame,
        });
    });
}

pub fn has_pending(object: ObjectId) -> bool {
    LOG.with(|log| log.borrow().iter().any(|e| e.object == object))
}

pub fn drain() -> Vec<HostContinuousFireEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

/// Move the queue out for a whole-world staging boundary without dropping the
/// active world's pending presentation mutations.
pub(crate) fn take_for_world_stage() -> Vec<HostContinuousFireEvent> {
    drain()
}

/// Restore or install the queue owned by a whole-world staging boundary.
pub(crate) fn replace_for_world_stage(
    next: Vec<HostContinuousFireEvent>,
) -> Vec<HostContinuousFireEvent> {
    LOG.with(|log| std::mem::replace(&mut *log.borrow_mut(), next))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}

pub fn len() -> usize {
    LOG.with(|log| log.borrow().len())
}
