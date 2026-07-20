//! Frame-local host ground-height log for GameWorld SetGroundHeight parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostGroundHeightEvent {
    pub object: ObjectId,
    pub ground_height: f32,
    pub from_terrain: bool,
}

thread_local! {
    static LOG: RefCell<Vec<HostGroundHeightEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, ground_height: f32, from_terrain: bool) {
    LOG.with(|log| {
        log.borrow_mut().push(HostGroundHeightEvent {
            object,
            ground_height,
            from_terrain,
        });
    });
}

pub fn drain() -> Vec<HostGroundHeightEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
