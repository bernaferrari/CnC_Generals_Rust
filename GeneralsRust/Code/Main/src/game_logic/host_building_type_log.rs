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

pub fn drain() -> Vec<HostBuildingTypeEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
