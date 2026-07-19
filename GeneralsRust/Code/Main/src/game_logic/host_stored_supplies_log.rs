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

pub fn drain() -> Vec<HostStoredSuppliesEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
