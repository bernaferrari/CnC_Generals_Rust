//! Frame-local host contain/garrison log for GameWorld SetContain parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostContainEvent {
    pub object: ObjectId,
    /// Passenger residual: host container id (0 = none).
    pub contained_by_host: u32,
    /// Container residual: garrison count (None = leave unchanged on apply).
    pub garrison_count: Option<u16>,
    /// Container residual: garrisoned host object ids.
    pub garrisoned_host_ids: Option<Vec<u32>>,
}

thread_local! {
    static LOG: RefCell<Vec<HostContainEvent>> = RefCell::new(Vec::new());
}

pub fn record_contained_by(object: ObjectId, container: Option<ObjectId>) {
    LOG.with(|log| {
        log.borrow_mut().push(HostContainEvent {
            object,
            contained_by_host: container.map(|c| c.0).unwrap_or(0),
            garrison_count: None,
            garrisoned_host_ids: None,
        });
    });
}

pub fn record_garrison(object: ObjectId, unit_ids: &[ObjectId], max_garrison: u16) {
    let ids: Vec<u32> = unit_ids.iter().map(|id| id.0).collect();
    let count = ids.len().min(u16::MAX as usize) as u16;
    let _ = max_garrison;
    LOG.with(|log| {
        log.borrow_mut().push(HostContainEvent {
            object,
            contained_by_host: 0,
            garrison_count: Some(count),
            garrisoned_host_ids: Some(ids),
        });
    });
}

pub fn drain() -> Vec<HostContainEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
