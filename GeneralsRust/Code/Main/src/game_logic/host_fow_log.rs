//! Frame-local host FOW visibility log for GameWorld SetFow parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostFowEvent {
    pub object: ObjectId,
    pub visibility_alpha: f32,
    pub is_explored: f32,
    pub visibility_falloff: f32,
}

thread_local! {
    static LOG: RefCell<Vec<HostFowEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, visibility_alpha: f32, is_explored: f32, visibility_falloff: f32) {
    LOG.with(|log| {
        log.borrow_mut().push(HostFowEvent {
            object,
            visibility_alpha,
            is_explored,
            visibility_falloff,
        });
    });
}

pub fn drain() -> Vec<HostFowEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
