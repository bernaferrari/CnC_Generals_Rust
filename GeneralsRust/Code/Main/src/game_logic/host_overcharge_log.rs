//! Frame-local host overcharge log for GameWorld SetOvercharge parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostOverchargeEvent {
    pub object: ObjectId,
    pub enabled: bool,
}

thread_local! {
    static LOG: RefCell<Vec<HostOverchargeEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, enabled: bool) {
    LOG.with(|log| {
        log.borrow_mut()
            .push(HostOverchargeEvent { object, enabled });
    });
}

pub fn drain() -> Vec<HostOverchargeEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
