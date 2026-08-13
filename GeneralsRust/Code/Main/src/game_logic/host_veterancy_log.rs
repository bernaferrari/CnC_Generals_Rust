//! Frame-local host veterancy log for GameWorld SetVeterancy parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostVeterancyEvent {
    pub object: ObjectId,
    /// 0 Rookie, 1 Veteran, 2 Elite, 3 Heroic.
    pub ordinal: u8,
}

thread_local! {
    static LOG: RefCell<Vec<HostVeterancyEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, ordinal: u8) {
    LOG.with(|log| {
        log.borrow_mut().push(HostVeterancyEvent {
            object,
            ordinal: ordinal.min(3),
        });
    });
}

pub fn drain() -> Vec<HostVeterancyEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

/// Move the queue out for a whole-world staging boundary without dropping the
/// active world's pending presentation mutations.
pub(crate) fn take_for_world_stage() -> Vec<HostVeterancyEvent> {
    drain()
}

/// Restore or install the queue owned by a whole-world staging boundary.
pub(crate) fn replace_for_world_stage(next: Vec<HostVeterancyEvent>) -> Vec<HostVeterancyEvent> {
    LOG.with(|log| std::mem::replace(&mut *log.borrow_mut(), next))
}

pub fn len() -> usize {
    LOG.with(|log| log.borrow().len())
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
