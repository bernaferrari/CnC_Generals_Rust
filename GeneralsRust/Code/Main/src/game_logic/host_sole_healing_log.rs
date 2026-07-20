//! Frame-local host sole-healing residual for GameWorld SetSoleHealing parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostSoleHealingEvent {
    pub object: ObjectId,
    pub sole_healing_benefactor_id: Option<u32>,
    pub sole_healing_benefactor_expiration_frame: u32,
}

thread_local! {
    static LOG: RefCell<Vec<HostSoleHealingEvent>> = RefCell::new(Vec::new());
}

pub fn record(
    object: ObjectId,
    sole_healing_benefactor_id: Option<u32>,
    sole_healing_benefactor_expiration_frame: u32,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostSoleHealingEvent {
            object,
            sole_healing_benefactor_id,
            sole_healing_benefactor_expiration_frame,
        });
    });
}

pub fn drain() -> Vec<HostSoleHealingEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
