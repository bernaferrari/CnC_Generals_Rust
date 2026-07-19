//! Frame-local host active weapon slot log for GameWorld SetActiveWeaponSlot parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostWeaponSlotEvent {
    pub object: ObjectId,
    /// 0 primary, 1 secondary, 2+ tertiary/extra residual.
    pub slot: u8,
}

thread_local! {
    static LOG: RefCell<Vec<HostWeaponSlotEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, slot: u8) {
    LOG.with(|log| {
        log.borrow_mut().push(HostWeaponSlotEvent { object, slot });
    });
}

pub fn drain() -> Vec<HostWeaponSlotEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
