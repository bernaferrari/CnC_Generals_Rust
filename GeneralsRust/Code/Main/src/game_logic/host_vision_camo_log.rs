//! Frame-local host vision-spied / camo residual log for GameWorld SetVisionCamo parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostVisionCamoEvent {
    pub object: ObjectId,
    pub vision_spied_mask: u32,
    pub camo_friendly_opacity: f32,
    pub camo_stealth_look: u8,
}

thread_local! {
    static LOG: RefCell<Vec<HostVisionCamoEvent>> = RefCell::new(Vec::new());
}

pub fn record(
    object: ObjectId,
    vision_spied_mask: u32,
    camo_friendly_opacity: f32,
    camo_stealth_look: u8,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostVisionCamoEvent {
            object,
            vision_spied_mask,
            camo_friendly_opacity,
            camo_stealth_look,
        });
    });
}

pub fn drain() -> Vec<HostVisionCamoEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
