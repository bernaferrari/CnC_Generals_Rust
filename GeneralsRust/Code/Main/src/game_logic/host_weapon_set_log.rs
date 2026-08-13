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

pub fn has_pending(object: ObjectId) -> bool {
    LOG.with(|log| log.borrow().iter().any(|e| e.object == object))
}

pub fn drain() -> Vec<HostWeaponSetEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

/// Move the queue out for a whole-world staging boundary without dropping the
/// active world's pending presentation mutations.
pub(crate) fn take_for_world_stage() -> Vec<HostWeaponSetEvent> {
    drain()
}

/// Restore or install the queue owned by a whole-world staging boundary.
pub(crate) fn replace_for_world_stage(next: Vec<HostWeaponSetEvent>) -> Vec<HostWeaponSetEvent> {
    LOG.with(|log| std::mem::replace(&mut *log.borrow_mut(), next))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}

pub fn len() -> usize {
    LOG.with(|log| log.borrow().len())
}
