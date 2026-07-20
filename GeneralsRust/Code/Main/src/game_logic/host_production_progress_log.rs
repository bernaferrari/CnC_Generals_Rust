//! Frame-local host production queue progress for GameWorld SetProductionQueue parity.

use super::ObjectId;
use std::cell::RefCell;

/// Snapshot of one production queue entry residual.
#[derive(Debug, Clone, PartialEq)]
pub struct HostProductionQueueItem {
    pub template_name: String,
    pub progress: f32,
    pub total_time: f32,
    pub cost_supplies: u32,
    pub is_upgrade: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HostProductionProgressEvent {
    pub producer: ObjectId,
    pub items: Vec<HostProductionQueueItem>,
}

thread_local! {
    static LOG: RefCell<Vec<HostProductionProgressEvent>> = RefCell::new(Vec::new());
}

pub fn record(producer: ObjectId, items: Vec<HostProductionQueueItem>) {
    LOG.with(|log| {
        log.borrow_mut()
            .push(HostProductionProgressEvent { producer, items });
    });
}

pub fn drain() -> Vec<HostProductionProgressEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
