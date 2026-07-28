//! Frame-local dozer bored-time logs for GW shadow parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct DozerBoredEvent {
    pub id: ObjectId,
}

thread_local! {
    static EVENTS: RefCell<Vec<DozerBoredEvent>> = RefCell::new(Vec::new());
}

pub fn record(ev: DozerBoredEvent) {
    EVENTS.with(|l| l.borrow_mut().push(ev));
}

pub fn drain() -> Vec<DozerBoredEvent> {
    EVENTS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn clear() {
    EVENTS.with(|l| l.borrow_mut().clear());
}
