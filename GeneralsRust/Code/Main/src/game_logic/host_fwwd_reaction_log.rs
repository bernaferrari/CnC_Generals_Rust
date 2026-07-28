//! Frame-local FireWeaponWhenDamaged reaction log for GW shadow parity.
//!
//! Under damage authority / coupled dual-tick, GW sole-emits onDamage reaction
//! weapon names here so host can apply without dual-ticking reaction debounce
//! against pre-writeback HP.

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
