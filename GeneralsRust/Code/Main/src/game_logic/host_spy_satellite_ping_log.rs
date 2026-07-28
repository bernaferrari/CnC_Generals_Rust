//! Frame-local SpySatellite ping expire logs for GW shadow parity.

use super::ObjectId;
use std::cell::RefCell;

thread_local! {
    static EXPIRES: RefCell<Vec<ObjectId>> = RefCell::new(Vec::new());
}

pub fn record_expire(id: ObjectId) {
    EXPIRES.with(|l| l.borrow_mut().push(id));
}

pub fn drain_expires() -> Vec<ObjectId> {
    EXPIRES.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn clear() {
    EXPIRES.with(|l| l.borrow_mut().clear());
}
