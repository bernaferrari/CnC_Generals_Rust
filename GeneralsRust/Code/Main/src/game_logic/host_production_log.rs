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
    /// Last drained batch (presentation freezes after shadow session drain).
    static LAST_DRAIN: RefCell<Vec<HostProductionEvent>> = RefCell::new(Vec::new());
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

/// Non-destructive peek of undrained log.
pub fn snapshot() -> Vec<HostProductionEvent> {
    LOG.with(|log| log.borrow().clone())
}

/// Take events from the most recent non-empty `drain()` (PresentationFrame sole consumer).
pub fn take_last_drain() -> Vec<HostProductionEvent> {
    LAST_DRAIN.with(|last| std::mem::take(&mut *last.borrow_mut()))
}

/// Non-destructive peek (tests).
pub fn last_drain_snapshot() -> Vec<HostProductionEvent> {
    LAST_DRAIN.with(|last| last.borrow().clone())
}
