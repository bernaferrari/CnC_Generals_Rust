//! Frame-local host RadarUpdate residual for GameWorld SetRadarExtend parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostRadarExtendEvent {
    pub object: ObjectId,
    pub radar_extend_done_frame: u32,
    pub radar_extend_complete: bool,
    pub radar_active: bool,
}

thread_local! {
    static LOG: RefCell<Vec<HostRadarExtendEvent>> = RefCell::new(Vec::new());
}

pub fn record(
    object: ObjectId,
    radar_extend_done_frame: u32,
    radar_extend_complete: bool,
    radar_active: bool,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostRadarExtendEvent {
            object,
            radar_extend_done_frame,
            radar_extend_complete,
            radar_active,
        });
    });
}

pub fn drain() -> Vec<HostRadarExtendEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
