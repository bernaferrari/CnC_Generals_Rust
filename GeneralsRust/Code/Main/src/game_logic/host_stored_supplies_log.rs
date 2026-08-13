//! Frame-local host unit/structure stored-supplies log for GameWorld parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostStoredSuppliesEvent {
    pub object: ObjectId,
    pub supplies: u32,
}

thread_local! {
    static LOG: RefCell<Vec<HostStoredSuppliesEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, supplies: u32) {
    LOG.with(|log| {
        log.borrow_mut()
            .push(HostStoredSuppliesEvent { object, supplies });
    });
}

pub fn has_pending(object: ObjectId) -> bool {
    LOG.with(|log| log.borrow().iter().any(|e| e.object == object))
}

pub fn drain() -> Vec<HostStoredSuppliesEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

/// Move the queue out for a whole-world staging boundary without dropping the
/// active world's pending presentation mutations.
pub(crate) fn take_for_world_stage() -> Vec<HostStoredSuppliesEvent> {
    drain()
}

/// Restore or install the queue owned by a whole-world staging boundary.
pub(crate) fn replace_for_world_stage(
    next: Vec<HostStoredSuppliesEvent>,
) -> Vec<HostStoredSuppliesEvent> {
    LOG.with(|log| std::mem::replace(&mut *log.borrow_mut(), next))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}

pub fn len() -> usize {
    LOG.with(|log| log.borrow().len())
}
