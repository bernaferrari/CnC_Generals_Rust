//! Frame-local host model_condition_bits log for GameWorld SetModelCondition parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostModelConditionEvent {
    pub object: ObjectId,
    pub model_condition_bits: u128,
}

thread_local! {
    static LOG: RefCell<Vec<HostModelConditionEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, model_condition_bits: u128) {
    LOG.with(|log| {
        log.borrow_mut().push(HostModelConditionEvent {
            object,
            model_condition_bits,
        });
    });
}

pub fn drain() -> Vec<HostModelConditionEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
