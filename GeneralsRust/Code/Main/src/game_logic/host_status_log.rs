//! Under-construction footprint requests (receiving-owner migration pending).
use super::ObjectId;
use std::cell::RefCell;

thread_local! {
    static UC_MINE_SWEEP: RefCell<Vec<ObjectId>> = RefCell::new(Vec::new());
}

/// C++ Object::setStatus UnderConstruction edge — leftover set_status sweep.
pub fn request_under_construction_mine_sweep(object: ObjectId) {
    UC_MINE_SWEEP.with(|q| {
        let mut q = q.borrow_mut();
        if !q.contains(&object) {
            q.push(object);
        }
    });
}
pub fn drain_under_construction_mine_sweeps() -> Vec<ObjectId> {
    UC_MINE_SWEEP.with(|q| std::mem::take(&mut *q.borrow_mut()))
}
