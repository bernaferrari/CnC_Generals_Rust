//! Frame-local FireWeaponWhenDamaged continuous fire log for GW shadow parity.
//!
//! Under coupled dual-tick, GW sole-ticks continuous FWWDB and records weapon
//! names here so host can apply without dual-ticking the continuous reload.

use super::ObjectId;
use std::cell::RefCell;

thread_local! {
    static LOG: RefCell<Vec<(ObjectId, String)>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, weapon: String) {
    if weapon.is_empty() {
        return;
    }
    LOG.with(|log| log.borrow_mut().push((object, weapon)));
}

pub fn drain() -> Vec<(ObjectId, String)> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
