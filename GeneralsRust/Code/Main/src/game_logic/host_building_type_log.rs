//! Frame-local host building-type log for GameWorld SetBuildingType parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostBuildingTypeEvent {
    pub object: ObjectId,
    pub is_building: bool,
    /// 255 = not a building / unknown.
    pub building_type_ordinal: u8,
}

thread_local! {
    static LOG: RefCell<Vec<HostBuildingTypeEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, is_building: bool, building_type_ordinal: u8) {
    LOG.with(|log| {
        log.borrow_mut().push(HostBuildingTypeEvent {
            object,
            is_building,
            building_type_ordinal,
        });
    });
}

pub fn has_pending(object: ObjectId) -> bool {
    LOG.with(|log| log.borrow().iter().any(|e| e.object == object))
}

pub fn drain() -> Vec<HostBuildingTypeEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

/// Move the queue out for a whole-world staging boundary without dropping the
/// active world's pending presentation mutations.
pub(crate) fn take_for_world_stage() -> Vec<HostBuildingTypeEvent> {
    drain()
}

/// Restore or install the queue owned by a whole-world staging boundary.
pub(crate) fn replace_for_world_stage(
    next: Vec<HostBuildingTypeEvent>,
) -> Vec<HostBuildingTypeEvent> {
    LOG.with(|log| std::mem::replace(&mut *log.borrow_mut(), next))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}

pub fn len() -> usize {
    LOG.with(|log| log.borrow().len())
}
