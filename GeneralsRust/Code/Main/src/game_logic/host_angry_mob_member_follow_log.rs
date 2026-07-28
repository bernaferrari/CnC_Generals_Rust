//! Frame-local AngryMob member destroy logs for GW shadow parity.

use super::ObjectId;
use std::cell::RefCell;

thread_local! {
    static DESTROY: RefCell<Vec<ObjectId>> = RefCell::new(Vec::new());
}

pub fn record_destroy(id: ObjectId) {
    DESTROY.with(|l| l.borrow_mut().push(id));
}

pub fn drain_destroys() -> Vec<ObjectId> {
    DESTROY.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn clear() {
    DESTROY.with(|l| l.borrow_mut().clear());
}
