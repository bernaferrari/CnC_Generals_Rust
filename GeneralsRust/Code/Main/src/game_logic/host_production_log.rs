//! Frame-local host production enqueue log for GameWorld shadow parity.
//!
//! Completions already flow through `host_spawn_log` via `create_object`.
//! This log captures queue intent (producer + template) for command-channel probes.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostProductionEvent {
    pub producer: ObjectId,
    pub template_name: String,
}

thread_local! {
    static LOG: RefCell<Vec<HostProductionEvent>> = RefCell::new(Vec::new());
}

pub fn record(producer: ObjectId, template_name: impl Into<String>) {
    LOG.with(|log| {
        log.borrow_mut().push(HostProductionEvent {
            producer,
            template_name: template_name.into(),
        });
    });
}

pub fn drain() -> Vec<HostProductionEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
