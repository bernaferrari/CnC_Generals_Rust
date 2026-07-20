//! Frame-local host faerie-fire log for GameWorld SetFaerieFire parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostFaerieFireEvent {
    pub object: ObjectId,
    pub active: bool,
    pub until_frame: u32,
}

thread_local! {
    static LOG: RefCell<Vec<HostFaerieFireEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, active: bool, until_frame: u32) {
    LOG.with(|log| {
        log.borrow_mut().push(HostFaerieFireEvent {
            object,
            active,
            until_frame,
        });
    });
}

pub fn drain() -> Vec<HostFaerieFireEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
