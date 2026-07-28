//! Frame-local ACTIVELY_CONSTRUCTING model-bit logs for GW shadow parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct ActivelyConstructingEvent {
    pub id: ObjectId,
    pub model_condition_bits: u128,
    pub want: bool,
}

thread_local! {
    static EVENTS: RefCell<Vec<ActivelyConstructingEvent>> = RefCell::new(Vec::new());
}

pub fn record(ev: ActivelyConstructingEvent) {
    EVENTS.with(|l| l.borrow_mut().push(ev));
}

pub fn drain() -> Vec<ActivelyConstructingEvent> {
    EVENTS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn clear() {
    EVENTS.with(|l| l.borrow_mut().clear());
}
