//! Frame-local host crush/vision residual log for GameWorld SetCrushVision parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostCrushVisionEvent {
    pub object: ObjectId,
    pub crusher_level: u8,
    pub crushable_level: u8,
    pub vision_range: f32,
    pub shroud_clearing_range: f32,
    pub front_crushed: bool,
    pub back_crushed: bool,
}

thread_local! {
    static LOG: RefCell<Vec<HostCrushVisionEvent>> = RefCell::new(Vec::new());
}

pub fn record(
    object: ObjectId,
    crusher_level: u8,
    crushable_level: u8,
    vision_range: f32,
    shroud_clearing_range: f32,
    front_crushed: bool,
    back_crushed: bool,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostCrushVisionEvent {
            object,
            crusher_level,
            crushable_level,
            vision_range,
            shroud_clearing_range,
            front_crushed,
            back_crushed,
        });
    });
}

pub fn drain() -> Vec<HostCrushVisionEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
