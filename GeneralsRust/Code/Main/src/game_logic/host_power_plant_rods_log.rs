//! Frame-local power plant rods completion logs for GW shadow parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct PowerPlantRodsCompleteEvent {
    pub id: ObjectId,
    pub model_condition_bits: u128,
}

thread_local! {
    static COMPLETE: RefCell<Vec<PowerPlantRodsCompleteEvent>> = RefCell::new(Vec::new());
}

pub fn record_complete(ev: PowerPlantRodsCompleteEvent) {
    COMPLETE.with(|l| l.borrow_mut().push(ev));
}

pub fn drain_completes() -> Vec<PowerPlantRodsCompleteEvent> {
    COMPLETE.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn clear() {
    COMPLETE.with(|l| l.borrow_mut().clear());
}
