//! Frame-local host production log for GameWorld shadow parity.
//!
//! - Enqueue: queue intent (producer + template)
//! - Complete: producer finished a unit (spawned id + template); spawn also
//!   flows through `host_spawn_log` via `create_object`.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostProductionEvent {
    Enqueue {
        producer: ObjectId,
        template_name: String,
    },
    Complete {
        producer: ObjectId,
        template_name: String,
        spawned: ObjectId,
    },
}

thread_local! {
    static LOG: RefCell<Vec<HostProductionEvent>> = RefCell::new(Vec::new());
}

pub fn record_enqueue(producer: ObjectId, template_name: impl Into<String>) {
    LOG.with(|log| {
        log.borrow_mut().push(HostProductionEvent::Enqueue {
            producer,
            template_name: template_name.into(),
        });
    });
}

/// Backward-compatible alias used by existing enqueue sites.
pub fn record(producer: ObjectId, template_name: impl Into<String>) {
    record_enqueue(producer, template_name);
}

pub fn record_complete(producer: ObjectId, template_name: impl Into<String>, spawned: ObjectId) {
    LOG.with(|log| {
        log.borrow_mut().push(HostProductionEvent::Complete {
            producer,
            template_name: template_name.into(),
            spawned,
        });
    });
}

pub fn drain() -> Vec<HostProductionEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
