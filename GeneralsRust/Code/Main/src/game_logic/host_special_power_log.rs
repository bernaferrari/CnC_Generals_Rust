//! Frame-local host special-power ready log for GameWorld SetSpecialPower parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostSpecialPowerEvent {
    pub object: ObjectId,
    pub ready: bool,
}

thread_local! {
    static LOG: RefCell<Vec<HostSpecialPowerEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, ready: bool) {
    LOG.with(|log| {
        log.borrow_mut()
            .push(HostSpecialPowerEvent { object, ready });
    });
}

pub fn drain() -> Vec<HostSpecialPowerEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
