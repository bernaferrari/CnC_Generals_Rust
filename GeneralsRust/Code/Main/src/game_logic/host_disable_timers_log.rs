//! Frame-local host disable-timer log for GameWorld SetDisableTimers parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostDisableTimersEvent {
    pub object: ObjectId,
    pub emp_until_frame: u32,
    pub hacked_until_frame: u32,
    pub paralyzed_until_frame: u32,
}

thread_local! {
    static LOG: RefCell<Vec<HostDisableTimersEvent>> = RefCell::new(Vec::new());
}

pub fn record(
    object: ObjectId,
    emp_until_frame: u32,
    hacked_until_frame: u32,
    paralyzed_until_frame: u32,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostDisableTimersEvent {
            object,
            emp_until_frame,
            hacked_until_frame,
            paralyzed_until_frame,
        });
    });
}

pub fn drain() -> Vec<HostDisableTimersEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
