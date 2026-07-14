//! Frame-local host destroy log for GameWorld shadow parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostDestroyEvent {
    pub id: ObjectId,
}

thread_local! {
    static LOG: RefCell<Vec<HostDestroyEvent>> = RefCell::new(Vec::new());
}

pub fn record(id: ObjectId) {
    LOG.with(|log| log.borrow_mut().push(HostDestroyEvent { id }));
}

pub fn drain() -> Vec<HostDestroyEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}

pub fn len() -> usize {
    LOG.with(|log| log.borrow().len())
}
