//! Frame-local host destroy log for GameWorld shadow parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostDestroyEvent {
    pub id: ObjectId,
}

thread_local! {
    static LOG: RefCell<Vec<HostDestroyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostDestroyEvent>> = RefCell::new(Vec::new());
}

pub fn record(id: ObjectId) {
    LOG.with(|log| log.borrow_mut().push(HostDestroyEvent { id }));
}

pub fn drain() -> Vec<HostDestroyEvent> {
    let v = LOG.with(|log| std::mem::take(&mut *log.borrow_mut()));
    // Keep last non-empty batch for PresentationFrame after shadow session.
    if !v.is_empty() {
        LAST_DRAIN.with(|last| *last.borrow_mut() = v.clone());
    }
    v
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
    LAST_DRAIN.with(|last| last.borrow_mut().clear());
}

pub fn len() -> usize {
    LOG.with(|log| log.borrow().len())
}

/// Events from the most recent `drain()` (presentation residual after shadow).
pub fn last_drain_snapshot() -> Vec<HostDestroyEvent> {
    LAST_DRAIN.with(|last| last.borrow().clone())
}
