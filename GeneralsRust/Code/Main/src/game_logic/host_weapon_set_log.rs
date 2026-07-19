//! Frame-local host weapon-set flag log for GameWorld SetWeaponSetFlags parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostWeaponSetEvent {
    pub object: ObjectId,
    pub player_upgrade: bool,
    pub armed_riders: bool,
}

thread_local! {
    static LOG: RefCell<Vec<HostWeaponSetEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, player_upgrade: bool, armed_riders: bool) {
    LOG.with(|log| {
        log.borrow_mut().push(HostWeaponSetEvent {
            object,
            player_upgrade,
            armed_riders,
        });
    });
}

pub fn drain() -> Vec<HostWeaponSetEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
