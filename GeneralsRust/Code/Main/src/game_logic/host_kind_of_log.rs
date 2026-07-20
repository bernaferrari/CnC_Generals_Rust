//! Frame-local host kind_of bits log for GameWorld SetKindOfBits parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostKindOfEvent {
    pub object: ObjectId,
    pub kind_of_bits: u32,
}

thread_local! {
    static LOG: RefCell<Vec<HostKindOfEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, kind_of_bits: u32) {
    LOG.with(|log| {
        log.borrow_mut().push(HostKindOfEvent {
            object,
            kind_of_bits,
        });
    });
}

pub fn drain() -> Vec<HostKindOfEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
